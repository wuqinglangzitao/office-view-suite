//! `rwml` — one native Rust reader for **both** Microsoft Word formats: legacy
//! `.doc` (Word 97–2003 binary, [MS-DOC]) and modern `.docx` (OOXML
//! WordprocessingML). No JVM, no Apache POI, no external `.docx` crate, no
//! shelling out — [`Document::open`] format-detects from the magic bytes and
//! both feed the **same** [`DocModel`] and Markdown/HTML exporters.
//!
//! * **`.doc`** is an OLE2/CFB compound file. The text lives in the
//!   `WordDocument` stream; the **piece table** (CLX) in the `0Table`/`1Table`
//!   stream maps character positions to byte offsets, and each piece is either
//!   UTF-16LE (Korean body text) or 8-bit text in the document's ANSI codepage
//!   (`fCompressed` — cp1252 for Western, cp949 for Korean, from the FIB language
//!   id).
//! * **`.docx`** is a ZIP of XML parts (`word/document.xml` + styles, numbering,
//!   relationships, media), parsed with `zip` + `quick-xml` behind the default
//!   `docx` feature. Disable it (`default-features = false`) for a
//!   dependency-light `.doc`-only build.
//!
//! ```no_run
//! // Works for either format — detection is automatic.
//! let bytes = std::fs::read("report.doc").unwrap();
//! let text = rwml::extract_text(&bytes).unwrap();
//! println!("{text}");
//! ```
//!
//! Two surfaces:
//!
//! * **Flat text** — [`extract_text`] / [`Document::text`], the same output as
//!   POI `WordExtractor.getText()` (fast, allocation-light).
//! * **A full document model** — [`Document::model`] (paragraphs, character runs
//!   with bold/italic/…, structured tables with colspan/rowspan, headings,
//!   lists, hyperlinks, and extracted images), plus [`Document::to_markdown`]
//!   and [`Document::to_html`]. Built lazily, so the flat path never pays for it.
//!
//! Parsing **untrusted input** is panic-free / bounds-checked: a malformed or hostile
//! `.doc`/`.docx` yields [`Error`], never a crash. (The only `expect` is on the crate's
//! own compiled-in blank template behind the infallible [`Document::new`]/[`Default`];
//! use [`Document::try_new`] for a `Result` instead of that build-invariant panic.)
//!
//! ---
//! `rwml` (from WordprocessingML) is an independent project, not affiliated with or
//! endorsed by Microsoft. Microsoft Word and the `.doc`/`.docx` formats are Microsoft
//! trademarks, referenced only to indicate format compatibility; the crate is built
//! solely from the public [MS-DOC]/[MS-CFB]/OOXML specifications.

#![forbid(unsafe_code)]
#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]

mod annotation;
mod assemble;
mod builder;
mod chpx;
mod clx;
#[cfg(feature = "docx")]
mod docx;
mod error;
mod export;
mod ffn;
mod fib;
mod image;
mod list;
#[cfg(feature = "docx")]
mod metafile;
mod model;
mod numfmt;
mod ole;
#[cfg(feature = "docx")]
mod opc;
mod papx;
#[cfg(feature = "render")]
mod render;
mod report;
mod stsh;
mod table;
mod text;
mod util;
pub mod wasm;
#[cfg(feature = "docx")]
mod write;
#[cfg(feature = "docx")]
mod xmltree;

pub use annotation::{
    Comment, Field, FieldContext, FieldKind, FloatingShape, HeaderFooter, HeaderFooterKind, Note,
    NoteKind, Revision, RevisionKind, RevisionView, ShapeDistance, ShapeEffectExtent, ShapeExtent,
    ShapePoint, ShapePosition, ShapeWrapping, TextAnchor, TextBox,
};
pub use builder::{
    CellBuilder, ChartBuilder, CommentBuilder, ContentControlBuilder, DocBuilder, ImageBuilder,
    ParagraphBuilder, ParagraphStyleBuilder, RevisionBuilder, RunBuilder, TableBuilder,
};
pub use error::{Error, Result};
pub use model::{
    Align, AuthoredComment, AuthoredContentControl, AuthoredNote, AuthoredRevision, Block, Cell,
    CellMargins, CharProps, Chart, ChartKind, ChartSeries, ChartShape, Color, DocGrid, DocGridType,
    DocMeta, DocModel, DocSetup, FieldRole, FieldUnsupportedReason, Image, Indent, ListInfo,
    PageNumberFormat, PageSetup, ParaProps, Paragraph, ParagraphStyle, Row, Run, SectionBreakKind,
    SectionSetup, SourceRegion, SourceRegionKind, Spacing, Stats, Table, TableBorderColors,
    TableBorderSide, TableBorderSizes, TableBorderStyle, TableBorderStyles, TextDirection, VCell,
    VertAlign, WebExtensionTaskPane,
};
#[cfg(feature = "render")]
pub use render::LayoutPages;
pub use report::{
    DocumentFormat, DocumentReport, DocumentWarning, EditCapability, EditReadOnlyReason,
    FeatureInventory, FieldEvaluationReason, FieldEvaluationReasonCount, FieldKindCount,
    MetafileFormat, MetafileInfo, RenderReport, RenderWarning, RenderedPdf,
};

use fib::Fib;

/// Convenience: decode `.doc` bytes into normalized plain text (all
/// sub-documents — main body, then footnotes/endnotes/headers). Errors with
/// [`Error::NoText`] if nothing indexable was found.
pub fn extract_text(bytes: &[u8]) -> Result<String> {
    let doc = Document::open(bytes)?;
    let t = doc.text();
    if text::has_indexable(&t) {
        Ok(t)
    } else {
        Err(Error::NoText)
    }
}

/// Serialize a [`DocModel`] — one you built from data, built with [`DocBuilder`],
/// or read from a `.doc`/`.docx` — to a clean, Office-openable **`.docx`** byte
/// buffer. This is the authoring entry point: construct a model (paragraphs/runs
/// with fonts, sizes, colors; headings; styled/sized/shaded tables; images; page
/// setup) and write a styled Word document. Available with the default `docx`
/// feature.
///
/// Hyperlink targets beginning with `#` are emitted as internal bookmark anchors
/// without relationships when the bookmark name is referenceable. Malformed
/// internal targets remain visible plain text.
///
/// **Image bytes are trusted as-is:** an embedded [`Image`]'s `bytes` are written
/// verbatim under a part typed from its `mime` — the writer does not transcode or
/// validate the raster, so the caller must ensure `bytes` really are that format (a
/// mismatch produces a part Word can't render). The element-tree editor's
/// [`Document::add_image_png`] / [`Document::replace_image_png`] and
/// JPEG/GIF/BMP/TIFF/WebP counterparts do validate, since they accept arbitrary
/// caller input.
#[cfg(feature = "docx")]
pub fn write_docx(model: &DocModel) -> Vec<u8> {
    write::to_docx(model)
}

/// Fallible variant of [`write_docx`]: returns the serialization error instead of
/// an empty buffer if packaging ever fails. Prefer this when you need to surface
/// write failures rather than silently emit nothing.
#[cfg(feature = "docx")]
pub fn try_write_docx(model: &DocModel) -> Result<Vec<u8>> {
    write::try_to_docx(model)
}

/// Render a [`DocModel`] — one you built from data, or read from a `.doc`/`.docx`
/// — to a **PDF** with native typesetting (`parley` + `krilla`).
/// The rendering entry point for previews and generated reports: rich text
/// (color/size/font), lists, indentation, bordered tables with shaded cells, and
/// images. Available with the `render` feature.
#[cfg(feature = "render")]
pub fn render_pdf(model: &DocModel) -> Vec<u8> {
    render::to_pdf(model)
}

/// Fallible variant of [`render_pdf`]: returns PDF serialization errors instead
/// of collapsing them to an empty byte buffer. Available with the `render`
/// feature.
#[cfg(feature = "render")]
pub fn try_render_pdf(model: &DocModel) -> Result<Vec<u8>> {
    render::try_to_pdf(model)
}

/// Render a [`DocModel`] to PDF after registering caller-supplied fonts (e.g. a
/// bundled Korean face via `include_bytes!`). Use this in headless/server
/// environments that lack system CJK fonts: each blob is added to the layout font
/// collection, made available by its family name and used for script fallback.
/// Available with the `render` feature.
#[cfg(feature = "render")]
pub fn render_pdf_with_fonts(model: &DocModel, fonts: &[Vec<u8>]) -> Vec<u8> {
    render::to_pdf_with_fonts(model, fonts)
}

/// Fallible variant of [`render_pdf_with_fonts`]. Available with the `render`
/// feature.
#[cfg(feature = "render")]
pub fn try_render_pdf_with_fonts(model: &DocModel, fonts: &[Vec<u8>]) -> Result<Vec<u8>> {
    render::try_to_pdf_with_fonts(model, fonts)
}

/// Return layout-derived page numbers from rwml's preview-grade pagination.
///
/// This matches rwml's own PDF output, not Microsoft Word's pagination. Page
/// indices are physical, 1-based page numbers; section page-number restarts and
/// formats are intentionally not applied. The supplied fonts are used strictly:
/// system fonts are disabled and only successfully registered caller bytes are
/// considered. Available with the `render` feature.
#[cfg(feature = "render")]
pub fn layout_pages_with_fonts(model: &DocModel, fonts: &[Vec<u8>]) -> Result<LayoutPages> {
    render::layout_pages_with_fonts(model, fonts)
}

/// Render a [`DocModel`] to PDF with rwml's bundled Noto Sans subsets registered
/// first. The bundled faces cover KS X 1001 Hangul, 4,885 of 4,888 KS X 1001
/// hanja, common Arabic and Hebrew ranges, and Basic Latin. Other scripts fall
/// back to system fonts exactly like [`render_pdf_with_fonts`]. Available with
/// the `render` and `bundled-fonts` features.
#[cfg(all(feature = "render", feature = "bundled-fonts"))]
pub fn render_pdf_bundled(model: &DocModel) -> Vec<u8> {
    let fonts = bundled_render_fonts();
    render_pdf_with_fonts(model, &fonts)
}

/// Fallible variant of [`render_pdf_bundled`]. Available with the `render` and
/// `bundled-fonts` features.
#[cfg(all(feature = "render", feature = "bundled-fonts"))]
pub fn try_render_pdf_bundled(model: &DocModel) -> Result<Vec<u8>> {
    let fonts = bundled_render_fonts();
    try_render_pdf_with_fonts(model, &fonts)
}

#[cfg(all(feature = "render", feature = "bundled-fonts"))]
fn bundled_render_fonts() -> [Vec<u8>; 3] {
    [
        rwml_fonts::noto_sans_kr_subset_with_hanja().to_vec(),
        rwml_fonts::noto_sans_arabic_subset().to_vec(),
        rwml_fonts::noto_sans_hebrew_subset().to_vec(),
    ]
}

/// Render a [`DocModel`] to PDF and return renderer metrics/warnings produced by
/// the same pagination pass. Available with the `render` feature.
#[cfg(feature = "render")]
pub fn render_pdf_with_report(model: &DocModel) -> RenderedPdf {
    render_pdf_with_fonts_and_report(model, &[])
}

/// Fallible variant of [`render_pdf_with_report`]. Available with the `render`
/// feature.
#[cfg(feature = "render")]
pub fn try_render_pdf_with_report(model: &DocModel) -> Result<RenderedPdf> {
    try_render_pdf_with_fonts_and_report(model, &[])
}

/// Render a [`DocModel`] to PDF with caller-supplied fonts and return renderer
/// metrics/warnings produced by the same pagination pass. Available with the
/// `render` feature.
#[cfg(feature = "render")]
pub fn render_pdf_with_fonts_and_report(model: &DocModel, fonts: &[Vec<u8>]) -> RenderedPdf {
    let features = report::render_inventory_for_model(&model.blocks);
    render::to_pdf_with_fonts_and_report(model, fonts, features)
}

/// Fallible variant of [`render_pdf_with_fonts_and_report`]. Available with the
/// `render` feature.
#[cfg(feature = "render")]
pub fn try_render_pdf_with_fonts_and_report(
    model: &DocModel,
    fonts: &[Vec<u8>],
) -> Result<RenderedPdf> {
    let features = report::render_inventory_for_model(&model.blocks);
    render::try_to_pdf_with_fonts_and_report(model, fonts, features)
}

/// A parsed Word document — either legacy `.doc` (OLE2/[MS-DOC]) or modern
/// `.docx` (OOXML). [`Document::open`] format-detects from the magic bytes and
/// both backends feed the **same** [`DocModel`] and exporters, so `text()`,
/// `to_markdown()`, `to_html()`, and `images()` behave identically regardless of
/// which Word format the bytes are in.
pub struct Document {
    backend: Backend,
    #[cfg(feature = "docx")]
    edit_session_active: bool,
}

/// A package-level transaction over an editable `.docx` [`Document`].
///
/// Existing `Document` edit methods are available through this guard via
/// `DerefMut`. Call [`EditSession::commit`] to retain all staged mutations and
/// rebuild the document's read views from the committed package.
/// Calling [`EditSession::rollback`], dropping the guard, or unwinding through
/// its scope restores the complete package snapshot from session creation,
/// including any touched-part state that existed before the session.
///
/// An individual edit error does not poison the session because every existing
/// edit method is independently transactional. A caller may handle that error
/// and continue, but a session that is not explicitly committed still rolls
/// back. External side effects, such as bytes already returned by
/// [`Document::save`] and written elsewhere by the caller, are outside this
/// in-memory transaction.
#[cfg(feature = "docx")]
#[must_use = "an edit session rolls back unless commit() is called"]
pub struct EditSession<'a> {
    document: &'a mut Document,
    original_package: Option<opc::Package>,
}

#[cfg(feature = "docx")]
impl std::fmt::Debug for EditSession<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EditSession")
            .field("pending", &self.original_package.is_some())
            .finish_non_exhaustive()
    }
}

#[cfg(feature = "docx")]
impl std::ops::Deref for EditSession<'_> {
    type Target = Document;

    fn deref(&self) -> &Self::Target {
        self.document
    }
}

#[cfg(feature = "docx")]
impl std::ops::DerefMut for EditSession<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.document
    }
}

#[cfg(feature = "docx")]
impl EditSession<'_> {
    /// Validate and retain every mutation staged through this session, then
    /// rebuild the document's model, text, metadata, and side-table read views.
    ///
    /// If final package validation or read-view refresh fails, the session is
    /// dropped and the original package is restored before the error is returned.
    pub fn commit(mut self) -> Result<()> {
        self.document.refresh_read_view_impl()?;
        self.original_package = None;
        Ok(())
    }

    /// Restore the package snapshot captured when this session was created.
    pub fn rollback(mut self) {
        self.restore();
    }

    fn restore(&mut self) {
        let Some(package) = self.original_package.take() else {
            return;
        };
        if let Backend::Docx(state) = &mut self.document.backend {
            state.package = package;
        }
    }
}

#[cfg(feature = "docx")]
impl Drop for EditSession<'_> {
    fn drop(&mut self) {
        self.restore();
        self.document.edit_session_active = false;
    }
}

/// Editable `.docx` core document properties supported by
/// [`Document::set_core_property`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreProperty {
    /// Dublin Core `dc:title`.
    Title,
    /// Dublin Core `dc:subject`.
    Subject,
    /// Dublin Core `dc:creator`.
    Creator,
    /// Dublin Core `dc:description`.
    Description,
    /// Core-properties `cp:keywords`.
    Keywords,
    /// Core-properties `cp:lastModifiedBy`.
    LastModifiedBy,
    /// Core-properties `cp:category`.
    Category,
    /// Core-properties `cp:contentStatus`.
    ContentStatus,
    /// Dublin Core Terms `dcterms:created`.
    Created,
    /// Dublin Core Terms `dcterms:modified`.
    Modified,
    /// Core-properties `cp:lastPrinted`.
    LastPrinted,
    /// Core-properties `cp:revision`.
    Revision,
    /// Core-properties `cp:version`.
    Version,
}

/// Kind of one conservative atomic direct `.docx` body block exposed by
/// [`Document::body_blocks`].
#[cfg(feature = "docx")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BodyBlockKind {
    /// A direct WordprocessingML paragraph (`w:p`).
    Paragraph,
    /// A direct WordprocessingML table (`w:tbl`).
    Table,
    /// A direct block-level content control subtree (`w:sdt`).
    ContentControl,
}

/// Index and kind of one atomic direct `.docx` body block.
///
/// These descriptors intentionally are not persistent source handles. Indices
/// address the current retained package tree and should be enumerated again after
/// a structural edit or save/reopen cycle.
#[cfg(feature = "docx")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct BodyBlockInfo {
    /// Zero-based index used by [`Document::remove_body_block`] and
    /// [`Document::move_body_block`], and the position before which
    /// [`Document::insert_body_paragraph`] inserts.
    pub index: usize,
    /// Direct body element kind.
    pub kind: BodyBlockKind,
}

/// Core document properties extracted from `docProps/core.xml` or generated
/// document setup metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CoreProperties {
    /// Dublin Core `dc:title`.
    pub title: Option<String>,
    /// Dublin Core `dc:subject`.
    pub subject: Option<String>,
    /// Dublin Core `dc:creator`.
    pub creator: Option<String>,
    /// Dublin Core `dc:description`.
    pub description: Option<String>,
    /// Core-properties `cp:keywords`.
    pub keywords: Option<String>,
    /// Core-properties `cp:category`.
    pub category: Option<String>,
    /// Core-properties `cp:contentStatus`.
    pub content_status: Option<String>,
    /// Core-properties `cp:lastModifiedBy`.
    pub last_modified_by: Option<String>,
    /// Dublin Core Terms `dcterms:created`, typically an ISO-8601 timestamp.
    pub created: Option<String>,
    /// Dublin Core Terms `dcterms:modified`, typically an ISO-8601 timestamp.
    pub modified: Option<String>,
    /// Core-properties `cp:lastPrinted`, typically an ISO-8601 timestamp.
    pub last_printed: Option<String>,
    /// Core-properties `cp:revision`.
    pub revision: Option<String>,
    /// Core-properties `cp:version`.
    pub version: Option<String>,
}

impl CoreProperties {
    fn from_doc_setup(setup: &DocSetup) -> Self {
        CoreProperties {
            title: setup.title.clone(),
            subject: setup.subject.clone(),
            creator: setup.creator.clone(),
            description: setup.description.clone(),
            keywords: setup.keywords.clone(),
            category: setup.category.clone(),
            content_status: setup.content_status.clone(),
            last_modified_by: setup.last_modified_by.clone(),
            created: setup.created.clone(),
            modified: setup.modified.clone(),
            last_printed: setup.last_printed.clone(),
            revision: setup.revision.clone(),
            version: setup.version.clone(),
        }
    }
}

#[cfg(feature = "docx")]
impl CoreProperty {
    fn ns(self) -> &'static [u8] {
        match self {
            CoreProperty::Title
            | CoreProperty::Subject
            | CoreProperty::Creator
            | CoreProperty::Description => DC_NS,
            CoreProperty::Keywords
            | CoreProperty::LastModifiedBy
            | CoreProperty::Category
            | CoreProperty::ContentStatus
            | CoreProperty::LastPrinted
            | CoreProperty::Revision
            | CoreProperty::Version => CORE_PROPERTIES_NS,
            CoreProperty::Created | CoreProperty::Modified => DCTERMS_NS,
        }
    }

    fn local(self) -> &'static [u8] {
        match self {
            CoreProperty::Title => b"title",
            CoreProperty::Subject => b"subject",
            CoreProperty::Creator => b"creator",
            CoreProperty::Description => b"description",
            CoreProperty::Keywords => b"keywords",
            CoreProperty::LastModifiedBy => b"lastModifiedBy",
            CoreProperty::Category => b"category",
            CoreProperty::ContentStatus => b"contentStatus",
            CoreProperty::Created => b"created",
            CoreProperty::Modified => b"modified",
            CoreProperty::LastPrinted => b"lastPrinted",
            CoreProperty::Revision => b"revision",
            CoreProperty::Version => b"version",
        }
    }

    fn qname(self) -> &'static str {
        match self {
            CoreProperty::Title => "dc:title",
            CoreProperty::Subject => "dc:subject",
            CoreProperty::Creator => "dc:creator",
            CoreProperty::Description => "dc:description",
            CoreProperty::Keywords => "cp:keywords",
            CoreProperty::LastModifiedBy => "cp:lastModifiedBy",
            CoreProperty::Category => "cp:category",
            CoreProperty::ContentStatus => "cp:contentStatus",
            CoreProperty::Created => "dcterms:created",
            CoreProperty::Modified => "dcterms:modified",
            CoreProperty::LastPrinted => "cp:lastPrinted",
            CoreProperty::Revision => "cp:revision",
            CoreProperty::Version => "cp:version",
        }
    }

    fn fallback_attrs(self) -> &'static [(&'static str, &'static str)] {
        match self {
            CoreProperty::Created | CoreProperty::Modified => DCTERMS_W3CDTF_ATTRS,
            _ => &[],
        }
    }
}

/// The format-specific state behind a [`Document`]. Boxed so the enum isn't
/// dominated by the larger `.doc` variant.
enum Backend {
    Doc(Box<DocState>),
    #[cfg(feature = "docx")]
    Docx(Box<docx::DocxState>),
}

/// Legacy `.doc` state: decoded text plus the FIB and retained structures for
/// the lazy rich-model build.
struct DocState {
    /// Full render with reconstructed list autonumbers (used by `text()`).
    labeled: String,
    fib: Fib,
    // Retained for the lazy rich-model build ([`Document::model`]); none of this
    // is touched by the fast `text()` path.
    word: Vec<u8>,
    table: Vec<u8>,
    pieces: Vec<clx::Piece>,
    papx: papx::PapxTable,
    chpx: chpx::ChpxTable,
    prm1_patches: Vec<Option<chpx::PcdPrm1Patch>>,
    stylesheet: stsh::StyleSheet,
    lists: list::Lists,
    /// Font-name table (`SttbfFfn`), for resolving CHPX font indices to names.
    fonts: Vec<String>,
    /// Legacy comment owner names from `GrpXstAtnOwners`.
    annotation_owners: Vec<String>,
    /// Legacy comment metadata from `PlcfandRef` ATRD records.
    annotation_metadata: Vec<annotation::LegacyDocCommentMetadata>,
    /// The `Data` stream bytes (inline pictures), empty if absent.
    data: Vec<u8>,
    enc: &'static encoding_rs::Encoding,
}

fn doc_model_from_doc_state(state: &DocState) -> DocModel {
    let assemble::LegacyBuildOutput {
        model,
        #[cfg(feature = "docx")]
            promoted_running_block_ranges: _promoted_running_block_ranges,
        pagination_hints: _pagination_hints,
        line_spacing_hints: _line_spacing_hints,
        #[cfg(any(feature = "docx", feature = "render"))]
            tab_stops: _tab_stops,
        #[cfg(feature = "render")]
            render_tab_stops: _render_tab_stops,
        #[cfg(feature = "docx")]
            note_reference_anchors: _note_reference_anchors,
        column_break_offsets: _column_break_offsets,
        #[cfg(feature = "docx")]
            table_cell_column_break_offsets: _table_cell_column_break_offsets,
        section_column_gap_pt: _section_column_gap_pt,
        final_section_column_gap_pt: _final_section_column_gap_pt,
        section_column_layouts: _section_column_layouts,
        final_section_column_layout: _final_section_column_layout,
        section_column_separators: _section_column_separators,
        final_section_column_separator: _final_section_column_separator,
        section_column_rtl: _section_column_rtl,
        final_section_column_rtl: _final_section_column_rtl,
        table_row_pagination: _table_row_pagination,
        table_cell_pagination: _table_cell_pagination,
        table_cell_line_spacing: _table_cell_line_spacing,
        #[cfg(any(feature = "docx", feature = "render"))]
            table_cell_tab_stops: _table_cell_tab_stops,
        #[cfg(feature = "render")]
            render_table_cell_tab_stops: _render_table_cell_tab_stops,
        #[cfg(any(feature = "docx", feature = "render"))]
            running_line_spacing_hints: _running_line_spacing_hints,
        #[cfg(feature = "docx")]
            running_pagination_hints: _running_pagination_hints,
        #[cfg(feature = "docx")]
            running_column_break_offsets: _running_column_break_offsets,
        #[cfg(any(feature = "docx", feature = "render"))]
            running_tab_stops: _running_tab_stops,
        #[cfg(any(feature = "docx", feature = "render"))]
            running_table_cell_tab_stops: _running_table_cell_tab_stops,
        running_surface_distances: _running_surface_distances,
    } = legacy_build_output_from_doc_state(state);
    model
}

fn legacy_build_output_from_doc_state(state: &DocState) -> assemble::LegacyBuildOutput {
    let mut numberer = list::Numberer::new(&state.lists);
    assemble::build_model_with_render_hints(
        assemble::BuildInputs {
            word: &state.word,
            table: &state.table,
            pieces: &state.pieces,
            enc: state.enc,
            papx: &state.papx,
            chpx: &state.chpx,
            prm1_patches: &state.prm1_patches,
            stylesheet: &state.stylesheet,
            data: &state.data,
            fonts: &state.fonts,
            fib: &state.fib,
        },
        &mut numberer,
    )
}

impl Document {
    /// Open and decode a Word document from its raw bytes, detecting the format:
    /// the OLE2/CFB magic (`D0CF11E0`) routes to the legacy `.doc` parser, the
    /// ZIP magic (`PK\x03\x04`) to the `.docx` parser (when the `docx` feature is
    /// enabled). Neither ⇒ [`Error::NotOle2`].
    pub fn open(bytes: &[u8]) -> Result<Self> {
        if ole::is_ole2(bytes) {
            return Ok(Document {
                backend: Backend::Doc(Box::new(DocState::open(bytes)?)),
                #[cfg(feature = "docx")]
                edit_session_active: false,
            });
        }
        #[cfg(feature = "docx")]
        if docx::is_zip(bytes) {
            return Ok(Document {
                backend: Backend::Docx(Box::new(docx::open(bytes)?)),
                edit_session_active: false,
            });
        }
        #[cfg(not(feature = "docx"))]
        if bytes.starts_with(b"PK\x03\x04") {
            return Err(Error::Docx(
                "`.docx` support not compiled in (enable the `docx` cargo feature)".into(),
            ));
        }
        Err(Error::NotOle2)
    }

    /// Create a new, empty `.docx`-backed document from the bundled blank template
    /// (one empty paragraph, default page setup) — mirroring how python-docx's
    /// `Document()` opens its `default.docx`. The returned document carries a full,
    /// valid OPC package, so [`Document::save`] produces an Office-openable file.
    ///
    /// Panics only if the crate's own bundled template is corrupt (a build-time
    /// invariant covered by tests); use [`Document::try_new`] for a non-panicking
    /// variant. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn new() -> Self {
        Document {
            backend: Backend::Docx(Box::new(docx::blank())),
            edit_session_active: false,
        }
    }

    /// Fallible [`Document::new`]: returns an error instead of panicking if the
    /// bundled blank template can't be opened. Available with the default `docx`
    /// feature.
    #[cfg(feature = "docx")]
    pub fn try_new() -> Result<Self> {
        Ok(Document {
            backend: Backend::Docx(Box::new(docx::try_blank()?)),
            edit_session_active: false,
        })
    }

    /// Build the rich document model — paragraphs, character runs (bold/italic/
    /// …), structured tables, lists, and fields. For `.doc` this is built lazily
    /// (the flat [`Document::text`] path never pays for it); for `.docx` the model
    /// is built eagerly at open and cloned here.
    ///
    /// **Stale after an in-place edit.** This (and everything derived from it —
    /// [`Document::to_markdown`], [`Document::to_html`], [`Document::images`],
    /// [`Document::to_docx`], `Document::to_pdf`) reflects the current parsed read view.
    /// Preservation edits ([`Document::replace_body_text`], [`Document::set_field_result`],
    /// [`Document::fill_content_control_by_tag`], [`Document::fill_content_controls_by_tag`],
    /// [`Document::fill_template_fields`],
    /// [`Document::accept_all_revisions`], [`Document::reject_all_revisions`],
    /// [`Document::insert_body_paragraph`], [`Document::remove_body_block`],
    /// [`Document::move_body_block`],
    /// [`Document::set_hyperlink_target`], [`Document::add_image_png`],
    /// [`Document::replace_image_png`]) mutate the package
    /// directly, not this model, so they are not visible here until
    /// [`Document::refresh_read_view`] runs or you [`Document::save`] and
    /// re-[`Document::open`] the result. [`EditSession::commit`] refreshes the read
    /// view automatically.
    pub fn model(&self) -> DocModel {
        match &self.backend {
            Backend::Doc(d) => doc_model_from_doc_state(d),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => {
                // The stored model is body-only; re-append footnote/endnote blocks for
                // the read/render view (they are kept separate because their parts are
                // preserved verbatim on save, never inlined into document.xml).
                let mut m = d.model.clone();
                m.blocks.extend(d.notes.iter().cloned());
                m
            }
        }
    }

    /// Render the document as GitHub-Flavored **Markdown** (headings, bold/italic,
    /// lists, hyperlinks, and tables).
    pub fn to_markdown(&self) -> String {
        export::markdown::render(&self.model())
    }

    /// Render the document as semantic **HTML** (`<h1>`–`<h6>`, `<strong>`,
    /// `<table>` with `colspan`/`rowspan`, nested `<ol>`/`<ul>`, `<a href>`).
    pub fn to_html(&self) -> String {
        export::html::render(&self.model())
    }

    /// Extract every embedded raster image (PNG/JPEG/GIF) with its bytes, in
    /// reading order — the equivalent of POI's `PicturesTable.getAllPictures()`.
    pub fn images(&self) -> Vec<Image> {
        fn walk(blocks: &[Block], out: &mut Vec<Image>) {
            for b in blocks {
                match b {
                    Block::Paragraph(p) => {
                        for r in &p.runs {
                            if let Some(img) = &r.image {
                                if img.bytes.is_some() {
                                    out.push(img.clone());
                                }
                            }
                        }
                    }
                    Block::Image(img) if img.bytes.is_some() => out.push(img.clone()),
                    Block::Chart(_) | Block::PageBreak | Block::SectionBreak(_) => {}
                    Block::Table(t) => {
                        for row in &t.rows {
                            for c in &row.cells {
                                walk(&c.blocks, out);
                            }
                        }
                    }
                    Block::Image(_) => {}
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.model().blocks, &mut out);
        out
    }

    /// Return whether package-preserving edit APIs are available for this opened
    /// document, with typed read-only reasons when they are not.
    ///
    /// This is the non-mutating counterpart to the edit APIs' own preflight
    /// checks: `.doc` sources, incomplete retained packages, and lossy OPC
    /// metadata are reported here before a caller attempts
    /// [`Document::replace_body_text`], [`Document::add_image_png`], or related
    /// preservation edits.
    pub fn edit_capability(&self) -> EditCapability {
        match &self.backend {
            Backend::Doc(_) => report::doc_edit_capability(),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => report::docx_edit_capability(d),
        }
    }

    /// Return package part names touched by preservation edits since this
    /// document was opened or created.
    ///
    /// The list is sorted, has no leading slash, and reflects the retained OPC
    /// package's authoritative dirty set: edited XML parts, replaced media parts,
    /// regenerated relationship parts, and regenerated `[Content_Types].xml` all
    /// appear when an edit dirties them. A freshly opened package returns an
    /// empty list. Legacy `.doc` documents are read-only for preservation edits
    /// and return an empty list.
    pub fn edited_parts(&self) -> Vec<String> {
        match &self.backend {
            Backend::Doc(_) => Vec::new(),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => d.package.touched_parts(),
        }
    }

    /// Return a machine-readable summary of the document's source format,
    /// visible model statistics, observed Word feature markers, and warnings.
    ///
    /// Feature counts are conservative: they mean rwml observed markers for a
    /// construct, not that every behavior of that construct is fully modeled,
    /// editable, or renderable.
    pub fn report(&self) -> DocumentReport {
        match &self.backend {
            Backend::Doc(d) => {
                let model = self.model();
                let features = report::feature_inventory_for_model(&model.blocks);
                let edit = self.edit_capability();
                let mut warnings = report::warnings_for(&features, &edit);
                if let Some(warning) = report::legacy_doc_flattened_subdocuments_warning(
                    d.fib.ccp_ftn as usize,
                    d.fib.ccp_hdd as usize,
                    d.fib.ccp_atn as usize,
                    d.fib.ccp_edn as usize,
                    d.fib.ccp_txbx as usize,
                ) {
                    warnings.push(warning);
                }
                DocumentReport {
                    format: DocumentFormat::Doc,
                    stats: model.meta.stats,
                    core_properties: CoreProperties::from_doc_setup(&model.setup),
                    custom_properties: Default::default(),
                    edit,
                    edited_parts: Vec::new(),
                    features,
                    warnings,
                }
            }
            #[cfg(feature = "docx")]
            Backend::Docx(d) => {
                let features = report::docx_features(d);
                let edit = self.edit_capability();
                let edited_parts = self.edited_parts();
                let warnings = report::warnings_for(&features, &edit);
                DocumentReport {
                    format: DocumentFormat::Docx,
                    stats: d.model.meta.stats,
                    core_properties: d.core_properties.clone(),
                    custom_properties: d.model.custom_properties.clone(),
                    edit,
                    edited_parts,
                    features,
                    warnings,
                }
            }
        }
    }

    /// Extract core document metadata.
    ///
    /// For `.docx`, this reads `docProps/core.xml` when present and returns the
    /// supported Dublin Core/core-properties fields. For model-backed legacy
    /// documents, this surfaces the title and creator metadata available through
    /// [`DocSetup`].
    pub fn core_properties(&self) -> CoreProperties {
        match &self.backend {
            Backend::Doc(_) => CoreProperties::from_doc_setup(&self.model().setup),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => d.core_properties.clone(),
        }
    }

    /// Extract comments from a `.docx` comments part or recoverable legacy
    /// `.doc` annotation subdocument.
    ///
    /// The returned comments are a side table. `.docx` comments include
    /// metadata and body/note/header/footer anchors when present; legacy `.doc`
    /// annotation regions expose stable synthetic ids, visible comment text,
    /// recovered author/initials metadata when the annotation tables are
    /// present, and best-effort source-region anchors.
    pub fn comments(&self) -> Vec<Comment> {
        match &self.backend {
            Backend::Doc(d) => legacy_doc_comments_from_state(d),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => d.comments.clone(),
        }
    }

    /// Extract recovered footnote/endnote records where rwml has a semantic note
    /// side table.
    ///
    /// Legacy `.doc` notes are recovered from exact FIB footnote/endnote
    /// subdocument regions with synthetic ids, visible note text, and
    /// exact body-text anchors from `PlcffndRef`/`PlcfendRef` when those tables
    /// match the recovered note records. Malformed or mismatched tables keep
    /// source-region anchors, and documents without usable tables retain the
    /// single-marker body-anchor fallback. `.docx` notes are recovered from
    /// `word/footnotes.xml` and `word/endnotes.xml` with their Word ids, note
    /// kind, visible text, and reference id anchors when the body references
    /// them.
    pub fn notes(&self) -> Vec<Note> {
        match &self.backend {
            Backend::Doc(d) => legacy_doc_notes_from_state(d),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => d.note_records.clone(),
        }
    }

    /// Extract recovered text-box records where rwml has a semantic text-box
    /// side table.
    ///
    /// Legacy `.doc` text boxes are recovered from exact FIB text-box
    /// subdocument regions with synthetic ids, visible text, and exact body-text
    /// anchors from `PlcSpaMom` when the SPA count matches the recovered
    /// text-box records. Malformed or mismatched shape tables keep
    /// source-region anchors. `.docx` text boxes are recovered from
    /// body/note/header/footer `w:txbxContent` shapes with synthetic ids and
    /// visible text.
    pub fn text_boxes(&self) -> Vec<TextBox> {
        match &self.backend {
            Backend::Doc(d) => legacy_doc_text_boxes_from_state(d),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => d.text_boxes.clone(),
        }
    }

    /// Extract recovered floating-shape geometry records.
    ///
    /// `.docx` records are recovered from body/note/header/footer `wp:anchor` drawing
    /// markup with `wp:extent`, `wp:docPr`, and simple `wp:positionH`/
    /// `wp:positionV` metadata when present. Legacy `.doc` floating shape
    /// geometry is not decoded yet and returns an empty side table.
    pub fn floating_shapes(&self) -> Vec<FloatingShape> {
        match &self.backend {
            Backend::Doc(_) => Vec::new(),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => d.floating_shapes.clone(),
        }
    }

    /// Extract recovered running header/footer records.
    ///
    /// `.docx` records use the referenced package part plus `default`, `first`,
    /// or `even` reference type as stable ids, and distinguish default, even-page,
    /// and first-page header/footer variants where present. Legacy `.doc` records
    /// are recovered from the combined FIB header/footer subdocument region with
    /// synthetic ids, using `PlcfHdd` story indexes for exact even/odd/first-page
    /// variants when available.
    pub fn header_footers(&self) -> Vec<HeaderFooter> {
        match &self.backend {
            Backend::Doc(_) => legacy_doc_header_footers_from_model(&self.model()),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => d.header_footers.clone(),
        }
    }

    /// Extract recovered field records.
    ///
    /// For `.docx`, the returned side table includes body, note, and modeled
    /// header/footer simple fields and common complex fields with their
    /// normalized instruction text and cached visible result. For legacy `.doc`,
    /// fields are reconstructed from the rich model's field-marked result runs
    /// where the binary field instruction was recoverable.
    pub fn fields(&self) -> Vec<Field> {
        match &self.backend {
            Backend::Doc(_) => report::fields_for_model(&self.model().blocks),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => d.fields.clone(),
        }
    }

    /// Extract the editable cached fields in one explicit `.docx`
    /// WordprocessingML story part.
    ///
    /// Supported parts are `word/document.xml`, the standard
    /// `word/footnotes.xml` and `word/endnotes.xml` parts, and existing header or
    /// footer parts with the corresponding OOXML content type. Header/footer
    /// parts do not need to be referenced by the current section graph. Note
    /// separator entries, rejected revision content, and untaken
    /// `mc:AlternateContent` branches are excluded.
    ///
    /// The returned zero-based order is the order accepted by
    /// [`Document::set_field_result_in_part`]. It inventories the live package,
    /// including staged edits, and exposes cached results only:
    /// [`Field::computed_result`] is always `None`. The document's general read
    /// views remain unchanged until explicitly refreshed or reopened.
    #[cfg(feature = "docx")]
    pub fn fields_in_part(&self, part_name: &str) -> Result<Vec<Field>> {
        let d = self.docx_tree_editable_ref()?;
        let target = explicit_field_story_target(&d.package, part_name, "fields_in_part")?;
        field_story_inventory(&d.package, &target, "fields_in_part")
    }

    /// Extract fields like [`Document::fields`], additionally computing
    /// volatile fields the caller-supplied [`FieldContext`] covers (`DATE`/
    /// `TIME` with an explicit `\@` picture, `USERNAME`/`USERINITIALS`/
    /// `USERADDRESS` without literal overrides).
    ///
    /// Results are deterministic given the same document and context: the
    /// context values are inputs. Fields the default evaluation already
    /// computes keep their document-derived results, and fields the context
    /// does not cover keep cached text only.
    #[cfg(feature = "docx")]
    pub fn fields_with_context(&self, context: &FieldContext) -> Vec<Field> {
        let mut fields = self.fields();
        docx::fields::apply_context_results(&mut fields, context);
        fields
    }

    /// Extract tracked revisions from `.docx` body/note/header/footer content.
    ///
    /// The returned side table includes insertion, deletion, and move markers
    /// with metadata and visible subtree text. Legacy `.doc` revisions are not
    /// exposed through this API yet.
    pub fn revisions(&self) -> Vec<Revision> {
        match &self.backend {
            Backend::Doc(_) => Vec::new(),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => d.revisions.clone(),
        }
    }

    /// Normalized main-body text under a tracked-revision view policy.
    ///
    /// For `.docx`, [`RevisionView::Accepted`] includes insertions and move
    /// destinations, [`RevisionView::Original`] includes deletions and move
    /// sources, and [`RevisionView::Annotated`] emits compact textual markers
    /// for both sides. Legacy `.doc` revision views are not modeled yet and
    /// return [`Document::main_text`].
    pub fn main_text_with_revision_view(&self, view: RevisionView) -> String {
        let _ = view;
        match &self.backend {
            Backend::Doc(_) => self.main_text(),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => docx::main_text_with_revision_view(d, view),
        }
    }

    /// Serialize this document to a modern **`.docx`** (OOXML WordprocessingML)
    /// byte buffer — the inverse of the reader. `read → DocModel → write → read`
    /// preserves the structure (text, character runs, headings, alignment, lists,
    /// tables with colspan/rowspan, images, hyperlinks), so a legacy `.doc` can be
    /// converted to a clean, Office-openable `.docx` through the shared model.
    /// Opened DOCX inputs also retain every supported nonempty core property in
    /// the public model and regenerated `docProps/core.xml`: title, subject,
    /// creator, description, keywords, category, content status,
    /// last-modified-by, created/modified/last-printed timestamps, revision, and
    /// version.
    /// Opened legacy and DOCX documents also carry validated source-only section
    /// column gaps, complete unequal geometry, separator flags, right-to-left
    /// population, and running header/footer distances into the generated
    /// package. Opened legacy and DOCX documents additionally carry
    /// exact/minimum line rules and effective keep/widow pagination controls for
    /// aligned top-level body paragraphs and direct paragraph blocks in surviving
    /// cells of aligned top-level tables, plus effective no-split state for
    /// aligned top-level table rows. That direct body subset also carries
    /// reader-resolved explicit paragraph tab stops, while aligned top-level body
    /// paragraphs and aligned direct paragraph blocks in surviving cells of top-
    /// level body tables carry visible manual column breaks through independently
    /// validated source character offsets. Opened DOCX inputs additionally carry
    /// row no-split state, direct cell-paragraph keep/widow controls,
    /// exact/minimum line rules, explicit tabs, and visible column breaks
    /// recursively through nested descendants of those body tables; each
    /// recursive component is validated independently. Direct top-level
    /// paragraphs in selected default/first/even running headers and footers from
    /// opened DOCX and legacy DOC inputs also carry reader-resolved explicit tab
    /// stops through section-aligned private hints. Direct paragraph blocks in
    /// top-level tables on those running surfaces use a companion block/row/
    /// surviving-cell/paragraph-aligned bridge for explicit tabs. Opened DOCX and
    /// legacy DOC inputs both
    /// retain reader-resolved exact/minimum line rules on those direct table-cell
    /// paragraphs and on direct top-level running paragraphs through section-
    /// aligned source-only hints. Those same selected running surfaces retain
    /// effective `keepNext`, `keepLines`, and widow-off state on direct top-level
    /// paragraphs and direct paragraphs in surviving cells of top-level tables,
    /// plus effective no-split state for aligned top-level table rows. Opened DOCX
    /// and legacy DOC inputs also carry visible manual column breaks in ordinary
    /// top-level paragraphs across those six running variants through a separately
    /// validated section/variant/paragraph-aligned bridge. Legacy source-story
    /// offsets follow the same six-slot section ownership used by the other
    /// running-surface hints; opened DOCX hints follow default-surface inheritance
    /// and keep local relationships. Opened DOCX inputs carry visible manual
    /// column breaks in direct paragraphs of top-level running-table cells
    /// and recursively carry row no-split state,
    /// cell-paragraph keep/widow controls, exact/minimum line rules, explicit
    /// tabs, and visible column breaks through nested running-table descendants
    /// across all six variants. The private tree follows default-surface
    /// inheritance, validates each table slot and recursive component
    /// independently, and keeps header/footer-local relationships. Fresh
    /// legacy conversion excludes an exact nonempty HeaderFooter source region
    /// from the generated body only when assembly actually promoted that range
    /// into a selected section running slot; unselected running stories and all
    /// other unsupported subdocuments retain their flattened fallback. Opened
    /// legacy DOC inputs with exact nonempty
    /// `PlcffndRef`/`PlcfendRef` tables, one-to-one Main-story markers, and
    /// ordinary top-level paragraph anchors promote normalized footnote/endnote
    /// text into real references and note parts. Reopened DOCX inputs retain that
    /// exact body-reference subset through a private block/id/offset bridge and
    /// also retain mixed ordinary paragraphs and supported table trees
    /// rooted at top-level tables. Surviving cells may recursively contain
    /// ordinary paragraphs or further tables under the same constraint. The
    /// private payload preserves modeled table properties and merges,
    /// paragraph/run formatting, row no-split and cell keep/widow controls,
    /// exact/minimum line rules, explicit tabs, and visible manual column breaks
    /// at every table depth. Manual page breaks between top-level paragraph
    /// fragments and inside table cells at every selected depth are also retained.
    /// Relationship-backed external hyperlinks in those paragraphs retain
    /// relationships owned by the corresponding footnote or endnote part,
    /// including through nested tables. Simple HYPERLINK field syntax may
    /// normalize to a `w:hyperlink` element. Validator-approved internal-anchor
    /// links and run bookmarks likewise survive at every supported depth without
    /// note relationships, using paired bookmark IDs unique across body and note
    /// stories.
    /// Parser-modeled run content controls with a nonempty alias or tag, or a
    /// complete XPath/store-item data-binding pair, likewise survive at every
    /// supported depth with trimmed metadata and no additional relationships.
    /// A one-sided binding rejects that note atomically; property-empty source
    /// wrappers are normalized out by the reader and are not claimed.
    /// Parser-normalized cached result runs for nonempty compatibility/private,
    /// inserted-content, mail-merge-helper, and barcode fields reported as
    /// `NoComputedResult` likewise survive at every supported depth as non-dirty
    /// `w:fldSimple` fields with normalized instructions and modeled formatting.
    /// Complex source fields normalize to the same form; source simple/complex
    /// grouping and source update state are not preserved. Parser-validated,
    /// non-dirty `MERGEFIELD` cached results with valid names and bounded before/
    /// after and text-format switches also survive as normalized simple fields;
    /// no external merge data source is copied or evaluated. Parser-validated,
    /// non-dirty `FILENAME` cached results with bounded path-display and text-
    /// format switches likewise survive as normalized simple fields; no
    /// filesystem path is read and no destination filename is inferred.
    /// Parser-validated, non-dirty `RD`, `TA`, `XE`, and `TC` marker instructions
    /// also survive in source order as empty simple fields with modeled formatting
    /// while stale marker cache text stays hidden. This path transports markers
    /// only; it does not generate or update indexes, tables of authorities, tables
    /// of contents, or master-document references. Marker-only notes and generated
    /// `BIBLIOGRAPHY`/`CITATION`/`INDEX`/`TOA`/`TOC` fields remain excluded. Dirty
    /// modeled runs, malformed field syntax, and unknown or other context-
    /// sensitive/generated fields reject that note atomically.
    /// Parser-validated, non-dirty `SYMBOL` instructions that actually compute
    /// one character likewise survive as normalized simple fields with modeled
    /// formatting; stale cached characters stay replaced by the deterministic
    /// result. Malformed or unmapped symbols remain excluded. Context-free, non-
    /// dirty `EQ` and `ADVANCE` instructions with one modeled result run likewise
    /// survive. Supported EQ expressions retain deterministic plain-text
    /// approximations and modeled formatting; parser-valid ADVANCE instructions
    /// retain an empty current result and any modeled result-run formatting.
    /// Malformed or unsupported display syntax, split results, and action fields
    /// remain excluded. This does not add native PDF equation layout, ADVANCE
    /// displacement, or action execution.
    /// Parser-validated, non-dirty literal `QUOTE` instructions with one modeled
    /// result run likewise survive as normalized simple fields with computed
    /// text, supported text-format switches, and modeled formatting. Malformed
    /// or split-result `QUOTE` fields and other dynamic fields remain excluded.
    /// Context-free, non-dirty `IF` and `COMPARE` instructions with one modeled
    /// result run likewise survive when quoted-text or finite-number operands
    /// compute without bookmark or `SET` state, including compact comparisons
    /// and wildcard equality. Bookmark- or `SET`-backed, malformed or nonfinite,
    /// and split-result comparisons remain excluded.
    /// Context-free, non-dirty formula (`=`) instructions with one modeled result
    /// run likewise survive when finite literal arithmetic or scalar-function
    /// expressions compute without bookmark, `SET`, table-cell, or positional
    /// context and do not use `DEFINED`; supported numeric pictures, computed text,
    /// and modeled formatting are retained. Stateful or table-dependent,
    /// `DEFINED`, malformed or nonfinite, and split-result formulas remain
    /// excluded.
    /// Parser-validated, non-dirty `FILLIN` instructions with an explicit literal
    /// default and one modeled result run likewise retain quoted or bounded multi-
    /// token defaults, neutral `\o`, supported text formatting, and deterministic
    /// current results. This does not add interactive prompting or field updates;
    /// default-less or malformed prompts, split results, and stateful `ASK`, `SET`,
    /// and merge-control fields remain excluded.
    /// Parser-validated, non-dirty core-property and custom-string `DOCPROPERTY`
    /// fields with one modeled result run likewise survive when the referenced
    /// property exists and each core value already matches the fresh writer's
    /// nonempty trim normalization. Direct core-property and `INFO` aliases,
    /// supported text/date pictures, deterministic current results, and modeled
    /// formatting are retained. Extended application properties, document
    /// variables, `FILESIZE`, volatile date/user fields, `REVNUM`, malformed or
    /// dirty instructions, and split results remain excluded. This path uses the
    /// core/custom properties already carried by the fresh model; it does not
    /// copy app/settings parts or add field updates, user context, relationships,
    /// or layout behavior.
    /// Parser-validated, non-dirty `REVNUM` fields with one modeled result run
    /// likewise survive when the core revision exists and already matches the
    /// fresh writer's nonempty trim normalization. Normalized instructions,
    /// deterministic current results, supported text-format switches, and modeled
    /// formatting are retained. Missing, empty, trim-unstable, malformed, dirty,
    /// or split-result cases remain excluded. This does not add revision tracking,
    /// field updates, relationships, or layout behavior.
    /// Parser-validated, non-dirty explicit bookmark-text `REF` fields with one
    /// modeled result run likewise survive when the target is exactly one stable,
    /// nonempty, non-field run in the same note payload and the modeled result
    /// matches a fresh evaluation of that text. Neutral hyperlink/lock switches,
    /// supported text and integer number formats, numeric pictures, target
    /// bookmarks, and modeled formatting are retained. Note-mark, relative-
    /// position, paragraph/full/relative-context numbering, sequence-separator,
    /// cross-note, missing, empty, multi-run, field-backed, malformed, dirty,
    /// mismatched-result, or split-result cases remain excluded. This does not add
    /// global bookmark lookup, field updates, relationships, or layout behavior.
    /// Complete nonempty extracted PNG, JPEG, GIF, BMP, TIFF, and WebP inline
    /// runs also retain globally unique media parts and relationships owned by
    /// the corresponding note part, including through nested tables and under
    /// an external hyperlink.
    /// Parser-modeled native literal-cache core and ChartEx chart blocks in
    /// chart-only sibling paragraphs retain note-local relationships, globally
    /// unique chart/drawing IDs, and fresh chart package parts at the top level
    /// or through nested tables. Core chart data receives a regenerated embedded
    /// XLSX workbook; ChartEx literal data remains native.
    /// An unsupported opened-DOCX note body falls back independently to normalized
    /// one-paragraph text.
    /// Legacy nested-table descendants and note paragraph layout properties
    /// remain outside these layout-hint paths. Legacy manual breaks in running-
    /// table cells and nested running tables remain unsupported. Settings-defined
    /// default-tab intervals and legacy note manual breaks also remain outside the
    /// bounded paths. Opened DOCX typed manual page breaks in direct and
    /// recursively nested body and running-surface table cells survive fresh
    /// conversion through the ordered public block tree across all six selected
    /// running variants; preview table fragmentation and Word-exact pagination
    /// are not claimed. Missing/empty, unknown-MIME, raw-RGBA, floating, block,
    /// or image-only raster cases, chart-only notes, arbitrary Office chart
    /// grammars, source chart styling/formulas/external data, exact floating chart
    /// placement, vector metafiles, malformed internal-anchor or bookmark names,
    /// other fields, annotations, nested notes, source IDs and numbering,
    /// separators, custom marks, complex anchors, and page-bottom placement remain
    /// outside the bounded opened-DOCX note path.
    /// Standalone [`write_docx`] remains model-only for all of these private
    /// hints and does not emit note parts; it does serialize modeled body/running
    /// internal anchors and bookmarks.
    /// Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn to_docx(&self) -> Vec<u8> {
        match &self.backend {
            Backend::Doc(state) => {
                let mut assembled = legacy_build_output_from_doc_state(state);
                legacy_project_promoted_running_surfaces(&mut assembled);
                legacy_promote_exact_notes_for_fresh_conversion(state, &mut assembled);
                write::to_docx_with_source_hints(
                    &assembled.model,
                    write::SourceWriteHints {
                        gaps: &assembled.section_column_gap_pt,
                        layouts: &assembled.section_column_layouts,
                        separators: &assembled.section_column_separators,
                        rtl: &assembled.section_column_rtl,
                        final_gap: assembled.final_section_column_gap_pt,
                        final_layout: assembled.final_section_column_layout.as_ref(),
                        final_separator: assembled.final_section_column_separator,
                        final_rtl: assembled.final_section_column_rtl,
                        running_surface_distances: &assembled.running_surface_distances,
                        running_line_spacing: &assembled.running_line_spacing_hints,
                        running_pagination: &assembled.running_pagination_hints,
                        running_tab_stops: &assembled.running_tab_stops,
                        running_table_cell_tab_stops: &assembled.running_table_cell_tab_stops,
                        running_table_layout: &[],
                        running_column_break_offsets: &assembled.running_column_break_offsets,
                        note_payloads: &[],
                        paragraph_line_spacing: &assembled.line_spacing_hints,
                        paragraph_pagination: &assembled.pagination_hints,
                        paragraph_tab_stops: &assembled.tab_stops,
                        column_break_offsets: &assembled.column_break_offsets,
                        table_cell_column_break_offsets: &assembled.table_cell_column_break_offsets,
                        table_row_pagination: &assembled.table_row_pagination,
                        table_cell_pagination: &assembled.table_cell_pagination,
                        table_cell_line_spacing: &assembled.table_cell_line_spacing,
                        table_nested_pagination: &[],
                        table_cell_tab_stops: &assembled.table_cell_tab_stops,
                    },
                )
            }
            Backend::Docx(state) => {
                let mut model = state.model.clone();
                let note_payloads = if let Some(notes) = &state.fresh_conversion_notes {
                    model.blocks = notes.body_blocks.clone();
                    notes.payloads.as_slice()
                } else {
                    model.blocks.extend(state.notes.iter().cloned());
                    &[]
                };
                write::to_docx_with_source_hints(
                    &model,
                    write::SourceWriteHints {
                        gaps: &state.section_column_gap_pt,
                        layouts: &state.section_column_layouts,
                        separators: &state.section_column_separators,
                        rtl: &state.section_column_rtl,
                        final_gap: state.final_section_column_gap_pt,
                        final_layout: state.final_section_column_layout.as_ref(),
                        final_separator: state.final_section_column_separator,
                        final_rtl: state.final_section_column_rtl,
                        running_surface_distances: &state.running_surface_distances,
                        running_line_spacing: &state.running_line_spacing_hints,
                        running_pagination: &state.running_pagination_hints,
                        running_tab_stops: &state.running_tab_stops,
                        running_table_cell_tab_stops: &state.running_table_cell_tab_stops,
                        running_table_layout: &state.running_table_layout_hints,
                        running_column_break_offsets: &state.running_column_break_offsets,
                        note_payloads,
                        paragraph_line_spacing: &state.line_spacing_hints,
                        paragraph_pagination: &state.pagination_hints,
                        paragraph_tab_stops: &state.tab_stops,
                        column_break_offsets: &state.column_break_offsets,
                        table_cell_column_break_offsets: &state.table_cell_column_break_offsets,
                        table_row_pagination: &state.table_row_pagination,
                        table_cell_pagination: &state.table_cell_pagination,
                        table_cell_line_spacing: &state.table_cell_line_spacing,
                        table_nested_pagination: &state.table_nested_pagination,
                        table_cell_tab_stops: &state.table_cell_tab_stops,
                    },
                )
            }
        }
    }

    /// **Package-preserving save** — re-emit this document's `.docx` with every part
    /// it doesn't model preserved verbatim (themes, settings, fonts, comments,
    /// custom XML, charts, embeddings, unknown parts). A no-op `open → save` is
    /// byte-stable per part. Preservation edits ([`Document::replace_body_text`],
    /// [`Document::set_field_result`], [`Document::replace_header_footer_text`],
    /// [`Document::replace_text_in_part`], [`Document::add_footnote_on_text`],
    /// [`Document::add_endnote_on_text`], [`Document::add_image_png`],
    /// [`Document::fill_content_control_by_tag`], [`Document::fill_content_controls_by_tag`],
    /// [`Document::fill_template_fields`],
    /// [`Document::accept_all_revisions`], [`Document::reject_all_revisions`],
    /// [`Document::insert_body_paragraph`], [`Document::remove_body_block`],
    /// [`Document::move_body_block`],
    /// [`Document::set_hyperlink_target`], [`Document::replace_image_png`]) mutate only
    /// their target XML/media/relationship parts, so
    /// untouched **non-metadata** parts stay byte-for-byte;
    /// `[Content_Types].xml` is rewritten only when an edit must *repair* a touched
    /// part's content typing (e.g. the source lacked or mistyped the `word/document.xml`
    /// override) so the output stays Word-openable. This is distinct
    /// from [`Document::to_docx`], which regenerates a fresh package from the lossy
    /// model (use that to *convert* a `.doc`). `save()` requires a `.docx`-backed
    /// document (one from [`Document::open`] on a `.docx`, or [`Document::new`]); a
    /// `.doc`-backed document has no package to preserve and returns an error pointing
    /// to [`Document::to_docx`]. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn save(&self) -> Result<Vec<u8>> {
        match &self.backend {
            // Refuse to save a package that wasn't fully retained on open: if `from_zip`
            // had to skip an unreadable/corrupt entry, `save()` would silently drop that
            // part, breaking the preservation guarantee. (Reading still works — use
            // `to_docx()` to emit a fresh package from the model instead.)
            Backend::Docx(d) if !d.package.is_complete() => Err(Error::Docx(
                "save() cannot preserve this package: it was opened with one or more \
                 unreadable/corrupt parts that were not retained — re-acquire the source \
                 file, or use to_docx() to emit a fresh package from the model"
                    .into(),
            )),
            // The retained package already holds every part (and any element-tree edit
            // is already applied to its `document.xml`); serializing it preserves them.
            Backend::Docx(d) => d.package.to_zip(),
            Backend::Doc(_) => Err(Error::Docx(
                "save() preserves an opened .docx package; this document was opened \
                 from a legacy .doc — use to_docx() to convert it"
                    .into(),
            )),
        }
    }

    /// Begin a package-level transaction for several `.docx` edit operations.
    ///
    /// The retained package is snapshotted after the same safety checks used by
    /// every package-preserving edit. Existing mutable `Document` methods can be
    /// called directly on the returned [`EditSession`]. Call
    /// [`EditSession::commit`] to keep the staged changes; explicit rollback,
    /// ordinary drop, and panic unwinding restore the exact pre-session package.
    ///
    /// This requires a safely editable `.docx` backend. Legacy `.doc`, incomplete
    /// retained packages, and lossy OPC metadata return the existing editability
    /// error before a session is created.
    #[cfg(feature = "docx")]
    pub fn edit_session(&mut self) -> Result<EditSession<'_>> {
        if self.edit_session_active {
            return Err(Error::Docx(
                "cannot begin an edit session while another edit session is active".into(),
            ));
        }
        let original_package = self.docx_tree_editable_ref()?.package.clone();
        self.edit_session_active = true;
        Ok(EditSession {
            document: self,
            original_package: Some(original_package),
        })
    }

    /// Rebuild all `.docx` read views from the current retained package.
    ///
    /// Package-preserving edit methods update the authoritative OPC package
    /// without incrementally mutating the lossy model, text, metadata, notes,
    /// comments, fields, shapes, images, or renderer sidecars. This method
    /// validates and serializes that package to ephemeral bytes, reparses every
    /// read surface, and then restores the original retained package object so
    /// touched-part diagnostics and future package-preserving saves are kept.
    /// No read state changes unless both serialization and reparsing succeed.
    ///
    /// [`EditSession::commit`] calls this automatically. Direct edit callers may
    /// invoke it explicitly instead of saving and reopening. Calling it through
    /// an active edit session is rejected so a later rollback cannot leave staged
    /// read state behind. Legacy `.doc`, incomplete packages, and lossy OPC
    /// metadata return the existing editability error.
    #[cfg(feature = "docx")]
    pub fn refresh_read_view(&mut self) -> Result<()> {
        if self.edit_session_active {
            return Err(Error::Docx(
                "refresh_read_view cannot run during an active edit session; commit refreshes automatically"
                    .into(),
            ));
        }
        self.refresh_read_view_impl()
    }

    #[cfg(feature = "docx")]
    fn refresh_read_view_impl(&mut self) -> Result<()> {
        let d = self.docx_tree_editable()?;
        refresh_docx_read_view(d)
    }

    /// **Package-preserving edit: set a `.docx` core document property.**
    /// Updates or creates `docProps/core.xml`, ensures the package-root
    /// core-properties relationship and content type, and writes the selected
    /// property as text.
    ///
    /// This edits package metadata only; `word/document.xml` and other content parts
    /// remain untouched. Read views remain stale until explicitly refreshed or reopened.
    /// Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn set_core_property(&mut self, property: CoreProperty, value: &str) -> Result<()> {
        let d = self.docx_tree_editable()?;
        let mut pkg = d.package.clone();
        if !pkg.has_part("docProps/core.xml") {
            pkg.set_part(
                "docProps/core.xml",
                core_properties_skeleton().to_vec(),
                Some(CT_CORE_PROPERTIES),
            );
        } else {
            pkg.ensure_content_type("docProps/core.xml", CT_CORE_PROPERTIES);
        }
        pkg.ensure_relationship("", REL_CORE_PROPERTIES, "docProps/core.xml");
        {
            let tree = pkg.part_tree_mut("docProps/core.xml")?;
            let root = tree.part_root_strict_ns(
                "docProps/core.xml",
                CORE_PROPERTIES_NS,
                b"coreProperties",
                "cp",
            )?;
            tree.set_child_text_ns_local_with_attrs(
                root,
                property.ns(),
                property.local(),
                property.qname(),
                property.fallback_attrs(),
                value,
            )?;
        }
        pkg.ensure_content_type("docProps/core.xml", CT_CORE_PROPERTIES);
        commit_docx_package(d, pkg)?;
        Ok(())
    }

    /// **Element-tree editing: replace body text in place.** Finds
    /// every accepted-current text run (`w:t`) whose text equals `old` and rewrites it to `new`,
    /// editing the live `word/document.xml` element tree — so **everything else is
    /// preserved**, including content the model can't represent (fields, content
    /// controls, shapes, comments, tracked changes). Returns how many runs changed.
    ///
    /// This promotes `document.xml` to an editable tree; [`Document::save`] then
    /// re-serializes only that part (every other part stays byte-for-byte). Requires
    /// a `.docx`-backed document. Note: this edits the package directly, not the
    /// `model()`/`text()` views, which remain stale until explicitly refreshed or
    /// reopened. On any error the document is left untouched (the edit is
    /// transactional). Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn replace_body_text(&mut self, old: &str, new: &str) -> Result<usize> {
        // Backend/editability check FIRST (so a `.doc` or un-editable package gets the
        // documented error, not a misleading `Ok`), then short-circuit a same-value
        // no-op so we don't promote/canonicalize `document.xml` for no actual change.
        let d = self.docx_tree_editable()?;
        if old == new {
            return Ok(0);
        }
        // Preflight on a throwaway parse (no promotion): confirm a body exists and
        // count matches. WordprocessingML `w:t` only (namespace-resolved), so
        // DrawingML `a:t`/default-ns `<t>` inside shapes is left alone while genuine
        // text-box `w:t` is still editable.
        let raw = d
            .package
            .part("word/document.xml")
            .ok_or_else(|| Error::Docx("missing word/document.xml".into()))?;
        let probe = xmltree::XmlTree::parse(&raw)?;
        // Strict: a multi-root / non-`w:document` `document.xml` is malformed → passthrough-only.
        let probe_body = probe.wml_body_strict()?;
        // Anchored to the body, so a stray `w:t` sibling of `w:body` is never edited.
        let matched: Vec<_> = probe
            .wml_text_runs_under(probe_body)
            .into_iter()
            .filter(|&id| probe.text_of(id) == old)
            .collect();
        if matched.is_empty() {
            // Nothing to do: don't promote/canonicalize the part or change edit mode.
            return Ok(0);
        }
        // Each match without a reusable text carrier (e.g. an empty `<w:t/>`) allocates a
        // new node; preflight that against the node budget so the commit can't exceed it.
        // Count against the LIVE arena (which includes any detached nodes a prior edit
        // left) — the throwaway `probe` re-parses the serialized form and would undercount.
        let new_nodes = wml_single_text_run_replacement_new_nodes(&probe, &matched, new)?;
        let live_count = d
            .package
            .part_tree_ref("word/document.xml")
            .map_or(probe.node_count(), |t| t.node_count());
        if live_count.saturating_add(new_nodes) > xmltree::node_budget() {
            return Err(Error::Docx(
                "replace_body_text: edit would exceed the node budget".into(),
            ));
        }
        // Attribute-budget preflight: if `new` carries significant edge whitespace,
        // `set_element_text` will add `xml:space="preserve"`. Reject up front (so the
        // commit stays transactional — never edits some runs then fails) any matched run
        // that is already at the attribute cap and lacks that attribute.
        if wml_replacement_needs_space_attr_preflight(new)
            && matched
                .iter()
                .any(|&id| !probe.can_set_attr(id, b"xml:space"))
        {
            return Err(Error::Docx(
                "replace_body_text: edit would exceed an element's attribute budget".into(),
            ));
        }
        // Commit on a CLONE, swapped in only after the whole edit succeeds. The budgets are
        // preflighted, but `set_element_text` is fallible (a no-carrier run allocates via
        // `try_reserve`), so a mid-loop out-of-memory could otherwise leave SOME runs
        // rewritten and others not. Building on a clone keeps the edit all-or-nothing.
        let mut pkg = d.package.clone();
        let tree = pkg.part_tree_mut("word/document.xml")?;
        let body = tree.wml_body_strict()?;
        let mut changed = 0;
        for id in tree.wml_text_runs_under(body) {
            if tree.text_of(id) == old {
                // Preflighted above (node budget + attribute budget), so this only ever
                // surfaces a genuine out-of-memory condition rather than a logic failure.
                set_wml_text_runs(tree, [id], new)?;
                changed += 1;
            }
        }
        // We've edited (touched) document.xml — guarantee the saved package types it as
        // the WML main document, so `save()` can't fail on a missing/generic override.
        pkg.ensure_content_type("word/document.xml", CT_DOCUMENT_MAIN);
        commit_docx_package(d, pkg)?;
        Ok(changed)
    }

    /// Enumerate atomic direct body blocks addressable by the conservative
    /// package-preserving structural edit methods.
    ///
    /// The returned indices cover direct `w:p`, `w:tbl`, and `w:sdt` children in
    /// retained package order. They deliberately do not claim one-to-one parity
    /// with [`DocModel::blocks`]: a direct content control may contain several
    /// modeled blocks. The same structural hazard preflight used by move/remove is
    /// applied, so opaque direct children or cross-block ranges return an error.
    #[cfg(feature = "docx")]
    pub fn body_blocks(&self) -> Result<Vec<BodyBlockInfo>> {
        let d = self.docx_tree_editable_ref()?;
        let raw = d
            .package
            .part("word/document.xml")
            .ok_or_else(|| Error::Docx("missing word/document.xml".into()))?;
        let tree = xmltree::XmlTree::parse(&raw)?;
        let body = tree.wml_body_strict()?;
        let kinds = tree.wml_atomic_body_block_kinds_under(body)?;
        let mut blocks = Vec::new();
        blocks
            .try_reserve(kinds.len())
            .map_err(|_| Error::Docx("body block descriptor inventory: out of memory".into()))?;
        blocks.extend(
            kinds
                .into_iter()
                .enumerate()
                .map(|(index, kind)| BodyBlockInfo {
                    index,
                    kind: match kind {
                        xmltree::WmlAtomicBodyBlockKind::Paragraph => BodyBlockKind::Paragraph,
                        xmltree::WmlAtomicBodyBlockKind::Table => BodyBlockKind::Table,
                        xmltree::WmlAtomicBodyBlockKind::ContentControl => {
                            BodyBlockKind::ContentControl
                        }
                    },
                }),
        );
        Ok(blocks)
    }

    /// **Package-preserving structural edit: insert one plain top-level paragraph.**
    ///
    /// `block_index` is a position in the current [`Document::body_blocks`] space:
    /// `0..=body_blocks().len()`. A position below the block count inserts
    /// immediately before that atomic direct `w:p`, `w:tbl`, or `w:sdt`; the
    /// block-count position appends before direct final `w:sectPr`.
    ///
    /// `text` is encoded as one unstyled WordprocessingML paragraph using the same
    /// escaping and significant-whitespace rules as other package-preserving text
    /// edits. Tabs become `w:tab`, line feeds become `w:br`, carriage returns and
    /// XML-forbidden controls are omitted, and an empty result inserts a blank
    /// `w:p`. Rich paragraph properties, numbering, fields, bookmarks, revisions,
    /// relationships, nested containers, and other story parts are intentionally
    /// outside this method.
    ///
    /// The same conservative structural preflight as move/remove rejects opaque
    /// direct body elements and malformed or cross-block ranges/complex fields.
    /// Read views remain stale until explicitly refreshed or reopened. On any
    /// error the retained package is unchanged.
    #[cfg(feature = "docx")]
    pub fn insert_body_paragraph(&mut self, block_index: usize, text: &str) -> Result<()> {
        let d = self.docx_tree_editable()?;
        edit_docx_atomic_body_block(
            d,
            AtomicBodyBlockEdit::InsertParagraph { block_index, text },
        )
    }

    /// **Package-preserving structural edit: remove one atomic top-level body block.**
    /// `block_index` addresses direct `w:p`, `w:tbl`, and `w:sdt` children of the
    /// main `.docx` body in source order. The exact XML subtree is removed; every
    /// other package part and sibling subtree is preserved.
    ///
    /// The edit is intentionally conservative. It rejects opaque direct body
    /// elements, malformed or cross-block ranges/complex fields, and any block
    /// carrying section properties. The read model and text views remain stale
    /// until explicitly refreshed or reopened. When the removed subtree no longer
    /// references an internal image relationship, an unreachable `word/media/*`
    /// target is pruned only when no other retained relationship points at it;
    /// other relationship kinds and shared media remain preserved. On any error
    /// the document is unchanged.
    #[cfg(feature = "docx")]
    pub fn remove_body_block(&mut self, block_index: usize) -> Result<()> {
        let d = self.docx_tree_editable()?;
        edit_docx_atomic_body_block(d, AtomicBodyBlockEdit::Remove { block_index })
    }

    /// **Package-preserving structural edit: move one atomic top-level body block.**
    /// `from_index` and `to_index` address direct `w:p`, `w:tbl`, and `w:sdt`
    /// children in source order; `to_index` is the block's final zero-based
    /// position. The exact subtree moves without regenerating its content.
    ///
    /// Moves across blocks carrying section properties, opaque body children,
    /// and malformed or cross-block ranges/complex fields are rejected. A validated
    /// same-index move is a no-op and does not promote or dirty `document.xml`.
    /// Read views remain stale until explicitly refreshed or reopened. On any error
    /// the document is unchanged.
    #[cfg(feature = "docx")]
    pub fn move_body_block(&mut self, from_index: usize, to_index: usize) -> Result<()> {
        let d = self.docx_tree_editable()?;
        edit_docx_atomic_body_block(
            d,
            AtomicBodyBlockEdit::Move {
                from_index,
                to_index,
            },
        )
    }

    /// **Package-preserving edit: accept tracked body/note/header/footer revisions.** In
    /// `word/document.xml`'s body, existing footnote/endnote parts, and referenced
    /// header/footer parts, this
    /// unwraps accepted current-content revision containers (`w:ins`,
    /// `w:moveTo`), removes rejected old-content containers (`w:del`,
    /// `w:moveFrom`), and drops tracked property-change history such as
    /// `w:pPrChange`/`w:rPrChange` while preserving the current properties.
    ///
    /// This is a focused body/note/header/footer edit, not a full Word review engine for every
    /// package part. It is transactional and returns the number of revision
    /// elements removed or unwrapped. Read views remain stale until explicitly
    /// refreshed or reopened. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn accept_all_revisions(&mut self) -> Result<usize> {
        let d = self.docx_tree_editable()?;
        edit_docx_revisions(d, RevisionEditMode::Accept)
    }

    /// **Package-preserving edit: reject tracked body/note/header/footer revisions.** In
    /// `word/document.xml`'s body, existing footnote/endnote parts, and referenced
    /// header/footer parts, this
    /// removes inserted current-content revision containers (`w:ins`,
    /// `w:moveTo`), unwraps rejected old-content containers (`w:del`,
    /// `w:moveFrom`), normalizes kept `w:delText` nodes back to `w:t`, and drops
    /// tracked property-change history such as `w:pPrChange`/`w:rPrChange` while
    /// preserving the current properties.
    ///
    /// This is a focused body/note/header/footer edit, not a full Word review engine for every
    /// package part. It is transactional and returns the number of revision or
    /// revision-text elements removed, unwrapped, or normalized. Read views remain
    /// stale until explicitly refreshed or reopened. Available with the default
    /// `docx` feature.
    #[cfg(feature = "docx")]
    pub fn reject_all_revisions(&mut self) -> Result<usize> {
        let d = self.docx_tree_editable()?;
        edit_docx_revisions(d, RevisionEditMode::Reject)
    }

    /// **Element-tree editing: rewrite a body field's cached visible result.** The
    /// zero-based `field_index` is the same accepted-current order as the body
    /// field entries returned by [`Document::fields`]. Simple fields (`w:fldSimple`)
    /// and common complex fields (`begin` / `separate` / `end`) are supported; only
    /// cached result `w:t` nodes are changed, never the field instruction.
    ///
    /// This is a preservation edit: unmodeled field markup and surrounding package
    /// parts are kept, and the edit is transactional. Like other element-tree edits,
    /// read views remain stale until explicitly refreshed or reopened. Available with
    /// the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn set_field_result(&mut self, field_index: usize, result: &str) -> Result<()> {
        let d = self.docx_tree_editable()?;
        set_field_result_in_story(
            d,
            &FieldStoryTarget::body(),
            field_index,
            result,
            "set_field_result",
            "the editable body field range",
        )
    }

    /// **Element-tree editing: rewrite one explicit story part's cached field
    /// result.** The zero-based `field_index` is obtained from
    /// [`Document::fields_in_part`] for the same `part_name`.
    ///
    /// The supported story parts and accepted-current selection rules are the
    /// same as [`Document::fields_in_part`]. Only cached result `w:t` content is
    /// changed; instructions, note separator entries, untaken compatibility
    /// branches, and every other package part are preserved. The edit is
    /// transactional, and read views remain stale until explicitly refreshed or
    /// reopened.
    #[cfg(feature = "docx")]
    pub fn set_field_result_in_part(
        &mut self,
        part_name: &str,
        field_index: usize,
        result: &str,
    ) -> Result<()> {
        let d = self.docx_tree_editable()?;
        let target =
            explicit_field_story_target(&d.package, part_name, "set_field_result_in_part")?;
        let range = format!("the editable field range for {part_name:?}");
        set_field_result_in_story(
            d,
            &target,
            field_index,
            result,
            "set_field_result_in_part",
            &range,
        )
    }

    /// **Template-fill edit: replace body content-control text by tag.** Finds
    /// accepted-current body `w:sdt` content controls whose `w:sdtPr/w:tag/@w:val`
    /// exactly equals `tag`, replaces each matching control's visible
    /// WordprocessingML `w:t` content with `text`, and preserves the
    /// content-control metadata and surrounding package. Returns the number of
    /// content controls filled.
    ///
    /// This is intentionally focused on plain-text template fields represented by
    /// content controls. It does not remove the controls, alter aliases/tags, or
    /// evaluate data binding. Tabs and newlines become inline WordprocessingML
    /// `w:tab` and text-wrapping `w:br` elements in the first existing run;
    /// page/column breaks and other run objects are preserved. For a record of
    /// tag/value pairs, use
    /// [`Document::fill_content_controls_by_tag`]. Read views remain stale until
    /// explicitly refreshed or reopened. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn fill_content_control_by_tag(&mut self, tag: &str, text: &str) -> Result<usize> {
        self.fill_content_controls_by_tag_impl(
            vec![(tag.to_string(), text.to_string())],
            "fill_content_control_by_tag",
        )
    }

    /// **Template-fill edit: replace multiple body content controls by tag.**
    /// Each `(tag, text)` pair fills every accepted-current body `w:sdt` content
    /// control whose `w:sdtPr/w:tag/@w:val` exactly equals `tag`. All fills are
    /// validated first and then committed as one package-preserving edit. Missing
    /// tags are ignored, and the return value is the number of content controls
    /// filled.
    ///
    /// Duplicate input tags are rejected so callers do not accidentally depend on
    /// ordering. Use repeated content controls with the same tag when one value
    /// should populate several template locations. Tabs and newlines become inline
    /// `w:tab` and text-wrapping `w:br` elements; later fills clear those generated
    /// markers, and marker-only values retain an empty `w:t` refill anchor. Available
    /// with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn fill_content_controls_by_tag<I, K, V>(&mut self, values: I) -> Result<usize>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let entries = values
            .into_iter()
            .map(|(tag, text)| (tag.as_ref().to_string(), text.as_ref().to_string()))
            .collect();
        self.fill_content_controls_by_tag_impl(entries, "fill_content_controls_by_tag")
    }

    /// **Template-fill edit: fill logical template fields by name.** Each
    /// `(name, text)` pair fills every body, real footnote/endnote, or
    /// accepted-current referenced header/footer content control whose
    /// `w:sdtPr/w:tag/@w:val` exactly equals `name` and every matching
    /// `MERGEFIELD` in those same story regions. Cached merge-field result text is
    /// replaced while the field instruction markup is preserved; note separator
    /// boilerplate is not edited.
    ///
    /// All fills are validated first and then committed as one
    /// package-preserving edit. Missing names are ignored, and the return value is
    /// the number of template locations filled. Duplicate input names are
    /// rejected so callers do not accidentally depend on ordering. Tabs and
    /// newlines become inline `w:tab` and text-wrapping `w:br` elements in the
    /// existing result run; repeated fills remove those generated markers while
    /// preserving page/column breaks and other run objects. Read views remain stale
    /// until explicitly refreshed or reopened. Available with the default `docx`
    /// feature.
    #[cfg(feature = "docx")]
    pub fn fill_template_fields<I, K, V>(&mut self, values: I) -> Result<usize>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let entries = values
            .into_iter()
            .map(|(name, text)| (name.as_ref().to_string(), text.as_ref().to_string()))
            .collect();
        self.fill_template_fields_impl(entries, "fill_template_fields")
    }

    #[cfg(feature = "docx")]
    fn fill_content_controls_by_tag_impl(
        &mut self,
        entries: Vec<(String, String)>,
        caller: &str,
    ) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let mut seen_tags = std::collections::HashSet::new();
        for (tag, _) in &entries {
            if tag.is_empty() {
                return Err(Error::Docx(format!("{caller}: tag must not be empty")));
            }
            if !seen_tags.insert(tag.as_str()) {
                return Err(Error::Docx(format!("{caller}: duplicate tag {tag:?}")));
            }
        }

        let d = self.docx_tree_editable()?;
        let raw = d
            .package
            .part("word/document.xml")
            .ok_or_else(|| Error::Docx("missing word/document.xml".into()))?;
        let probe = xmltree::XmlTree::parse(&raw)?;
        let probe_body = probe.wml_body_strict()?;
        let mut matched = Vec::new();
        for (entry_index, (tag, _)) in entries.iter().enumerate() {
            for group in probe.wml_content_control_text_groups_by_tag_under(probe_body, tag) {
                if group.text_runs().is_empty() {
                    return Err(Error::Docx(format!(
                        "{caller}: content control tag {tag:?} has no visible text"
                    )));
                }
                matched.push((entry_index, group));
            }
        }
        if matched.is_empty() {
            return Ok(0);
        }

        let mut seen_runs = std::collections::HashSet::new();
        for (_, group) in &matched {
            for &id in group.text_runs().iter().chain(group.marker_nodes().iter()) {
                if !seen_runs.insert(id) {
                    return Err(Error::Docx(format!(
                        "{caller}: requested tags overlap in nested content controls"
                    )));
                }
            }
        }

        let new_nodes = matched
            .iter()
            .try_fold(0usize, |total, (entry_index, group)| {
                wml_grouped_template_text_replacement_new_nodes(
                    &probe,
                    group,
                    &entries[*entry_index].1,
                )
                .map(|count| total.saturating_add(count))
            })?;
        let live_count = d
            .package
            .part_tree_ref("word/document.xml")
            .map_or(probe.node_count(), |t| t.node_count());
        if live_count.saturating_add(new_nodes) > xmltree::node_budget() {
            return Err(Error::Docx(format!(
                "{caller}: edit would exceed the node budget"
            )));
        }

        if matched.iter().any(|(entry_index, runs)| {
            let text = &entries[*entry_index].1;
            wml_replacement_needs_space_attr_preflight(text)
                && runs
                    .text_runs()
                    .first()
                    .is_some_and(|&id| !probe.can_set_attr(id, b"xml:space"))
        }) {
            return Err(Error::Docx(format!(
                "{caller}: edit would exceed an element's attribute budget"
            )));
        }

        let mut pkg = d.package.clone();
        {
            let tree = pkg.part_tree_mut("word/document.xml")?;
            let body = tree.wml_body_strict()?;
            let mut replacements = Vec::new();
            for (entry_index, (tag, _)) in entries.iter().enumerate() {
                for group in tree.wml_content_control_text_groups_by_tag_under(body, tag) {
                    if group.text_runs().is_empty() {
                        return Err(Error::Docx(format!(
                            "{caller}: content control tag {tag:?} has no visible text"
                        )));
                    }
                    replacements.push((entry_index, group));
                }
            }
            tree.prepare_wml_text_groups_for_replacement(
                replacements.iter().map(|(_, group)| group),
            )?;
            for (entry_index, group) in replacements {
                set_wml_template_text_runs(
                    tree,
                    group.into_replacement_text_runs(),
                    &entries[entry_index].1,
                )?;
            }
        }
        pkg.ensure_content_type("word/document.xml", CT_DOCUMENT_MAIN);
        let changed = matched.len();
        commit_docx_package(d, pkg)?;
        Ok(changed)
    }

    #[cfg(feature = "docx")]
    fn fill_template_fields_impl(
        &mut self,
        entries: Vec<(String, String)>,
        caller: &str,
    ) -> Result<usize> {
        if entries.is_empty() {
            return Ok(0);
        }

        let mut seen_names = std::collections::HashSet::new();
        for (name, _) in &entries {
            if name.is_empty() {
                return Err(Error::Docx(format!(
                    "{caller}: field name must not be empty"
                )));
            }
            if !seen_names.insert(name.as_str()) {
                return Err(Error::Docx(format!(
                    "{caller}: duplicate field name {name:?}"
                )));
            }
        }

        let d = self.docx_tree_editable()?;
        let raw = d
            .package
            .part("word/document.xml")
            .ok_or_else(|| Error::Docx("missing word/document.xml".into()))?;
        let probe = xmltree::XmlTree::parse(&raw)?;
        let probe_body = probe.wml_body_strict()?;
        let body_field_instructions = probe.wml_field_instructions_under(probe_body);

        let mut matched_runs = Vec::new();
        for (entry_index, (name, _)) in entries.iter().enumerate() {
            for group in probe.wml_content_control_text_groups_by_tag_under(probe_body, name) {
                if group.text_runs().is_empty() {
                    return Err(Error::Docx(format!(
                        "{caller}: template field {name:?} has no visible text"
                    )));
                }
                matched_runs.push((entry_index, group));
            }
        }

        let mut matched_fields = Vec::new();
        for (field_index, instruction) in body_field_instructions.iter().enumerate() {
            let Some(name) = merge_field_name(instruction) else {
                continue;
            };
            let Some(entry_index) = entries
                .iter()
                .position(|(entry_name, _)| entry_name == &name)
            else {
                continue;
            };
            let group = probe
                .wml_field_result_text_group_under(probe_body, field_index)
                .ok_or_else(|| {
                    Error::Docx(format!(
                        "{caller}: merge field {name:?} has no cached result"
                    ))
                })?;
            if group.text_runs().is_empty() {
                return Err(Error::Docx(format!(
                    "{caller}: merge field {name:?} has no cached result text"
                )));
            }
            matched_fields.push((field_index, entry_index));
            matched_runs.push((entry_index, group));
        }

        let story_targets = note_part_targets()
            .into_iter()
            .map(StoryTemplateTarget::from)
            .chain(
                header_footer_targets(&d.package)
                    .into_iter()
                    .map(StoryTemplateTarget::from),
            );
        let mut matched_story_content_targets = Vec::new();
        let mut matched_story_content_count = 0usize;
        let mut matched_story_fields = Vec::new();
        for target in story_targets {
            let Some(part_match) =
                collect_story_template_match(&d.package, target, &entries, caller)?
            else {
                continue;
            };
            if part_match.content_count > 0 {
                matched_story_content_count += part_match.content_count;
                matched_story_content_targets.push(part_match.target.clone());
            }
            for (field_index, entry_index) in part_match.fields {
                matched_story_fields.push((part_match.target.clone(), field_index, entry_index));
            }
        }

        let changed = matched_runs.len() + matched_story_content_count + matched_story_fields.len();
        if changed == 0 {
            return Ok(0);
        }

        if !matched_runs.is_empty() {
            let mut seen_runs = std::collections::HashSet::new();
            for (_, group) in &matched_runs {
                for &id in group.text_runs().iter().chain(group.marker_nodes().iter()) {
                    if !seen_runs.insert(id) {
                        return Err(Error::Docx(format!(
                            "{caller}: requested template fields overlap"
                        )));
                    }
                }
            }

            let new_nodes =
                matched_runs
                    .iter()
                    .try_fold(0usize, |total, (entry_index, group)| {
                        wml_grouped_template_text_replacement_new_nodes(
                            &probe,
                            group,
                            &entries[*entry_index].1,
                        )
                        .map(|count| total.saturating_add(count))
                    })?;
            let live_count = d
                .package
                .part_tree_ref("word/document.xml")
                .map_or(probe.node_count(), |t| t.node_count());
            if live_count.saturating_add(new_nodes) > xmltree::node_budget() {
                return Err(Error::Docx(format!(
                    "{caller}: edit would exceed the node budget"
                )));
            }

            if matched_runs.iter().any(|(entry_index, group)| {
                let text = &entries[*entry_index].1;
                wml_replacement_needs_space_attr_preflight(text)
                    && group
                        .text_runs()
                        .first()
                        .is_some_and(|&id| !probe.can_set_attr(id, b"xml:space"))
            }) {
                return Err(Error::Docx(format!(
                    "{caller}: edit would exceed an element's attribute budget"
                )));
            }
        }

        let mut pkg = d.package.clone();
        if !matched_runs.is_empty() {
            {
                let tree = pkg.part_tree_mut("word/document.xml")?;
                let body = tree.wml_body_strict()?;
                let mut replacements = Vec::new();
                for (entry_index, (name, _)) in entries.iter().enumerate() {
                    for group in tree.wml_content_control_text_groups_by_tag_under(body, name) {
                        if group.text_runs().is_empty() {
                            return Err(Error::Docx(format!(
                                "{caller}: template field {name:?} has no visible text"
                            )));
                        }
                        replacements.push((entry_index, group));
                    }
                }
                for (field_index, entry_index) in &matched_fields {
                    let name = &entries[*entry_index].0;
                    let group = tree
                        .wml_field_result_text_group_under(body, *field_index)
                        .ok_or_else(|| {
                            Error::Docx(format!(
                                "{caller}: merge field {name:?} has no cached result"
                            ))
                        })?;
                    if group.text_runs().is_empty() {
                        return Err(Error::Docx(format!(
                            "{caller}: merge field {name:?} has no cached result text"
                        )));
                    }
                    replacements.push((*entry_index, group));
                }
                tree.prepare_wml_text_groups_for_replacement(
                    replacements.iter().map(|(_, group)| group),
                )?;
                for (entry_index, group) in replacements {
                    set_wml_template_text_runs(
                        tree,
                        group.into_replacement_text_runs(),
                        &entries[entry_index].1,
                    )?;
                }
            }
            pkg.ensure_content_type("word/document.xml", CT_DOCUMENT_MAIN);
        }

        for target in &matched_story_content_targets {
            {
                let tree = pkg.part_tree_mut(&target.part)?;
                let root = tree.wml_part_root_strict(&target.part, target.root_local)?;
                let roots = story_template_roots(tree, target, root);
                let mut replacements = Vec::new();
                for (entry_index, (name, _)) in entries.iter().enumerate() {
                    for &story_root in &roots {
                        for group in
                            tree.wml_content_control_text_groups_by_tag_under(story_root, name)
                        {
                            if group.text_runs().is_empty() {
                                return Err(Error::Docx(format!(
                                    "{caller}: template field {name:?} has no visible text"
                                )));
                            }
                            replacements.push((entry_index, group));
                        }
                    }
                }
                tree.prepare_wml_text_groups_for_replacement(
                    replacements.iter().map(|(_, group)| group),
                )?;
                for (entry_index, group) in replacements {
                    set_wml_template_text_runs(
                        tree,
                        group.into_replacement_text_runs(),
                        &entries[entry_index].1,
                    )?;
                }
            }
            pkg.ensure_content_type(&target.part, target.content_type);
        }

        for (target, field_index, entry_index) in &matched_story_fields {
            {
                let tree = pkg.part_tree_mut(&target.part)?;
                let root = tree.wml_part_root_strict(&target.part, target.root_local)?;
                let roots = story_template_roots(tree, target, root);
                let name = &entries[*entry_index].0;
                let text = &entries[*entry_index].1;
                let group =
                    story_field_result_text_group(tree, &roots, *field_index).ok_or_else(|| {
                        Error::Docx(format!(
                            "{caller}: merge field {name:?} has no cached result"
                        ))
                    })?;
                if group.text_runs().is_empty() {
                    return Err(Error::Docx(format!(
                        "{caller}: merge field {name:?} has no cached result text"
                    )));
                }
                tree.prepare_wml_text_groups_for_replacement(std::iter::once(&group))?;
                set_wml_template_text_runs(tree, group.into_replacement_text_runs(), text)?;
            }
            pkg.ensure_content_type(&target.part, target.content_type);
        }
        commit_docx_package(d, pkg)?;
        Ok(changed)
    }

    /// **Package-preserving edit: retarget a body hyperlink.** The zero-based
    /// `hyperlink_index` is the accepted-current order of `w:hyperlink r:id="..."`
    /// elements in `word/document.xml` body order. Only relationship-backed external
    /// hyperlinks are supported; deleted/moved-from links, field-code hyperlinks,
    /// and internal anchors are left untouched.
    ///
    /// This rewrites the matching external hyperlink relationship target in
    /// `word/_rels/document.xml.rels` and leaves `word/document.xml` byte-preserved.
    /// If multiple body hyperlinks share the same relationship id, updating any one
    /// of those indexes updates the shared relationship. Read views remain stale
    /// until explicitly refreshed or reopened. Available with the default `docx`
    /// feature.
    #[cfg(feature = "docx")]
    pub fn set_hyperlink_target(&mut self, hyperlink_index: usize, target: &str) -> Result<()> {
        let d = self.docx_tree_editable()?;
        let rids = body_hyperlink_rids(&d.package)?;
        let rid = rids.get(hyperlink_index).ok_or_else(|| {
            Error::Docx(format!("hyperlink index {hyperlink_index} out of range"))
        })?;

        let mut pkg = d.package.clone();
        pkg.set_external_relationship_target(
            "word/document.xml",
            REL_HYPERLINK,
            rid.as_str(),
            target,
        )?;
        commit_docx_package(d, pkg)?;
        Ok(())
    }

    /// **Element-tree editing: rewrite an existing `.docx` comment body.**
    /// Locates the `w:comment` with `w:id == comment_id` in `word/comments.xml`,
    /// replaces its cached visible `w:t` text with `text`, and preserves the
    /// comment's metadata, body anchors, and all other comments.
    ///
    /// This updates existing comments only. Creating a new comment requires
    /// coordinated body markers and relationships and is a separate edit surface.
    /// Read views remain stale until explicitly refreshed or reopened. Available with
    /// the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn set_comment_text(&mut self, comment_id: &str, text: &str) -> Result<()> {
        let d = self.docx_tree_editable()?;
        let mut pkg = d.package.clone();
        {
            let tree = pkg.part_tree_mut("word/comments.xml")?;
            let root = tree.wml_part_root_strict("word/comments.xml", b"comments")?;
            tree.set_wml_comment_text_under(root, comment_id, text)?;
        }
        pkg.ensure_content_type("word/comments.xml", CT_COMMENTS);
        commit_docx_package(d, pkg)?;
        Ok(())
    }

    /// **Package-preserving edit: add a `.docx` comment anchored to body text.**
    /// Finds the first accepted-current body `w:r` or adjacent body `w:r`
    /// sequence whose visible `w:t` text equals `anchor_text`, inserts comment
    /// range/reference markup around those runs, appends a new `w:comment` to
    /// `word/comments.xml`, and creates the comments part and document
    /// relationship if they are missing.
    ///
    /// This is intentionally conservative: it anchors whole adjacent runs, not an
    /// arbitrary character range inside a run. The returned string is the allocated
    /// comment id. Read views remain stale until explicitly refreshed or reopened.
    /// Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn add_comment_on_text(
        &mut self,
        anchor_text: &str,
        comment_text: &str,
        author: &str,
    ) -> Result<String> {
        if anchor_text.is_empty() {
            return Err(Error::Docx(
                "add_comment_on_text: anchor text must not be empty".into(),
            ));
        }
        let d = self.docx_tree_editable()?;
        let id = next_comment_id(&d.package)?;
        let mut pkg = d.package.clone();

        if !pkg.has_part("word/comments.xml") {
            pkg.set_part(
                "word/comments.xml",
                comments_part_skeleton().to_vec(),
                Some(CT_COMMENTS),
            );
        } else {
            pkg.ensure_content_type("word/comments.xml", CT_COMMENTS);
        }
        pkg.ensure_relationship("word/document.xml", REL_COMMENTS, "word/comments.xml");

        {
            let tree = pkg.part_tree_mut("word/document.xml")?;
            let body = tree.wml_body_strict()?;
            tree.add_wml_comment_anchor_on_text(body, anchor_text, &id)?;
        }
        {
            let tree = pkg.part_tree_mut("word/comments.xml")?;
            let root = tree.wml_part_root_strict("word/comments.xml", b"comments")?;
            tree.append_wml_comment(root, &id, comment_text, author)?;
        }
        pkg.ensure_content_type("word/document.xml", CT_DOCUMENT_MAIN);
        pkg.ensure_content_type("word/comments.xml", CT_COMMENTS);
        commit_docx_package(d, pkg)?;
        Ok(id)
    }

    /// **Element-tree editing: rewrite one existing `.docx` body table cell.**
    /// `table_index` and `row_index` are zero-based indexes into accepted-current
    /// top-level `w:tbl` elements in `word/document.xml`; `cell_index` is a
    /// zero-based logical column that accounts for horizontal `w:gridSpan`. A
    /// `row_index` inside a vertical `w:vMerge` continuation resolves to the
    /// restart/origin cell. The target cell's visible `w:t` content is replaced by
    /// `text`; surrounding table structure and other cells are preserved.
    ///
    /// This is intentionally a focused body-table edit surface. When a parent cell
    /// contains a nested table, only the parent's direct text is replaced; nested
    /// table content and structure remain untouched. Read views remain stale until
    /// explicitly refreshed or reopened.
    /// Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn set_table_cell_text(
        &mut self,
        table_index: usize,
        row_index: usize,
        cell_index: usize,
        text: &str,
    ) -> Result<()> {
        let d = self.docx_tree_editable()?;
        let mut pkg = d.package.clone();
        {
            let tree = pkg.part_tree_mut("word/document.xml")?;
            let body = tree.wml_body_strict()?;
            let index_error = || {
                Error::Docx(format!(
                    "table cell index out of range: table={table_index} row={row_index} cell={cell_index}"
                ))
            };
            let runs = tree
                .wml_table_cell_text_runs_under(body, table_index, row_index, cell_index)
                .ok_or_else(index_error)?;
            if runs.is_empty() {
                return Err(Error::Docx(format!(
                    "table={table_index} row={row_index} cell={cell_index} has no visible text"
                )));
            }

            let new_nodes = wml_grouped_text_run_replacement_new_nodes(tree, &runs, text)?;
            if tree.node_count().saturating_add(new_nodes) > xmltree::node_budget() {
                return Err(Error::Docx(
                    "set_table_cell_text: edit would exceed the node budget".into(),
                ));
            }

            if wml_replacement_needs_space_attr_preflight(text)
                && !tree.can_set_attr(runs[0], b"xml:space")
            {
                return Err(Error::Docx(
                    "set_table_cell_text: edit would exceed an element's attribute budget".into(),
                ));
            }

            set_wml_text_runs(tree, runs, text)?;
        }
        pkg.ensure_content_type("word/document.xml", CT_DOCUMENT_MAIN);
        commit_docx_package(d, pkg)?;
        Ok(())
    }

    /// **Package-preserving edit: add a `.docx` footnote anchored to body text.**
    /// Finds the first accepted-current body `w:r` or adjacent body `w:r`
    /// sequence whose visible `w:t` text equals `anchor_text`, inserts a
    /// `w:footnoteReference` run after the matched runs, appends a new real
    /// `w:footnote` to `word/footnotes.xml`, and creates the footnotes part,
    /// relationship, and content type if they are missing.
    ///
    /// This is intentionally conservative: it anchors whole adjacent runs, not an
    /// arbitrary character range inside a run. The returned string is the allocated
    /// footnote id. Read views remain stale until explicitly refreshed or reopened.
    /// Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn add_footnote_on_text(&mut self, anchor_text: &str, note_text: &str) -> Result<String> {
        if anchor_text.is_empty() {
            return Err(Error::Docx(
                "add_footnote_on_text: anchor text must not be empty".into(),
            ));
        }
        let d = self.docx_tree_editable()?;
        let id = next_footnote_id(&d.package)?;
        let mut pkg = d.package.clone();

        if !pkg.has_part("word/footnotes.xml") {
            pkg.set_part(
                "word/footnotes.xml",
                footnotes_part_skeleton().to_vec(),
                Some(CT_FOOTNOTES),
            );
        } else {
            pkg.ensure_content_type("word/footnotes.xml", CT_FOOTNOTES);
        }
        pkg.ensure_relationship("word/document.xml", REL_FOOTNOTES, "word/footnotes.xml");

        {
            let tree = pkg.part_tree_mut("word/document.xml")?;
            let body = tree.wml_body_strict()?;
            tree.add_wml_footnote_reference_on_text(body, anchor_text, &id)?;
        }
        {
            let tree = pkg.part_tree_mut("word/footnotes.xml")?;
            let root = tree.wml_part_root_strict("word/footnotes.xml", b"footnotes")?;
            tree.append_wml_footnote(root, &id, note_text)?;
        }
        pkg.ensure_content_type("word/document.xml", CT_DOCUMENT_MAIN);
        pkg.ensure_content_type("word/footnotes.xml", CT_FOOTNOTES);
        commit_docx_package(d, pkg)?;
        Ok(id)
    }

    /// **Package-preserving edit: add a `.docx` endnote anchored to body text.**
    /// Finds the first accepted-current body `w:r` or adjacent body `w:r`
    /// sequence whose visible `w:t` text equals `anchor_text`, inserts a
    /// `w:endnoteReference` run after the matched runs, appends a new real
    /// `w:endnote` to `word/endnotes.xml`, and creates the endnotes part,
    /// relationship, and content type if they are missing.
    ///
    /// This is intentionally conservative: it anchors whole adjacent runs, not an
    /// arbitrary character range inside a run. The returned string is the allocated
    /// endnote id. Read views remain stale until explicitly refreshed or reopened.
    /// Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn add_endnote_on_text(&mut self, anchor_text: &str, note_text: &str) -> Result<String> {
        if anchor_text.is_empty() {
            return Err(Error::Docx(
                "add_endnote_on_text: anchor text must not be empty".into(),
            ));
        }
        let d = self.docx_tree_editable()?;
        let id = next_endnote_id(&d.package)?;
        let mut pkg = d.package.clone();

        if !pkg.has_part("word/endnotes.xml") {
            pkg.set_part(
                "word/endnotes.xml",
                endnotes_part_skeleton().to_vec(),
                Some(CT_ENDNOTES),
            );
        } else {
            pkg.ensure_content_type("word/endnotes.xml", CT_ENDNOTES);
        }
        pkg.ensure_relationship("word/document.xml", REL_ENDNOTES, "word/endnotes.xml");

        {
            let tree = pkg.part_tree_mut("word/document.xml")?;
            let body = tree.wml_body_strict()?;
            tree.add_wml_endnote_reference_on_text(body, anchor_text, &id)?;
        }
        {
            let tree = pkg.part_tree_mut("word/endnotes.xml")?;
            let root = tree.wml_part_root_strict("word/endnotes.xml", b"endnotes")?;
            tree.append_wml_endnote(root, &id, note_text)?;
        }
        pkg.ensure_content_type("word/document.xml", CT_DOCUMENT_MAIN);
        pkg.ensure_content_type("word/endnotes.xml", CT_ENDNOTES);
        commit_docx_package(d, pkg)?;
        Ok(id)
    }

    /// **Element-tree editing: replace text in existing `.docx` footnotes and
    /// endnotes.** Finds visible `w:t` runs whose full text equals `old` in
    /// `word/footnotes.xml` and `word/endnotes.xml`, skips separator boilerplate
    /// notes, rewrites matches to `new`, and returns the number of runs changed.
    ///
    /// This edits existing notes only; creating notes and inserting body references
    /// is a separate structural edit surface. Read views remain stale until explicitly
    /// refreshed or reopened. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn replace_note_text(&mut self, old: &str, new: &str) -> Result<usize> {
        let d = self.docx_tree_editable()?;
        if old == new {
            return Ok(0);
        }

        let needs_space = new != new.trim_matches([' ', '\t', '\n', '\r']);
        let needs_markers = new.contains('\t') || new.contains('\n');
        let marker_node_count = if needs_markers {
            xmltree::wml_text_run_content_node_count(new)?
        } else {
            0
        };
        let mut editable_targets = Vec::new();
        let mut total_matches = 0usize;

        for target in note_part_targets() {
            let Some(raw) = d.package.part(target.part) else {
                continue;
            };
            let probe = xmltree::XmlTree::parse(&raw)?;
            let root = probe.wml_part_root_strict(target.part, target.root_local)?;
            let matched: Vec<_> = probe
                .wml_note_text_runs_under(root, target.note_local)
                .into_iter()
                .filter(|&id| probe.text_of(id) == old)
                .collect();
            if matched.is_empty() {
                continue;
            }

            let new_nodes = if needs_markers {
                marker_node_count.saturating_mul(matched.len())
            } else {
                matched
                    .iter()
                    .filter(|&&id| !probe.has_text_carrier(id))
                    .count()
            };
            let live_count = d
                .package
                .part_tree_ref(target.part)
                .map_or(probe.node_count(), |t| t.node_count());
            if live_count.saturating_add(new_nodes) > xmltree::node_budget() {
                return Err(Error::Docx(
                    "replace_note_text: edit would exceed the node budget".into(),
                ));
            }
            if !needs_markers
                && needs_space
                && matched
                    .iter()
                    .any(|&id| !probe.can_set_attr(id, b"xml:space"))
            {
                return Err(Error::Docx(
                    "replace_note_text: edit would exceed an element's attribute budget".into(),
                ));
            }

            total_matches += matched.len();
            editable_targets.push(target);
        }

        if total_matches == 0 {
            return Ok(0);
        }

        let mut pkg = d.package.clone();
        let mut changed = 0usize;
        for target in editable_targets {
            {
                let tree = pkg.part_tree_mut(target.part)?;
                let root = tree.wml_part_root_strict(target.part, target.root_local)?;
                for id in tree.wml_note_text_runs_under(root, target.note_local) {
                    if tree.text_of(id) == old {
                        if needs_markers {
                            tree.replace_wml_text_element_with_run_content(id, new)?;
                        } else {
                            tree.set_element_text(id, new)?;
                        }
                        changed += 1;
                    }
                }
            }
            pkg.ensure_content_type(target.part, target.content_type);
        }
        commit_docx_package(d, pkg)?;
        Ok(changed)
    }

    /// **Element-tree editing: replace text in accepted-current referenced headers and footers.**
    /// Finds `w:t` runs whose full text equals `old` in the header/footer parts
    /// accepted-current referenced from `word/document.xml`, rewrites them to
    /// `new`, and returns the number of runs changed. The main body and
    /// unreferenced or old-only header/footer parts are not touched.
    ///
    /// This uses the same package-preserving, transactional edit path as
    /// [`Document::replace_body_text`]. Read views such as [`Document::header_text`]
    /// remain stale until explicitly refreshed or reopened. Available with the
    /// default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn replace_header_footer_text(&mut self, old: &str, new: &str) -> Result<usize> {
        let d = self.docx_tree_editable()?;
        if old == new {
            return Ok(0);
        }

        let targets = header_footer_targets(&d.package);
        if targets.is_empty() {
            return Ok(0);
        }
        let mut editable_targets = Vec::new();
        let mut total_matches = 0usize;

        for target in &targets {
            let Some(raw) = d.package.part(&target.part) else {
                continue;
            };
            let probe = xmltree::XmlTree::parse(&raw)?;
            let root = probe.wml_part_root_strict(&target.part, target.root_local)?;
            let matched: Vec<_> = probe
                .wml_text_runs_under(root)
                .into_iter()
                .filter(|&id| probe.text_of(id) == old)
                .collect();
            if matched.is_empty() {
                continue;
            }

            let new_nodes = wml_single_text_run_replacement_new_nodes(&probe, &matched, new)?;
            let live_count = d
                .package
                .part_tree_ref(&target.part)
                .map_or(probe.node_count(), |t| t.node_count());
            if live_count.saturating_add(new_nodes) > xmltree::node_budget() {
                return Err(Error::Docx(
                    "replace_header_footer_text: edit would exceed the node budget".into(),
                ));
            }
            if wml_replacement_needs_space_attr_preflight(new)
                && matched
                    .iter()
                    .any(|&id| !probe.can_set_attr(id, b"xml:space"))
            {
                return Err(Error::Docx(
                    "replace_header_footer_text: edit would exceed an element's attribute budget"
                        .into(),
                ));
            }

            total_matches += matched.len();
            editable_targets.push(target.clone());
        }

        if total_matches == 0 {
            return Ok(0);
        }

        let mut pkg = d.package.clone();
        let mut changed = 0usize;
        for target in editable_targets {
            {
                let tree = pkg.part_tree_mut(&target.part)?;
                let root = tree.wml_part_root_strict(&target.part, target.root_local)?;
                for id in tree.wml_text_runs_under(root) {
                    if tree.text_of(id) == old {
                        set_wml_text_runs(tree, [id], new)?;
                        changed += 1;
                    }
                }
            }
            pkg.ensure_content_type(&target.part, target.content_type);
        }
        commit_docx_package(d, pkg)?;
        Ok(changed)
    }

    /// **Element-tree editing: replace text in one explicit existing
    /// WordprocessingML XML part.** `part_name` must be an existing conservative
    /// package path under `word/` ending in `.xml` and outside relationship parts
    /// (for example `word/header2.xml` or `word/styles.xml`). The method rewrites
    /// descendant WordprocessingML `w:t` runs whose full text equals `old` and returns
    /// the number of runs changed.
    ///
    /// Prefer specialized APIs such as [`Document::replace_body_text`] and
    /// [`Document::replace_header_footer_text`] when they match the job; this is an
    /// explicit escape hatch for parts the model does not yet expose semantically.
    /// The edit is transactional and does not infer or repair a part-specific content
    /// type. Read views remain stale until explicitly refreshed or reopened.
    /// Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn replace_text_in_part(&mut self, part_name: &str, old: &str, new: &str) -> Result<usize> {
        let d = self.docx_tree_editable()?;
        wml_xml_part_name(part_name, "replace_text_in_part")?;
        let raw = d.package.part(part_name).ok_or_else(|| {
            Error::Docx(format!("replace_text_in_part: missing part {part_name}"))
        })?;
        let probe = xmltree::XmlTree::parse(&raw)?;
        let root = probe.wml_any_part_root_strict(part_name)?;
        if old == new {
            return Ok(0);
        }

        let matched: Vec<_> = probe
            .wml_all_branch_text_runs_under(root)
            .into_iter()
            .filter(|&id| probe.text_of(id) == old)
            .collect();
        if matched.is_empty() {
            return Ok(0);
        }

        let new_nodes = wml_single_text_run_replacement_new_nodes(&probe, &matched, new)?;
        let live_count = d
            .package
            .part_tree_ref(part_name)
            .map_or(probe.node_count(), |t| t.node_count());
        if live_count.saturating_add(new_nodes) > xmltree::node_budget() {
            return Err(Error::Docx(
                "replace_text_in_part: edit would exceed the node budget".into(),
            ));
        }

        if wml_replacement_needs_space_attr_preflight(new)
            && matched
                .iter()
                .any(|&id| !probe.can_set_attr(id, b"xml:space"))
        {
            return Err(Error::Docx(
                "replace_text_in_part: edit would exceed an element's attribute budget".into(),
            ));
        }

        let mut pkg = d.package.clone();
        let tree = pkg.part_tree_mut(part_name)?;
        let root = tree.wml_any_part_root_strict(part_name)?;
        let mut changed = 0usize;
        for id in tree.wml_all_branch_text_runs_under(root) {
            if tree.text_of(id) == old {
                set_wml_text_runs(tree, [id], new)?;
                changed += 1;
            }
        }
        commit_docx_package(d, pkg)?;
        Ok(changed)
    }

    /// **Element-tree editing: append an inline PNG image** to the
    /// body, reconciling relationships transactionally — the media part, its
    /// `image/png` content-type, and a fresh non-colliding `rId` are added together,
    /// then a drawing paragraph referencing that `rId` (with a unique drawing id) is
    /// spliced into `w:body` **before** the final `w:sectPr`. Every existing
    /// part/relationship is preserved.
    ///
    /// `name` must be a plain `*.png` file name (no path separators or `..`) that does
    /// not already exist under `word/media/`. `png` is checked to be a structurally
    /// well-formed **PNG container** (signature/framing/CRCs/IHDR/ordering/zlib header,
    /// *not* a full image decode) so the declared `image/png` content type is honest.
    /// **Transactional:** all preconditions (name validity, PNG
    /// container validity, part not present, `w:body` exists, node budget) are checked
    /// before any mutation, so on error the document is unchanged. Like
    /// [`Document::replace_body_text`], this edits the package directly, so the
    /// `model()`/`images()`/`text()` read views remain stale until explicitly refreshed
    /// or reopened. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn add_image_png(&mut self, png: &[u8], name: &str) -> Result<()> {
        self.add_image_media(png, name, ImageMediaKind::Png, "add_image_png")
    }

    /// **Element-tree editing: append an inline JPEG image** to the body,
    /// reconciling the media part, `image/jpeg` content type, relationship, and
    /// drawing markup transactionally. This mirrors [`Document::add_image_png`]
    /// for plain `*.jpg`/`*.jpeg` names and structurally validated JPEG bytes.
    ///
    /// The validation is a bounded container check (SOI/EOI, segment framing,
    /// dimensions in a SOF marker, and an SOS scan start), not a full JPEG decode.
    /// Read views remain stale until explicitly refreshed or reopened. Available with
    /// the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn add_image_jpeg(&mut self, jpeg: &[u8], name: &str) -> Result<()> {
        self.add_image_media(jpeg, name, ImageMediaKind::Jpeg, "add_image_jpeg")
    }

    /// **Element-tree editing: append an inline GIF image** to the body,
    /// reconciling the media part, `image/gif` content type, relationship, and
    /// drawing markup transactionally. This mirrors [`Document::add_image_png`]
    /// for plain `*.gif` names and bounded structural GIF validation.
    ///
    /// The validation checks GIF87a/GIF89a framing, non-zero logical-screen
    /// dimensions, at least one image descriptor, and a final trailer; it does
    /// not decode LZW image data. Read views remain stale until explicitly refreshed
    /// or reopened. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn add_image_gif(&mut self, gif: &[u8], name: &str) -> Result<()> {
        self.add_image_media(gif, name, ImageMediaKind::Gif, "add_image_gif")
    }

    /// **Element-tree editing: append an inline BMP image** to the body,
    /// reconciling the media part, `image/bmp` content type, relationship, and
    /// drawing markup transactionally. This mirrors [`Document::add_image_png`]
    /// for plain `*.bmp` names and bounded structural BMP validation.
    ///
    /// The validation checks the BMP file header, BITMAPINFOHEADER dimensions,
    /// plane count, bit depth, and uncompressed pixel-data offset; it does not
    /// decode pixel rows. Read views remain stale until explicitly refreshed or
    /// reopened. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn add_image_bmp(&mut self, bmp: &[u8], name: &str) -> Result<()> {
        self.add_image_media(bmp, name, ImageMediaKind::Bmp, "add_image_bmp")
    }

    /// **Element-tree editing: append an inline TIFF image** to the body,
    /// reconciling the media part, `image/tiff` content type, relationship, and
    /// drawing markup transactionally. This mirrors [`Document::add_image_png`]
    /// for plain `*.tif` / `*.tiff` names and bounded structural TIFF dimension
    /// parsing.
    ///
    /// The validation checks a classic TIFF header and first IFD
    /// `ImageWidth`/`ImageLength` tags; it does not decode image data. Read
    /// views remain stale until explicitly refreshed or reopened. Available with the
    /// default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn add_image_tiff(&mut self, tiff: &[u8], name: &str) -> Result<()> {
        self.add_image_media(tiff, name, ImageMediaKind::Tiff, "add_image_tiff")
    }

    /// **Element-tree editing: append an inline WebP image** to the body,
    /// reconciling the media part, `image/webp` content type, relationship, and
    /// drawing markup transactionally. This mirrors [`Document::add_image_png`]
    /// for plain `*.webp` names and bounded structural WebP dimension parsing.
    ///
    /// The validation checks the RIFF/WebP container and supported VP8/VP8L/VP8X
    /// size headers; it does not decode image data. Read views remain stale until
    /// explicitly refreshed or reopened. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn add_image_webp(&mut self, webp: &[u8], name: &str) -> Result<()> {
        self.add_image_media(webp, name, ImageMediaKind::Webp, "add_image_webp")
    }

    /// **Package-preserving edit: replace an existing PNG media part.** `name`
    /// is the plain file name of an existing part under `word/media/` (for example
    /// `image1.png`). The new bytes must be a structurally valid PNG container; the
    /// existing body markup and relationships keep pointing at the same part.
    ///
    /// This is intentionally a media-part replacement, not a layout rewrite: drawing
    /// extents, alt text, captions, and relationship ids are preserved. Read views
    /// such as [`Document::images`] remain stale until explicitly refreshed or
    /// reopened. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn replace_image_png(&mut self, png: &[u8], name: &str) -> Result<()> {
        self.replace_image_media(png, name, ImageMediaKind::Png, "replace_image_png")
    }

    /// **Package-preserving edit: replace an existing JPEG media part.** `name`
    /// is the plain file name of an existing part under `word/media/` (for example
    /// `photo.jpg` or `photo.jpeg`). The new bytes must be a structurally valid
    /// JPEG container; existing drawing markup and relationships keep pointing at
    /// the same part. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn replace_image_jpeg(&mut self, jpeg: &[u8], name: &str) -> Result<()> {
        self.replace_image_media(jpeg, name, ImageMediaKind::Jpeg, "replace_image_jpeg")
    }

    /// **Package-preserving edit: replace an existing GIF media part.** `name`
    /// is the plain file name of an existing part under `word/media/` (for
    /// example `anim.gif`). The new bytes must be a structurally valid GIF
    /// container; existing drawing markup and relationships keep pointing at the
    /// same part. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn replace_image_gif(&mut self, gif: &[u8], name: &str) -> Result<()> {
        self.replace_image_media(gif, name, ImageMediaKind::Gif, "replace_image_gif")
    }

    /// **Package-preserving edit: replace an existing BMP media part.** `name`
    /// is the plain file name of an existing part under `word/media/` (for
    /// example `bitmap.bmp`). The new bytes must be a structurally valid BMP
    /// container; existing drawing markup and relationships keep pointing at the
    /// same part. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn replace_image_bmp(&mut self, bmp: &[u8], name: &str) -> Result<()> {
        self.replace_image_media(bmp, name, ImageMediaKind::Bmp, "replace_image_bmp")
    }

    /// **Package-preserving edit: replace an existing TIFF media part.** `name`
    /// is the plain file name of an existing part under `word/media/` (for
    /// example `scan.tiff`). The new bytes must be a structurally valid classic
    /// TIFF with first-IFD dimensions; existing drawing markup and relationships
    /// keep pointing at the same part. Available with the default `docx`
    /// feature.
    #[cfg(feature = "docx")]
    pub fn replace_image_tiff(&mut self, tiff: &[u8], name: &str) -> Result<()> {
        self.replace_image_media(tiff, name, ImageMediaKind::Tiff, "replace_image_tiff")
    }

    /// **Package-preserving edit: replace an existing WebP media part.** `name`
    /// is the plain file name of an existing part under `word/media/` (for
    /// example `picture.webp`). The new bytes must be a structurally valid WebP
    /// container; existing drawing markup and relationships keep pointing at the
    /// same part. Available with the default `docx` feature.
    #[cfg(feature = "docx")]
    pub fn replace_image_webp(&mut self, webp: &[u8], name: &str) -> Result<()> {
        self.replace_image_media(webp, name, ImageMediaKind::Webp, "replace_image_webp")
    }

    #[cfg(feature = "docx")]
    fn add_image_media(
        &mut self,
        bytes: &[u8],
        name: &str,
        kind: ImageMediaKind,
        op: &str,
    ) -> Result<()> {
        // Reject an oversized image FIRST — a cheap length check before the linear
        // format scan, matching the per-part budget `save()` enforces.
        if bytes.len() as u64 > opc::max_part() {
            return Err(Error::Docx(format!(
                "{op}: image exceeds the per-part size budget"
            )));
        }
        // Then validate the bytes are a structurally well-formed container, so the
        // declared image content type is not a lie.
        if !kind.is_valid(bytes) {
            return Err(Error::Docx(format!(
                "{op}: bytes are not a structurally-valid {} container",
                kind.label()
            )));
        }
        let d = self.docx_tree_editable()?;
        let part = image_media_part_name(name, kind, op)?;
        if d.package.has_part(&part) {
            return Err(Error::Docx(format!("media part {part} already exists")));
        }
        let (cx, cy) = kind.extent_emu(bytes);
        // Preflight WITHOUT promoting: read the live tree if `document.xml` is already
        // promoted (a prior edit) so the node count includes any detached nodes; else
        // parse a throwaway copy (a still-`Raw` part was never edited, so a fresh parse
        // has the same count). This keeps the budget accurate AND leaves a still-`Raw`
        // part untouched on failure (no canonicalizing promotion) — fully transactional.
        // `wml_body_strict` rejects a multi-root / non-`w:document` part before any mutation
        // (transactional preflight), so a malformed `document.xml` stays passthrough-only.
        let (draw_id, live_count) = match d.package.part_tree_ref("word/document.xml") {
            Some(t) => {
                t.wml_body_strict()?;
                (t.fresh_drawing_id(), t.node_count())
            }
            None => {
                let raw = d
                    .package
                    .part("word/document.xml")
                    .ok_or_else(|| Error::Docx("missing word/document.xml".into()))?;
                let t = xmltree::XmlTree::parse(&raw)?;
                t.wml_body_strict()?;
                (t.fresh_drawing_id(), t.node_count())
            }
        };
        // The fragment's node count is independent of the (not-yet-allocated) rId, so a
        // placeholder is fine for the budget check.
        let frag_probe = image_paragraph_xml("rIdPENDING", cx, cy, draw_id);
        let frag_nodes = xmltree::XmlTree::parse(frag_probe.as_bytes())?.node_count();
        if live_count.saturating_add(frag_nodes) > xmltree::node_budget() {
            return Err(Error::Docx(format!(
                "{op}: edit would exceed the node budget"
            )));
        }

        // Commit on a CLONE, swapped in only after every step succeeds. The budget is
        // preflighted, but the underlying tree edits are now fallible (`XmlTree::push` uses
        // `try_reserve`), so a genuine out-of-memory after `add_related_part` could
        // otherwise leave an orphaned media part + relationship. Building on a clone keeps
        // the documented guarantee literally true: on ANY error the document is unchanged.
        let mut pkg = d.package.clone();
        let rid = pkg.add_related_part(
            "word/document.xml",
            REL_IMAGE,
            &part,
            Some(kind.content_type()),
            bytes.to_vec(),
        );
        let frag = image_paragraph_xml(&rid, cx, cy, draw_id);
        let tree = pkg.part_tree_mut("word/document.xml")?;
        let body = tree.wml_body_strict()?;
        tree.insert_fragment_before_ns_local(body, frag.as_bytes(), xmltree::WML_NS, b"sectPr")?;
        // Guarantee the edited document.xml is typed as the WML main document on save.
        pkg.ensure_content_type("word/document.xml", CT_DOCUMENT_MAIN);
        commit_docx_package(d, pkg)?;
        Ok(())
    }

    #[cfg(feature = "docx")]
    fn replace_image_media(
        &mut self,
        bytes: &[u8],
        name: &str,
        kind: ImageMediaKind,
        op: &str,
    ) -> Result<()> {
        if bytes.len() as u64 > opc::max_part() {
            return Err(Error::Docx(format!(
                "{op}: image exceeds the per-part size budget"
            )));
        }
        if !kind.is_valid(bytes) {
            return Err(Error::Docx(format!(
                "{op}: bytes are not a structurally-valid {} container",
                kind.label()
            )));
        }
        let d = self.docx_tree_editable()?;
        let part = image_media_part_name(name, kind, op)?;
        if !d.package.has_part(&part) {
            return Err(Error::Docx(format!("media part {part} does not exist")));
        }

        let mut pkg = d.package.clone();
        pkg.set_part(&part, bytes.to_vec(), Some(kind.content_type()));
        // Validate the touched part's content type and write-side budgets before the
        // clone becomes authoritative, so a failed replacement leaves the document unchanged.
        commit_docx_package(d, pkg)?;
        Ok(())
    }

    /// Mutable `.docx` state for element-tree editing, refusing a `.doc` backend (no
    /// package to edit) or a package whose OPC metadata parsed lossily (editing would
    /// regenerate `[Content_Types].xml`/`.rels` from an incomplete view — the document
    /// still opens and round-trips raw, just can't be safely edited).
    #[cfg(feature = "docx")]
    fn docx_tree_editable(&mut self) -> Result<&mut docx::DocxState> {
        match &mut self.backend {
            Backend::Docx(d) => {
                ensure_docx_tree_editable(d)?;
                Ok(d)
            }
            Backend::Doc(_) => Err(Error::Docx(
                "element-tree editing requires a .docx-backed document".into(),
            )),
        }
    }

    #[cfg(feature = "docx")]
    fn docx_tree_editable_ref(&self) -> Result<&docx::DocxState> {
        match &self.backend {
            Backend::Docx(d) => {
                ensure_docx_tree_editable(d)?;
                Ok(d)
            }
            Backend::Doc(_) => Err(Error::Docx(
                "element-tree editing requires a .docx-backed document".into(),
            )),
        }
    }

    /// Test-only: the live `word/document.xml` arena node count (promotes the part),
    /// including any detached nodes a prior edit left — used to set a precise node
    /// budget for transactional-rollback tests.
    #[cfg(all(test, feature = "docx"))]
    fn docx_node_count(&mut self) -> usize {
        match &mut self.backend {
            Backend::Docx(d) => d
                .package
                .part_tree_mut("word/document.xml")
                .map(|t| t.node_count())
                .unwrap_or(0),
            Backend::Doc(_) => 0,
        }
    }

    /// Render this document to a **PDF** with native typesetting
    /// — `parley` lays out and shapes the text (Korean/CJK line-breaking and font
    /// fallback included) and `krilla` emits the PDF with subsetted embedded fonts
    /// and selectable text. Tables render as a real bordered grid with rich,
    /// shaded, vertically-aligned cells; paragraphs honor color/size/font, lists,
    /// and indentation; images are drawn. Available with the `render` feature
    /// (which raises the MSRV to 1.92).
    #[cfg(feature = "render")]
    pub fn to_pdf(&self) -> Vec<u8> {
        let features = self.report().features;
        let shapes = self.floating_shapes();
        self.with_render_model_and_hints(|model, source_hints| {
            render::to_pdf_with_fonts_and_features_and_shapes(
                model,
                &[],
                features,
                &shapes,
                source_hints,
            )
        })
    }

    /// Fallible variant of [`Document::to_pdf`]. Available with the `render`
    /// feature.
    #[cfg(feature = "render")]
    pub fn try_to_pdf(&self) -> Result<Vec<u8>> {
        let features = self.report().features;
        let shapes = self.floating_shapes();
        self.with_render_model_and_hints(|model, source_hints| {
            render::try_to_pdf_with_fonts_and_features_and_shapes(
                model,
                &[],
                features,
                &shapes,
                source_hints,
            )
        })
    }

    /// Render this document to PDF after registering caller-supplied font blobs.
    /// This is the opened-document counterpart to [`render_pdf_with_fonts`]: it
    /// keeps the same parsed-document model and lets callers provide fonts for
    /// headless/server environments. Available with the `render` feature.
    #[cfg(feature = "render")]
    pub fn to_pdf_with_fonts(&self, fonts: &[Vec<u8>]) -> Vec<u8> {
        let features = self.report().features;
        let shapes = self.floating_shapes();
        self.with_render_model_and_hints(|model, source_hints| {
            render::to_pdf_with_fonts_and_features_and_shapes(
                model,
                fonts,
                features,
                &shapes,
                source_hints,
            )
        })
    }

    /// Fallible variant of [`Document::to_pdf_with_fonts`]. Available with the
    /// `render` feature.
    #[cfg(feature = "render")]
    pub fn try_to_pdf_with_fonts(&self, fonts: &[Vec<u8>]) -> Result<Vec<u8>> {
        let features = self.report().features;
        let shapes = self.floating_shapes();
        self.with_render_model_and_hints(|model, source_hints| {
            render::try_to_pdf_with_fonts_and_features_and_shapes(
                model,
                fonts,
                features,
                &shapes,
                source_hints,
            )
        })
    }

    /// Return layout-derived page numbers from rwml's preview-grade pagination.
    ///
    /// This matches rwml's own PDF output, not Microsoft Word's pagination. Page
    /// indices are physical, 1-based page numbers; section page-number restarts
    /// and formats are intentionally not applied. The supplied fonts are used
    /// strictly: system fonts are disabled and only successfully registered
    /// caller bytes are considered. Available with the `render` feature.
    #[cfg(feature = "render")]
    pub fn layout_pages_with_fonts(&self, fonts: &[Vec<u8>]) -> Result<LayoutPages> {
        let shapes = self.floating_shapes();
        self.with_render_model_and_hints(|model, source_hints| {
            render::layout_pages_with_fonts_and_pagination(model, fonts, source_hints, &shapes)
        })
    }

    /// Render this document to PDF and return renderer metrics/warnings produced
    /// by the same pagination pass. Uses the opened document's feature inventory
    /// so warnings can include unsupported preserved features that are not fully
    /// represented in [`DocModel`]. Available with the `render` feature.
    #[cfg(feature = "render")]
    pub fn to_pdf_with_report(&self) -> RenderedPdf {
        let features = self.report().features;
        let shapes = self.floating_shapes();
        self.with_render_model_and_hints(|model, source_hints| {
            render::to_pdf_with_fonts_and_report_and_shapes(
                model,
                &[],
                features,
                &shapes,
                source_hints,
            )
        })
    }

    /// Render this document to PDF with caller-supplied fonts and return
    /// renderer metrics/warnings produced by the same pagination pass. Uses the
    /// opened document's feature inventory for unsupported preserved constructs.
    /// Available with the `render` feature.
    #[cfg(feature = "render")]
    pub fn to_pdf_with_fonts_and_report(&self, fonts: &[Vec<u8>]) -> RenderedPdf {
        let features = self.report().features;
        let shapes = self.floating_shapes();
        self.with_render_model_and_hints(|model, source_hints| {
            render::to_pdf_with_fonts_and_report_and_shapes(
                model,
                fonts,
                features,
                &shapes,
                source_hints,
            )
        })
    }

    /// Fallible variant of [`Document::to_pdf_with_report`]. Available with the
    /// `render` feature.
    #[cfg(feature = "render")]
    pub fn try_to_pdf_with_report(&self) -> Result<RenderedPdf> {
        let features = self.report().features;
        let shapes = self.floating_shapes();
        self.with_render_model_and_hints(|model, source_hints| {
            render::try_to_pdf_with_fonts_and_report_and_shapes(
                model,
                &[],
                features,
                &shapes,
                source_hints,
            )
        })
    }

    /// Fallible variant of [`Document::to_pdf_with_fonts_and_report`].
    /// Available with the `render` feature.
    #[cfg(feature = "render")]
    pub fn try_to_pdf_with_fonts_and_report(&self, fonts: &[Vec<u8>]) -> Result<RenderedPdf> {
        let features = self.report().features;
        let shapes = self.floating_shapes();
        self.with_render_model_and_hints(|model, source_hints| {
            render::try_to_pdf_with_fonts_and_report_and_shapes(
                model,
                fonts,
                features,
                &shapes,
                source_hints,
            )
        })
    }

    #[cfg(feature = "render")]
    fn with_render_model_and_hints<R>(
        &self,
        render_document: impl FnOnce(&DocModel, render::SourceRenderHints<'_>) -> R,
    ) -> R {
        match &self.backend {
            Backend::Doc(d) => {
                let assembled = legacy_build_output_from_doc_state(d);
                render_document(
                    &assembled.model,
                    render::SourceRenderHints {
                        pagination: &assembled.pagination_hints,
                        line_spacing: &assembled.line_spacing_hints,
                        column_break_offsets: &assembled.column_break_offsets,
                        section_column_gap_pt: &assembled.section_column_gap_pt,
                        section_column_layouts: &assembled.section_column_layouts,
                        section_column_separators: &assembled.section_column_separators,
                        section_column_rtl: &assembled.section_column_rtl,
                        final_section_column_gap_pt: assembled.final_section_column_gap_pt,
                        final_section_column_layout: assembled.final_section_column_layout.as_ref(),
                        final_section_column_separator: assembled.final_section_column_separator,
                        final_section_column_rtl: assembled.final_section_column_rtl,
                        table_row_pagination: &assembled.table_row_pagination,
                        table_cell_pagination: &assembled.table_cell_pagination,
                        table_cell_line_spacing: &assembled.table_cell_line_spacing,
                        tab_stops: &assembled.render_tab_stops,
                        table_cell_tab_stops: &assembled.render_table_cell_tab_stops,
                        running_line_spacing: &assembled.running_line_spacing_hints,
                        running_tab_stops: &assembled.running_tab_stops,
                        running_table_cell_tab_stops: &assembled.running_table_cell_tab_stops,
                        running_surface_distances: &assembled.running_surface_distances,
                        ..render::SourceRenderHints::default()
                    },
                )
            }
            #[cfg(feature = "docx")]
            Backend::Docx(d) => {
                let mut model = d.model.clone();
                model.blocks.extend(d.notes.iter().cloned());
                let mut line_spacing = d.line_spacing_hints.clone();
                line_spacing.extend_from_slice(&d.note_line_spacing_hints);
                let mut tab_stops = d.tab_stops.clone();
                tab_stops.extend_from_slice(&d.note_tab_stops);
                let mut table_cell_tab_stops = d.table_cell_tab_stops.clone();
                table_cell_tab_stops.extend_from_slice(&d.note_table_cell_tab_stops);
                render_document(
                    &model,
                    render::SourceRenderHints {
                        pagination: &d.pagination_hints,
                        line_spacing: &line_spacing,
                        tab_stops: &tab_stops,
                        column_break_offsets: &d.column_break_offsets,
                        section_column_gap_pt: &d.section_column_gap_pt,
                        section_column_layouts: &d.section_column_layouts,
                        section_column_separators: &d.section_column_separators,
                        section_column_rtl: &d.section_column_rtl,
                        final_section_column_gap_pt: d.final_section_column_gap_pt,
                        final_section_column_layout: d.final_section_column_layout.as_ref(),
                        final_section_column_separator: d.final_section_column_separator,
                        final_section_column_rtl: d.final_section_column_rtl,
                        table_row_pagination: &d.table_row_pagination,
                        table_cell_pagination: &d.table_cell_pagination,
                        table_cell_line_spacing: &d.table_cell_line_spacing,
                        table_nested_pagination: &d.table_nested_pagination,
                        table_cell_tab_stops: &table_cell_tab_stops,
                        running_line_spacing: &d.running_line_spacing_hints,
                        running_tab_stops: &d.running_tab_stops,
                        running_table_cell_tab_stops: &d.running_table_cell_tab_stops,
                        running_surface_distances: &d.running_surface_distances,
                        default_tab_stop_pt: d.default_tab_stop_pt,
                    },
                )
            }
        }
    }

    /// Normalized plain text of the entire document (all sub-documents), with
    /// reconstructed list autonumbers (`.doc`) or model-derived text (`.docx`).
    pub fn text(&self) -> String {
        match &self.backend {
            Backend::Doc(d) => text::finalize(&d.labeled),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => d.text.clone(),
        }
    }

    /// Normalized text of just the main document body. For `.doc` this is
    /// derived from the model's `Main` source region; for `.docx` it is the body
    /// part, excluding the running headers/footers that [`Document::text`] also
    /// includes.
    pub fn main_text(&self) -> String {
        match &self.backend {
            Backend::Doc(_) => self.model().source_region_kind_text(SourceRegionKind::Main),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => d.main_text.clone(),
        }
    }

    /// Normalized footnote + endnote text. For `.doc`, this combines the exact
    /// `ccpFtn` and `ccpEdn` regions even though other subdocuments sit between
    /// them in the FIB CP stream; for `.docx`, this combines parsed footnote
    /// side-table records.
    pub fn footnote_text(&self) -> String {
        match &self.backend {
            Backend::Doc(_) => {
                let model = self.model();
                let mut text = model.source_region_kind_text(SourceRegionKind::Footnote);
                text.push_str(&model.source_region_kind_text(SourceRegionKind::Endnote));
                text
            }
            #[cfg(feature = "docx")]
            Backend::Docx(d) => note_kind_text(&d.note_records, NoteKind::Footnote),
        }
    }

    /// Normalized endnote text. `.doc` uses the model's `Endnote` source region;
    /// `.docx` uses parsed endnote side-table records.
    pub fn endnote_text(&self) -> String {
        match &self.backend {
            Backend::Doc(_) => self
                .model()
                .source_region_kind_text(SourceRegionKind::Endnote),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => note_kind_text(&d.note_records, NoteKind::Endnote),
        }
    }

    /// Normalized header/footer text. `.doc` uses the model's `HeaderFooter`
    /// source region; `.docx` flattens the running header/footer parts resolved
    /// from the section refs.
    pub fn header_text(&self) -> String {
        match &self.backend {
            Backend::Doc(_) => self
                .model()
                .source_region_kind_text(SourceRegionKind::HeaderFooter),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => crate::docx::header_footer_text(&d.model),
        }
    }

    /// Normalized annotation/comment subdocument text. `.doc` uses the model's
    /// `Annotation` source region; `.docx` comments are available through
    /// [`Document::comments`].
    pub fn annotation_text(&self) -> String {
        match &self.backend {
            Backend::Doc(_) => self
                .model()
                .source_region_kind_text(SourceRegionKind::Annotation),
            #[cfg(feature = "docx")]
            Backend::Docx(_) => String::new(),
        }
    }

    /// Normalized text-box text. `.doc` uses the model's `TextBox` source region;
    /// `.docx` uses parsed body/note/header/footer text-box side-table records.
    pub fn text_box_text(&self) -> String {
        match &self.backend {
            Backend::Doc(_) => self
                .model()
                .source_region_kind_text(SourceRegionKind::TextBox),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => text_box_records_text(&d.text_boxes),
        }
    }

    /// Total character count: the FIB CP space across all sub-documents (`.doc`)
    /// or the model's visible character count (`.docx`).
    pub fn char_count(&self) -> usize {
        match &self.backend {
            Backend::Doc(d) => d.fib.total_cp(),
            #[cfg(feature = "docx")]
            Backend::Docx(d) => d.model.meta.stats.text_chars,
        }
    }

    /// `true` if a `.doc` is "complex" (fast-saved). Always `false` for `.docx`.
    pub fn is_complex(&self) -> bool {
        match &self.backend {
            Backend::Doc(d) => d.fib.complex,
            #[cfg(feature = "docx")]
            Backend::Docx(_) => false,
        }
    }
}

#[cfg(feature = "docx")]
#[derive(Clone, Copy)]
enum AtomicBodyBlockEdit<'a> {
    InsertParagraph { block_index: usize, text: &'a str },
    Remove { block_index: usize },
    Move { from_index: usize, to_index: usize },
}

#[cfg(feature = "docx")]
fn ensure_docx_tree_editable(d: &docx::DocxState) -> Result<()> {
    // An incomplete package (an unreadable entry was dropped on open) cannot be
    // package-preserving-saved, so editable must remain equivalent to saveable.
    if !d.package.is_complete() {
        return Err(Error::Docx(
            "cannot edit: this document was opened with unreadable/dropped parts, so a \
             package-preserving save is impossible — re-acquire the source file"
                .into(),
        ));
    }
    if d.package.is_meta_lossy() {
        return Err(Error::Docx(
            "cannot edit: this document's OPC metadata ([Content_Types].xml or a \
             .rels part) is malformed, so an edit would regenerate it lossily — \
             re-acquire the source file"
                .into(),
        ));
    }
    Ok(())
}

#[cfg(feature = "docx")]
fn apply_atomic_body_block_edit(
    tree: &mut xmltree::XmlTree,
    edit: AtomicBodyBlockEdit<'_>,
) -> Result<()> {
    let body = tree.wml_body_strict()?;
    match edit {
        AtomicBodyBlockEdit::InsertParagraph { block_index, text } => {
            tree.insert_wml_plain_body_paragraph_under(body, block_index, text)
        }
        AtomicBodyBlockEdit::Remove { block_index } => {
            tree.remove_wml_atomic_body_block_under(body, block_index)
        }
        AtomicBodyBlockEdit::Move {
            from_index,
            to_index,
        } => tree.move_wml_atomic_body_block_under(body, from_index, to_index),
    }
}

#[cfg(feature = "docx")]
fn edit_docx_atomic_body_block(
    d: &mut docx::DocxState,
    edit: AtomicBodyBlockEdit<'_>,
) -> Result<()> {
    let raw = d
        .package
        .part("word/document.xml")
        .ok_or_else(|| Error::Docx("missing word/document.xml".into()))?;
    let mut probe = xmltree::XmlTree::parse(&raw)?;
    apply_atomic_body_block_edit(&mut probe, edit)?;
    if matches!(
        edit,
        AtomicBodyBlockEdit::Move {
            from_index,
            to_index
        } if from_index == to_index
    ) {
        return Ok(());
    }

    let mut pkg = d.package.clone();
    {
        let tree = pkg.part_tree_mut("word/document.xml")?;
        apply_atomic_body_block_edit(tree, edit)?;
    }
    if matches!(edit, AtomicBodyBlockEdit::Remove { .. }) {
        pkg.prune_unreferenced_media_relationships("word/document.xml")?;
    }
    pkg.ensure_content_type("word/document.xml", CT_DOCUMENT_MAIN);
    commit_docx_package(d, pkg)
}

#[cfg(feature = "docx")]
fn commit_docx_package(d: &mut docx::DocxState, pkg: opc::Package) -> Result<()> {
    pkg.validate_for_save()?;
    d.package = pkg;
    Ok(())
}

#[cfg(feature = "docx")]
fn refresh_docx_read_view(d: &mut docx::DocxState) -> Result<()> {
    let bytes = d.package.to_zip()?;
    let mut refreshed = docx::open(&bytes)?;
    std::mem::swap(&mut refreshed.package, &mut d.package);
    *d = refreshed;
    Ok(())
}

#[cfg(feature = "docx")]
fn note_kind_text(notes: &[Note], kind: NoteKind) -> String {
    let mut raw = String::new();
    for note in notes.iter().filter(|note| note.kind == kind) {
        raw.push_str(&note.text);
        raw.push('\n');
    }
    text::finalize(&raw)
}

#[cfg(feature = "docx")]
fn text_box_records_text(text_boxes: &[TextBox]) -> String {
    let mut raw = String::new();
    for text_box in text_boxes {
        raw.push_str(&text_box.text);
        raw.push('\n');
    }
    text::finalize(&raw)
}

fn legacy_doc_comments_from_state(state: &DocState) -> Vec<Comment> {
    let model = doc_model_from_doc_state(state);
    legacy_doc_comments_from_model_with_metadata(
        &model,
        &state.annotation_metadata,
        &state.annotation_owners,
    )
}

fn legacy_doc_comments_from_model_with_metadata(
    model: &DocModel,
    metadata: &[annotation::LegacyDocCommentMetadata],
    owners: &[String],
) -> Vec<Comment> {
    let mut comments = legacy_doc_comments_from_model_regions(model);
    if comments.len() < metadata.len() {
        comments = legacy_doc_comments_from_annotation_blocks(model, metadata.len());
    }
    for (comment, meta) in comments.iter_mut().zip(metadata) {
        comment.initials = meta
            .initials
            .clone()
            .filter(|initials| !initials.is_empty());
        comment.author = meta
            .owner_index
            .and_then(|index| owners.get(index))
            .filter(|owner| !owner.is_empty())
            .cloned();
    }
    comments
}

fn legacy_doc_comments_from_model_regions(model: &DocModel) -> Vec<Comment> {
    let mut comments = Vec::new();
    for region in model.source_regions(SourceRegionKind::Annotation) {
        let text = model.source_region_text(region);
        if text.is_empty() {
            continue;
        }
        let index = comments.len();
        comments.push(Comment {
            id: format!("legacy-doc-annotation-{index}"),
            anchor: Some(legacy_doc_region_anchor(
                "legacy-doc-annotation",
                index,
                region,
                &text,
            )),
            text,
            ..Comment::default()
        });
    }
    comments
}

fn legacy_doc_comments_from_annotation_blocks(
    model: &DocModel,
    target_count: usize,
) -> Vec<Comment> {
    let mut comments = Vec::new();
    for region in model.source_regions(SourceRegionKind::Annotation) {
        for block in model.source_region_blocks(region) {
            let text = legacy_doc_block_text(block);
            if text.is_empty() {
                continue;
            }
            let index = comments.len();
            comments.push(Comment {
                id: format!("legacy-doc-annotation-{index}"),
                anchor: Some(legacy_doc_region_anchor(
                    "legacy-doc-annotation",
                    index,
                    region,
                    &text,
                )),
                text,
                ..Comment::default()
            });
            if comments.len() == target_count {
                return comments;
            }
        }
    }
    comments
}

fn legacy_doc_block_text(block: &Block) -> String {
    match block {
        Block::Paragraph(paragraph) => paragraph.text(),
        Block::Table(table) => table
            .rows
            .iter()
            .flat_map(|row| row.cells.iter())
            .map(|cell| cell.text())
            .collect::<Vec<_>>()
            .join(""),
        Block::PageBreak => "\n".to_string(),
        Block::Image(_) | Block::Chart(_) | Block::SectionBreak(_) => String::new(),
    }
}

fn legacy_doc_notes_from_model(model: &DocModel) -> Vec<Note> {
    let mut notes = Vec::new();
    push_legacy_doc_notes(
        model,
        SourceRegionKind::Footnote,
        NoteKind::Footnote,
        "legacy-doc-footnote",
        &mut notes,
    );
    push_legacy_doc_notes(
        model,
        SourceRegionKind::Endnote,
        NoteKind::Endnote,
        "legacy-doc-endnote",
        &mut notes,
    );
    notes
}

fn legacy_doc_notes_from_state(state: &DocState) -> Vec<Note> {
    let model = doc_model_from_doc_state(state);
    let footnote_ref_cps = legacy_doc_note_reference_cps_from_state(state, FIB_FCLCB_PLCFFND_REF);
    let endnote_ref_cps = legacy_doc_note_reference_cps_from_state(state, FIB_FCLCB_PLCFEND_REF);
    if footnote_ref_cps.is_empty() && endnote_ref_cps.is_empty() {
        let mut notes = legacy_doc_notes_from_model(&model);
        attach_legacy_doc_footnote_marker_anchors(&mut notes, state);
        attach_legacy_doc_endnote_marker_anchors(&mut notes, state);
        return notes;
    }
    let mut notes = Vec::new();
    let footnote_exact = push_legacy_doc_notes_with_reference_anchors(
        &model,
        SourceRegionKind::Footnote,
        NoteKind::Footnote,
        "legacy-doc-footnote",
        &footnote_ref_cps,
        state,
        &mut notes,
    );
    let endnote_exact = push_legacy_doc_notes_with_reference_anchors(
        &model,
        SourceRegionKind::Endnote,
        NoteKind::Endnote,
        "legacy-doc-endnote",
        &endnote_ref_cps,
        state,
        &mut notes,
    );
    if !footnote_exact {
        attach_legacy_doc_footnote_marker_anchors(&mut notes, state);
    }
    if !endnote_exact {
        attach_legacy_doc_endnote_marker_anchors(&mut notes, state);
    }
    notes
}

#[cfg(feature = "docx")]
fn legacy_project_promoted_running_surfaces(assembled: &mut assemble::LegacyBuildOutput) -> bool {
    if assembled.promoted_running_block_ranges.is_empty() {
        return true;
    }

    let block_count = assembled.model.blocks.len();
    let aligned = assembled.pagination_hints.len() == block_count
        && assembled.line_spacing_hints.len() == block_count
        && assembled.tab_stops.len() == block_count
        && assembled.note_reference_anchors.len() == block_count
        && assembled.column_break_offsets.len() == block_count
        && assembled.table_cell_column_break_offsets.len() == block_count
        && assembled.section_column_gap_pt.len() == block_count
        && assembled.section_column_layouts.len() == block_count
        && assembled.section_column_separators.len() == block_count
        && assembled.section_column_rtl.len() == block_count
        && assembled.table_row_pagination.len() == block_count
        && assembled.table_cell_pagination.len() == block_count
        && assembled.table_cell_line_spacing.len() == block_count
        && assembled.table_cell_tab_stops.len() == block_count;
    #[cfg(feature = "render")]
    let aligned = aligned
        && assembled.render_tab_stops.len() == block_count
        && assembled.render_table_cell_tab_stops.len() == block_count;
    if !aligned {
        return false;
    }

    let mut ranges = assembled.promoted_running_block_ranges.clone();
    ranges.sort_unstable();
    if ranges
        .iter()
        .any(|&(start, end)| start >= end || end > block_count)
        || ranges.windows(2).any(|pair| pair[0].1 > pair[1].0)
    {
        return false;
    }
    if ranges.iter().any(|&(start, end)| {
        !assembled.model.regions.iter().any(|region| {
            region.kind == SourceRegionKind::HeaderFooter
                && region.block_start == start
                && region.block_end == end
        })
    }) {
        return false;
    }
    let regions = &assembled.model.regions;
    if regions
        .iter()
        .any(|region| region.block_start > region.block_end || region.block_end > block_count)
        || regions.windows(2).any(|pair| {
            pair[0].block_end > pair[1].block_start
                || pair[0].text_start.saturating_add(pair[0].text_len) > pair[1].text_start
        })
    {
        return false;
    }

    let mut removed = vec![false; block_count];
    for &(start, end) in &ranges {
        let Some(slots) = removed.get_mut(start..end) else {
            return false;
        };
        if slots.iter().any(|slot| *slot) {
            return false;
        }
        slots.fill(true);
    }
    for region in &assembled.model.regions {
        let intersects = removed[region.block_start..region.block_end]
            .iter()
            .any(|slot| *slot);
        let selected = region.kind == SourceRegionKind::HeaderFooter
            && ranges
                .binary_search(&(region.block_start, region.block_end))
                .is_ok();
        if intersects != selected {
            return false;
        }
    }

    let mut removed_prefix = vec![0usize; block_count.saturating_add(1)];
    for (index, slot) in removed.iter().enumerate() {
        removed_prefix[index + 1] = removed_prefix[index] + usize::from(*slot);
    }
    let mut text_start = 0usize;
    let mut remapped_regions = Vec::with_capacity(assembled.model.regions.len());
    for region in &assembled.model.regions {
        if region.kind == SourceRegionKind::HeaderFooter
            && ranges
                .binary_search(&(region.block_start, region.block_end))
                .is_ok()
        {
            continue;
        }
        let mut region = region.clone();
        region.block_start -= removed_prefix[region.block_start];
        region.block_end -= removed_prefix[region.block_end];
        region.text_start = text_start;
        text_start = text_start.saturating_add(region.text_len);
        remapped_regions.push(region);
    }

    retain_legacy_conversion_slots(&mut assembled.model.blocks, &removed);
    retain_legacy_conversion_slots(&mut assembled.pagination_hints, &removed);
    retain_legacy_conversion_slots(&mut assembled.line_spacing_hints, &removed);
    retain_legacy_conversion_slots(&mut assembled.tab_stops, &removed);
    #[cfg(feature = "render")]
    retain_legacy_conversion_slots(&mut assembled.render_tab_stops, &removed);
    retain_legacy_conversion_slots(&mut assembled.note_reference_anchors, &removed);
    retain_legacy_conversion_slots(&mut assembled.column_break_offsets, &removed);
    retain_legacy_conversion_slots(&mut assembled.table_cell_column_break_offsets, &removed);
    retain_legacy_conversion_slots(&mut assembled.section_column_gap_pt, &removed);
    retain_legacy_conversion_slots(&mut assembled.section_column_layouts, &removed);
    retain_legacy_conversion_slots(&mut assembled.section_column_separators, &removed);
    retain_legacy_conversion_slots(&mut assembled.section_column_rtl, &removed);
    retain_legacy_conversion_slots(&mut assembled.table_row_pagination, &removed);
    retain_legacy_conversion_slots(&mut assembled.table_cell_pagination, &removed);
    retain_legacy_conversion_slots(&mut assembled.table_cell_line_spacing, &removed);
    retain_legacy_conversion_slots(&mut assembled.table_cell_tab_stops, &removed);
    #[cfg(feature = "render")]
    retain_legacy_conversion_slots(&mut assembled.render_table_cell_tab_stops, &removed);
    assembled.model.regions = remapped_regions;
    assembled.model.meta.stats = assemble::compute_stats(&assembled.model.blocks);
    assembled.promoted_running_block_ranges.clear();
    true
}

#[cfg(feature = "docx")]
fn retain_legacy_conversion_slots<T>(values: &mut Vec<T>, removed: &[bool]) {
    let mut slots = removed.iter();
    values.retain(|_| slots.next().is_some_and(|removed| !removed));
}

#[cfg(feature = "docx")]
fn legacy_promote_exact_notes_for_fresh_conversion(
    state: &DocState,
    assembled: &mut assemble::LegacyBuildOutput,
) -> bool {
    let Some((model, body_block_end)) = legacy_model_with_exact_promoted_notes(state, assembled)
    else {
        return false;
    };

    assembled.model = model;
    assembled.pagination_hints.truncate(body_block_end);
    assembled.line_spacing_hints.truncate(body_block_end);
    assembled.tab_stops.truncate(body_block_end);
    #[cfg(feature = "render")]
    assembled.render_tab_stops.truncate(body_block_end);
    assembled.note_reference_anchors.truncate(body_block_end);
    assembled.column_break_offsets.truncate(body_block_end);
    assembled
        .table_cell_column_break_offsets
        .truncate(body_block_end);
    assembled.section_column_gap_pt.truncate(body_block_end);
    assembled.section_column_layouts.truncate(body_block_end);
    assembled.section_column_separators.truncate(body_block_end);
    assembled.section_column_rtl.truncate(body_block_end);
    assembled.table_row_pagination.truncate(body_block_end);
    assembled.table_cell_pagination.truncate(body_block_end);
    assembled.table_cell_line_spacing.truncate(body_block_end);
    assembled.table_cell_tab_stops.truncate(body_block_end);
    #[cfg(feature = "render")]
    assembled
        .render_table_cell_tab_stops
        .truncate(body_block_end);
    true
}

#[cfg(feature = "docx")]
fn legacy_model_with_exact_promoted_notes(
    state: &DocState,
    assembled: &assemble::LegacyBuildOutput,
) -> Option<(DocModel, usize)> {
    if [SourceRegionKind::Annotation, SourceRegionKind::TextBox]
        .into_iter()
        .any(|kind| legacy_region_kind_has_text(&assembled.model, kind))
    {
        return None;
    }

    let footnote_refs = legacy_doc_note_reference_cps_from_state(state, FIB_FCLCB_PLCFFND_REF);
    let endnote_refs = legacy_doc_note_reference_cps_from_state(state, FIB_FCLCB_PLCFEND_REF);
    if footnote_refs.is_empty() && endnote_refs.is_empty() {
        return None;
    }

    let mut expected = legacy_exact_note_family(
        &assembled.model,
        SourceRegionKind::Footnote,
        NoteKind::Footnote,
        "legacy-doc-footnote",
        &footnote_refs,
    )?;
    expected.extend(legacy_exact_note_family(
        &assembled.model,
        SourceRegionKind::Endnote,
        NoteKind::Endnote,
        "legacy-doc-endnote",
        &endnote_refs,
    )?);
    if expected.is_empty() {
        return None;
    }
    expected.sort_by_key(|(source_cp, _)| *source_cp);
    if expected.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return None;
    }

    let body_block_end = assembled
        .model
        .source_regions(SourceRegionKind::Main)
        .map(|region| region.block_end)
        .max()?
        .min(assembled.model.blocks.len());
    if body_block_end == 0 || assembled.note_reference_anchors.len() != assembled.model.blocks.len()
    {
        return None;
    }

    let mut captured = assembled
        .note_reference_anchors
        .iter()
        .enumerate()
        .flat_map(|(block_index, anchors)| {
            anchors
                .iter()
                .map(move |anchor| (anchor.source_cp, block_index, anchor.text_offset))
        })
        .collect::<Vec<_>>();
    captured.sort_by_key(|(source_cp, _, _)| *source_cp);
    if captured.len() != expected.len() || captured.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return None;
    }

    let mut notes_by_block = (0..body_block_end)
        .map(|_| Vec::<(usize, AuthoredNote)>::new())
        .collect::<Vec<_>>();
    for ((expected_cp, note), (captured_cp, block_index, text_offset)) in
        expected.into_iter().zip(captured)
    {
        if expected_cp != captured_cp || block_index >= body_block_end {
            return None;
        }
        notes_by_block[block_index].push((text_offset, note));
    }

    let mut model = assembled.model.clone();
    for (block_index, notes) in notes_by_block.into_iter().enumerate() {
        if notes.is_empty() {
            continue;
        }
        let Some(Block::Paragraph(paragraph)) = model.blocks.get_mut(block_index) else {
            return None;
        };
        if !attach_authored_notes_to_paragraph(paragraph, notes) {
            return None;
        }
    }

    model.blocks.truncate(body_block_end);
    model.regions.retain(|region| {
        region.kind == SourceRegionKind::Main && region.block_end <= body_block_end
    });
    model.meta.stats = assemble::compute_stats(&model.blocks);
    Some((model, body_block_end))
}

#[cfg(feature = "docx")]
fn legacy_exact_note_family(
    model: &DocModel,
    region_kind: SourceRegionKind,
    note_kind: NoteKind,
    id_prefix: &str,
    reference_cps: &[usize],
) -> Option<Vec<(usize, AuthoredNote)>> {
    let has_note_text = legacy_region_kind_has_text(model, region_kind);
    if reference_cps.is_empty() {
        return (!has_note_text).then(Vec::new);
    }
    if !has_note_text {
        return None;
    }
    let records = legacy_doc_note_records_for_reference_count(
        model,
        region_kind,
        note_kind,
        id_prefix,
        reference_cps.len(),
    )?;
    if records.len() != reference_cps.len() {
        return None;
    }
    reference_cps
        .iter()
        .copied()
        .zip(records)
        .map(|(source_cp, note)| {
            (!note.text.is_empty()).then_some((
                source_cp,
                AuthoredNote {
                    kind: note.kind,
                    text: note.text,
                },
            ))
        })
        .collect()
}

#[cfg(feature = "docx")]
fn legacy_region_kind_has_text(model: &DocModel, kind: SourceRegionKind) -> bool {
    model
        .source_regions(kind)
        .any(|region| !model.source_region_text(region).is_empty())
}

#[cfg(feature = "docx")]
pub(crate) fn attach_authored_notes_to_paragraph(
    paragraph: &mut Paragraph,
    mut notes: Vec<(usize, AuthoredNote)>,
) -> bool {
    notes.sort_by_key(|(offset, _)| *offset);
    if notes.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return false;
    }
    let text_chars = paragraph
        .runs
        .iter()
        .map(|run| run.text.chars().count())
        .sum::<usize>();
    if notes.last().is_some_and(|(offset, _)| *offset > text_chars) {
        return false;
    }

    for (offset, note) in notes.into_iter().rev() {
        if !attach_authored_note_at_text_offset(paragraph, offset, note) {
            return false;
        }
    }
    true
}

#[cfg(feature = "docx")]
fn attach_authored_note_at_text_offset(
    paragraph: &mut Paragraph,
    offset: usize,
    note: AuthoredNote,
) -> bool {
    let mut char_start = 0usize;
    let mut target = None;
    for (run_index, run) in paragraph.runs.iter().enumerate() {
        let run_chars = run.text.chars().count();
        let char_end = char_start.saturating_add(run_chars);
        if run_chars > 0 && offset > char_start && offset <= char_end {
            target = Some((run_index, offset - char_start, run_chars));
            break;
        }
        if offset == 0 && run_chars > 0 {
            target = Some((run_index, 0, run_chars));
            break;
        }
        char_start = char_end;
    }
    let Some((run_index, local_offset, run_chars)) = target else {
        return false;
    };
    let run = &paragraph.runs[run_index];
    if run.image.is_some()
        || !matches!(run.field, FieldRole::None)
        || run.comment.is_some()
        || run.revision.is_some()
        || run.content_control.is_some()
        || run.bookmark.is_some()
        || (local_offset == run_chars && run.note.is_some())
    {
        return false;
    }

    let split_byte = if local_offset == run_chars {
        run.text.len()
    } else {
        let Some((byte, _)) = run.text.char_indices().nth(local_offset) else {
            return false;
        };
        byte
    };
    let mut before = run.clone();
    let mut after = run.clone();
    before.text = run.text[..split_byte].to_string();
    before.note = Some(note);
    after.text = run.text[split_byte..].to_string();
    let mut replacement = vec![before];
    if !after.text.is_empty() {
        replacement.push(after);
    }
    paragraph.runs.splice(run_index..=run_index, replacement);
    true
}

fn legacy_doc_text_boxes_from_model(model: &DocModel) -> Vec<TextBox> {
    model
        .source_regions(SourceRegionKind::TextBox)
        .enumerate()
        .filter_map(|(index, region)| {
            let text = model.source_region_text(region);
            (!text.is_empty()).then(|| TextBox {
                id: format!("legacy-doc-text-box-{index}"),
                anchor: Some(legacy_doc_region_anchor(
                    "legacy-doc-text-box",
                    index,
                    region,
                    &text,
                )),
                text,
            })
        })
        .collect()
}

fn legacy_doc_text_boxes_from_state(state: &DocState) -> Vec<TextBox> {
    let model = doc_model_from_doc_state(state);
    let shape_anchor_cps = legacy_doc_shape_anchor_cps_from_state(state);
    if !shape_anchor_cps.is_empty() {
        let anchors = legacy_doc_body_cp_anchors(state, &shape_anchor_cps);
        if anchors.len() == shape_anchor_cps.len() {
            if let Some(mut text_boxes) =
                legacy_doc_text_boxes_for_anchor_count(&model, anchors.len())
            {
                for (text_box, anchor) in text_boxes.iter_mut().zip(anchors) {
                    text_box.anchor = Some(TextAnchor {
                        id: format!("{}@body-cp{}", text_box.id, anchor.source_cp),
                        text: anchor.text,
                    });
                }
                return text_boxes;
            }
        }
    }
    legacy_doc_text_boxes_from_model(&model)
}

fn legacy_doc_text_boxes_for_anchor_count(
    model: &DocModel,
    anchor_count: usize,
) -> Option<Vec<TextBox>> {
    if anchor_count == 0 {
        return None;
    }
    if anchor_count == 1 {
        let text_boxes = legacy_doc_text_boxes_from_model(model);
        return (text_boxes.len() == 1).then_some(text_boxes);
    }

    let mut text_boxes = Vec::new();
    for region in model.source_regions(SourceRegionKind::TextBox) {
        for block in model.source_region_blocks(region) {
            let text = legacy_doc_block_text(block);
            if text.is_empty() {
                continue;
            }
            let index = text_boxes.len();
            text_boxes.push(TextBox {
                id: format!("legacy-doc-text-box-{index}"),
                anchor: Some(legacy_doc_region_anchor(
                    "legacy-doc-text-box",
                    index,
                    region,
                    &text,
                )),
                text,
            });
        }
    }
    (text_boxes.len() == anchor_count).then_some(text_boxes)
}

fn legacy_doc_header_footers_from_model(model: &DocModel) -> Vec<HeaderFooter> {
    let mut records = Vec::new();
    for region in model.source_regions(SourceRegionKind::HeaderFooter) {
        let text = model.source_region_text(region);
        if text.is_empty() {
            continue;
        }
        records.push(HeaderFooter {
            id: format!("legacy-doc-header-footer-{}", records.len()),
            kind: legacy_doc_header_footer_kind(region.source_story_index),
            section: legacy_doc_header_footer_section(region.source_story_index),
            text,
        });
    }
    records
}

fn legacy_doc_header_footer_section(story_index: Option<usize>) -> Option<usize> {
    story_index?.checked_sub(6).map(|index| index / 6)
}

fn legacy_doc_header_footer_kind(story_index: Option<usize>) -> HeaderFooterKind {
    let Some(story_index) = story_index else {
        return HeaderFooterKind::Unknown;
    };
    let Some(position) = story_index.checked_sub(6).map(|index| index % 6) else {
        return HeaderFooterKind::Unknown;
    };
    match position {
        0 => HeaderFooterKind::EvenPageHeader,
        1 => HeaderFooterKind::OddPageHeader,
        2 => HeaderFooterKind::EvenPageFooter,
        3 => HeaderFooterKind::OddPageFooter,
        4 => HeaderFooterKind::FirstPageHeader,
        _ => HeaderFooterKind::FirstPageFooter,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LegacyDocBodyCpAnchor {
    source_cp: usize,
    text: String,
}

const FIB_FCLCB_PLCFFND_REF: usize = 2;
const FIB_FCLCB_PLCSPA_MOM: usize = 40;
const FIB_FCLCB_PLCFEND_REF: usize = 46;

fn legacy_doc_note_reference_cps_from_state(state: &DocState, fclcb_index: usize) -> Vec<usize> {
    let Some((fc, lcb)) = fib::fc_lcb_pair(&state.word, fclcb_index) else {
        return Vec::new();
    };
    annotation::legacy_doc_note_reference_cps(&state.table, fc, lcb, state.fib.ccp_text)
}

fn legacy_doc_shape_anchor_cps_from_state(state: &DocState) -> Vec<usize> {
    let Some((fc, lcb)) = fib::fc_lcb_pair(&state.word, FIB_FCLCB_PLCSPA_MOM) else {
        return Vec::new();
    };
    annotation::legacy_doc_shape_anchor_cps(&state.table, fc, lcb, state.fib.ccp_text)
}

fn push_legacy_doc_notes_with_reference_anchors(
    model: &DocModel,
    region_kind: SourceRegionKind,
    note_kind: NoteKind,
    id_prefix: &str,
    reference_cps: &[usize],
    state: &DocState,
    out: &mut Vec<Note>,
) -> bool {
    if !reference_cps.is_empty() {
        let anchors = legacy_doc_body_cp_anchors(state, reference_cps);
        if anchors.len() == reference_cps.len() {
            if let Some(mut notes) = legacy_doc_note_records_for_reference_count(
                model,
                region_kind,
                note_kind,
                id_prefix,
                anchors.len(),
            ) {
                for (note, anchor) in notes.iter_mut().zip(anchors) {
                    note.anchor = Some(TextAnchor {
                        id: format!("{}@body-cp{}", note.id, anchor.source_cp),
                        text: anchor.text,
                    });
                }
                out.extend(notes);
                return true;
            }
        }
    }
    push_legacy_doc_notes(model, region_kind, note_kind, id_prefix, out);
    false
}

fn legacy_doc_note_records_for_reference_count(
    model: &DocModel,
    region_kind: SourceRegionKind,
    note_kind: NoteKind,
    id_prefix: &str,
    reference_count: usize,
) -> Option<Vec<Note>> {
    if reference_count == 0 {
        return None;
    }
    if reference_count == 1 {
        let mut notes = Vec::new();
        push_legacy_doc_notes(model, region_kind, note_kind, id_prefix, &mut notes);
        return (notes.len() == 1).then_some(notes);
    }

    let mut notes = Vec::new();
    for region in model.source_regions(region_kind) {
        for block in model.source_region_blocks(region) {
            let text = legacy_doc_block_text(block);
            if text.is_empty() {
                continue;
            }
            let index = notes.len();
            notes.push(Note {
                id: format!("{id_prefix}-{index}"),
                kind: note_kind,
                anchor: Some(legacy_doc_region_anchor(id_prefix, index, region, &text)),
                text,
            });
        }
    }
    (notes.len() == reference_count).then_some(notes)
}

fn legacy_doc_body_cp_anchors(
    state: &DocState,
    source_cps: &[usize],
) -> Vec<LegacyDocBodyCpAnchor> {
    if source_cps.is_empty() {
        return Vec::new();
    }
    let (units, _) = assemble::decode_with_fc(&state.word, &state.pieces, state.enc);
    let main_len = (state.fib.ccp_text as usize).min(units.len());
    let main_units = &units[..main_len];
    let mut anchors = Vec::with_capacity(source_cps.len());
    for &source_cp in source_cps {
        if source_cp >= main_units.len() {
            return Vec::new();
        }
        let Some(text) = legacy_doc_body_cp_anchor_text(main_units, source_cp) else {
            return Vec::new();
        };
        anchors.push(LegacyDocBodyCpAnchor { source_cp, text });
    }
    anchors
}

fn attach_legacy_doc_footnote_marker_anchors(notes: &mut [Note], state: &DocState) {
    if notes.iter().any(|note| note.kind == NoteKind::Endnote) {
        return;
    }
    let mut footnotes: Vec<_> = notes
        .iter_mut()
        .filter(|note| note.kind == NoteKind::Footnote)
        .collect();
    attach_legacy_doc_body_marker_anchors_to_notes(&mut footnotes, state, 0x0002);
}

fn attach_legacy_doc_endnote_marker_anchors(notes: &mut [Note], state: &DocState) {
    if notes.iter().any(|note| note.kind == NoteKind::Footnote) {
        return;
    }
    let mut endnotes: Vec<_> = notes
        .iter_mut()
        .filter(|note| note.kind == NoteKind::Endnote)
        .collect();
    attach_legacy_doc_body_marker_anchors_to_notes(&mut endnotes, state, 0x0002);
}

fn attach_legacy_doc_body_marker_anchors_to_notes(
    notes: &mut [&mut Note],
    state: &DocState,
    marker: u16,
) {
    if notes.is_empty() {
        return;
    }
    let anchors = legacy_doc_body_marker_anchors(state, marker);
    if anchors.len() != notes.len() {
        return;
    }
    for (note, anchor) in notes.iter_mut().zip(anchors) {
        note.anchor = Some(TextAnchor {
            id: format!("{}@body-cp{}", note.id, anchor.source_cp),
            text: anchor.text,
        });
    }
}

fn legacy_doc_body_marker_anchors(state: &DocState, marker: u16) -> Vec<LegacyDocBodyCpAnchor> {
    let (units, _) = assemble::decode_with_fc(&state.word, &state.pieces, state.enc);
    let main_len = (state.fib.ccp_text as usize).min(units.len());
    legacy_doc_body_marker_anchors_from_units(&units[..main_len], marker)
}

fn legacy_doc_body_marker_anchors_from_units(
    units: &[u16],
    marker: u16,
) -> Vec<LegacyDocBodyCpAnchor> {
    units
        .iter()
        .enumerate()
        .filter_map(|(source_cp, unit)| {
            if *unit != marker {
                return None;
            }
            legacy_doc_body_cp_anchor_text(units, source_cp)
                .map(|text| LegacyDocBodyCpAnchor { source_cp, text })
        })
        .collect()
}

fn legacy_doc_body_cp_anchor_text(units: &[u16], source_cp: usize) -> Option<String> {
    if source_cp >= units.len() {
        return None;
    }
    let start = (0..source_cp)
        .rev()
        .find(|index| legacy_doc_body_marker_context_boundary(units[*index]))
        .map_or(0, |index| index + 1);
    let end = (source_cp + 1..units.len())
        .find(|index| legacy_doc_body_marker_context_boundary(units[*index]))
        .unwrap_or(units.len());
    let raw = String::from_utf16_lossy(&units[start..end]);
    let text = text::finalize(&raw);
    (!text.is_empty()).then_some(text)
}

fn legacy_doc_body_marker_context_boundary(unit: u16) -> bool {
    matches!(unit, 0x0007 | 0x000D)
}

fn push_legacy_doc_notes(
    model: &DocModel,
    region_kind: SourceRegionKind,
    note_kind: NoteKind,
    id_prefix: &str,
    out: &mut Vec<Note>,
) {
    let mut index = 0usize;
    for region in model.source_regions(region_kind) {
        let text = model.source_region_text(region);
        if text.is_empty() {
            continue;
        }
        out.push(Note {
            id: format!("{id_prefix}-{index}"),
            kind: note_kind,
            anchor: Some(legacy_doc_region_anchor(id_prefix, index, region, &text)),
            text,
        });
        index += 1;
    }
}

fn legacy_doc_region_anchor(
    id_prefix: &str,
    index: usize,
    region: &SourceRegion,
    text: &str,
) -> TextAnchor {
    TextAnchor {
        id: format!(
            "{id_prefix}-{index}@cp{}+{}",
            region.source_start_cp, region.source_len_cp
        ),
        text: text.to_string(),
    }
}

#[cfg(feature = "docx")]
impl Default for Document {
    /// Equivalent to [`Document::new`] — a blank `.docx`-backed document.
    fn default() -> Self {
        Self::new()
    }
}

/// The WordprocessingML main-document content type — what `word/document.xml` must be
/// typed as for Word to open the package. An element-tree edit ensures this override.
#[cfg(feature = "docx")]
const CT_DOCUMENT_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

#[cfg(feature = "docx")]
const CT_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";

#[cfg(feature = "docx")]
const CT_CORE_PROPERTIES: &str = "application/vnd.openxmlformats-package.core-properties+xml";

#[cfg(feature = "docx")]
const CT_FOOTNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
#[cfg(feature = "docx")]
const CT_ENDNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml";

#[cfg(feature = "docx")]
const CT_HEADER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
#[cfg(feature = "docx")]
const CT_FOOTER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";
#[cfg(feature = "docx")]
const CT_IMAGE_PNG: &str = "image/png";
#[cfg(feature = "docx")]
const CT_IMAGE_JPEG: &str = "image/jpeg";
#[cfg(feature = "docx")]
const CT_IMAGE_GIF: &str = "image/gif";
#[cfg(feature = "docx")]
const CT_IMAGE_BMP: &str = "image/bmp";
#[cfg(feature = "docx")]
const CT_IMAGE_TIFF: &str = "image/tiff";
#[cfg(feature = "docx")]
const CT_IMAGE_WEBP: &str = "image/webp";

#[cfg(feature = "docx")]
const REL_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
#[cfg(feature = "docx")]
const REL_FOOTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
#[cfg(feature = "docx")]
const REL_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
#[cfg(feature = "docx")]
const REL_FOOTNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
#[cfg(feature = "docx")]
const REL_ENDNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
#[cfg(feature = "docx")]
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
#[cfg(feature = "docx")]
const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
#[cfg(feature = "docx")]
const REL_CORE_PROPERTIES: &str =
    "http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties";

#[cfg(feature = "docx")]
const CORE_PROPERTIES_NS: &[u8] =
    b"http://schemas.openxmlformats.org/package/2006/metadata/core-properties";
#[cfg(feature = "docx")]
const DC_NS: &[u8] = b"http://purl.org/dc/elements/1.1/";
#[cfg(feature = "docx")]
const DCTERMS_NS: &[u8] = b"http://purl.org/dc/terms/";
#[cfg(feature = "docx")]
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";
#[cfg(feature = "docx")]
const DCTERMS_W3CDTF_ATTRS: &[(&str, &str)] =
    &[("xmlns:xsi", XSI_NS), ("xsi:type", "dcterms:W3CDTF")];

#[cfg(feature = "docx")]
#[derive(Clone, Debug)]
struct HeaderFooterTarget {
    part: String,
    root_local: &'static [u8],
    content_type: &'static str,
}

#[cfg(feature = "docx")]
#[derive(Clone, Copy, Debug)]
struct NotePartTarget {
    part: &'static str,
    root_local: &'static [u8],
    note_local: &'static [u8],
    content_type: &'static str,
}

#[cfg(feature = "docx")]
#[derive(Clone, Debug)]
struct StoryTemplateTarget {
    part: String,
    root_local: &'static [u8],
    note_local: Option<&'static [u8]>,
    content_type: &'static str,
}

#[cfg(feature = "docx")]
#[derive(Clone, Debug)]
struct FieldStoryTarget {
    part: String,
    root_local: &'static [u8],
    note_local: Option<&'static [u8]>,
    content_type: &'static str,
    body: bool,
}

#[cfg(feature = "docx")]
impl FieldStoryTarget {
    fn body() -> Self {
        Self {
            part: "word/document.xml".to_string(),
            root_local: b"document",
            note_local: None,
            content_type: CT_DOCUMENT_MAIN,
            body: true,
        }
    }
}

#[cfg(feature = "docx")]
#[derive(Clone, Debug)]
struct StoryTemplateMatch {
    target: StoryTemplateTarget,
    content_count: usize,
    fields: Vec<(usize, usize)>,
}

#[cfg(feature = "docx")]
impl From<HeaderFooterTarget> for StoryTemplateTarget {
    fn from(target: HeaderFooterTarget) -> Self {
        Self {
            part: target.part,
            root_local: target.root_local,
            note_local: None,
            content_type: target.content_type,
        }
    }
}

#[cfg(feature = "docx")]
impl From<NotePartTarget> for StoryTemplateTarget {
    fn from(target: NotePartTarget) -> Self {
        Self {
            part: target.part.to_string(),
            root_local: target.root_local,
            note_local: Some(target.note_local),
            content_type: target.content_type,
        }
    }
}

#[cfg(feature = "docx")]
fn wml_text_needs_run_markers(text: &str) -> bool {
    text.contains('\t') || text.contains('\n')
}

#[cfg(feature = "docx")]
fn wml_replacement_needs_space_attr_preflight(text: &str) -> bool {
    !wml_text_needs_run_markers(text) && text != text.trim_matches([' ', '\t', '\n', '\r'])
}

#[cfg(feature = "docx")]
fn wml_single_text_run_replacement_new_nodes(
    tree: &xmltree::XmlTree,
    runs: &[xmltree::NodeId],
    text: &str,
) -> Result<usize> {
    if wml_text_needs_run_markers(text) {
        Ok(xmltree::wml_text_run_content_node_count(text)?.saturating_mul(runs.len()))
    } else {
        Ok(runs
            .iter()
            .filter(|&&id| !tree.has_text_carrier(id))
            .count())
    }
}

#[cfg(feature = "docx")]
fn wml_grouped_text_run_replacement_new_nodes(
    tree: &xmltree::XmlTree,
    runs: &[xmltree::NodeId],
    text: &str,
) -> Result<usize> {
    if wml_text_needs_run_markers(text) {
        Ok(
            xmltree::wml_text_run_content_node_count(text)?.saturating_add(
                runs.iter()
                    .skip(1)
                    .filter(|&&id| !tree.has_text_carrier(id))
                    .count(),
            ),
        )
    } else {
        Ok(runs
            .iter()
            .filter(|&&id| !tree.has_text_carrier(id))
            .count())
    }
}

#[cfg(feature = "docx")]
fn wml_grouped_template_text_replacement_new_nodes(
    tree: &xmltree::XmlTree,
    group: &xmltree::WmlTextRunGroup,
    text: &str,
) -> Result<usize> {
    if wml_text_needs_run_markers(text) {
        Ok(
            xmltree::wml_anchored_text_run_content_node_count(text)?.saturating_add(
                group
                    .replacement_text_runs()
                    .skip(1)
                    .filter(|&id| !tree.has_text_carrier(id))
                    .count(),
            ),
        )
    } else {
        Ok(group
            .replacement_text_runs()
            .filter(|&id| !tree.has_text_carrier(id))
            .count())
    }
}

#[cfg(feature = "docx")]
#[derive(Clone, Copy)]
enum RevisionEditMode {
    Accept,
    Reject,
}

#[cfg(feature = "docx")]
#[derive(Clone)]
struct RevisionEditTarget {
    part: String,
    root_local: Option<&'static [u8]>,
    content_type: &'static str,
}

#[cfg(feature = "docx")]
fn edit_docx_revisions(d: &mut docx::DocxState, mode: RevisionEditMode) -> Result<usize> {
    let targets = revision_edit_targets(&d.package, mode)?;
    let mut changed = Vec::new();
    let mut total = 0usize;
    for target in targets {
        let raw = d
            .package
            .part(&target.part)
            .ok_or_else(|| Error::Docx(format!("missing {}", target.part)))?;
        let mut probe = xmltree::XmlTree::parse(&raw)?;
        let root = revision_edit_root(&probe, &target)?;
        let count = apply_revision_edit(&mut probe, root, mode);
        total += count;
        if count > 0 {
            changed.push(target);
        }
    }
    if total == 0 {
        return Ok(0);
    }

    let mut pkg = d.package.clone();
    for target in changed {
        let tree = pkg.part_tree_mut(&target.part)?;
        let root = revision_edit_root(tree, &target)?;
        apply_revision_edit(tree, root, mode);
        pkg.ensure_content_type(target.part.as_str(), target.content_type);
    }
    commit_docx_package(d, pkg)?;
    Ok(total)
}

#[cfg(feature = "docx")]
fn revision_edit_targets(
    package: &opc::Package,
    mode: RevisionEditMode,
) -> Result<Vec<RevisionEditTarget>> {
    if !package.has_part("word/document.xml") {
        return Err(Error::Docx("missing word/document.xml".into()));
    }
    let mut targets = vec![RevisionEditTarget {
        part: "word/document.xml".to_string(),
        root_local: None,
        content_type: CT_DOCUMENT_MAIN,
    }];
    for (part, root_local, content_type) in [
        ("word/footnotes.xml", &b"footnotes"[..], CT_FOOTNOTES),
        ("word/endnotes.xml", &b"endnotes"[..], CT_ENDNOTES),
    ] {
        if package.has_part(part) {
            targets.push(RevisionEditTarget {
                part: part.to_string(),
                root_local: Some(root_local),
                content_type,
            });
        }
    }
    for target in header_footer_targets_for_revision_edit(package, mode) {
        targets.push(RevisionEditTarget {
            part: target.part,
            root_local: Some(target.root_local),
            content_type: target.content_type,
        });
    }
    Ok(targets)
}

#[cfg(feature = "docx")]
fn revision_edit_root(
    tree: &xmltree::XmlTree,
    target: &RevisionEditTarget,
) -> Result<xmltree::NodeId> {
    match target.root_local {
        Some(root) => tree.wml_part_root_strict(&target.part, root),
        None => tree.wml_body_strict(),
    }
}

#[cfg(feature = "docx")]
fn apply_revision_edit(
    tree: &mut xmltree::XmlTree,
    root: xmltree::NodeId,
    mode: RevisionEditMode,
) -> usize {
    match mode {
        RevisionEditMode::Accept => tree.accept_wml_revisions_under(root),
        RevisionEditMode::Reject => tree.reject_wml_revisions_under(root),
    }
}

#[cfg(feature = "docx")]
fn set_wml_text_runs<I>(tree: &mut xmltree::XmlTree, runs: I, text: &str) -> Result<()>
where
    I: IntoIterator<Item = xmltree::NodeId>,
{
    let needs_markers = wml_text_needs_run_markers(text);
    for (i, id) in runs.into_iter().enumerate() {
        if i == 0 && needs_markers {
            tree.replace_wml_text_element_with_run_content(id, text)?;
        } else {
            tree.set_element_text(id, if i == 0 { text } else { "" })?;
        }
    }
    Ok(())
}

#[cfg(feature = "docx")]
fn set_wml_template_text_runs<I>(tree: &mut xmltree::XmlTree, runs: I, text: &str) -> Result<()>
where
    I: IntoIterator<Item = xmltree::NodeId>,
{
    let needs_markers = wml_text_needs_run_markers(text);
    for (index, id) in runs.into_iter().enumerate() {
        if index == 0 && needs_markers {
            tree.replace_wml_text_element_with_anchored_run_content(id, text)?;
        } else {
            tree.set_element_text(id, if index == 0 { text } else { "" })?;
        }
    }
    Ok(())
}

#[cfg(feature = "docx")]
fn header_footer_targets(package: &opc::Package) -> Vec<HeaderFooterTarget> {
    let Some(document_xml) = package.part("word/document.xml") else {
        return Vec::new();
    };
    let document_xml = String::from_utf8_lossy(&document_xml);
    let referenced = docx::header_footer_ref_ids(&document_xml);
    header_footer_targets_for_ids(package, &referenced)
}

#[cfg(feature = "docx")]
fn header_footer_targets_for_revision_edit(
    package: &opc::Package,
    mode: RevisionEditMode,
) -> Vec<HeaderFooterTarget> {
    let Some(document_xml) = package.part("word/document.xml") else {
        return Vec::new();
    };
    let document_xml = String::from_utf8_lossy(&document_xml);
    let referenced = match mode {
        RevisionEditMode::Accept => docx::header_footer_ref_ids(&document_xml),
        RevisionEditMode::Reject => docx::header_footer_ref_ids_for_revision_reject(&document_xml),
    };
    header_footer_targets_for_ids(package, &referenced)
}

#[cfg(feature = "docx")]
fn header_footer_targets_for_ids(
    package: &opc::Package,
    referenced: &std::collections::HashSet<String>,
) -> Vec<HeaderFooterTarget> {
    let mut seen = std::collections::HashSet::new();
    let mut targets = Vec::new();
    for rel in package.rels_for("word/document.xml") {
        if rel.external {
            continue;
        }
        if !referenced.contains(&rel.id) {
            continue;
        }
        let (root_local, content_type) = match rel.rel_type.as_str() {
            REL_HEADER => (b"hdr".as_slice(), CT_HEADER),
            REL_FOOTER => (b"ftr".as_slice(), CT_FOOTER),
            _ => continue,
        };
        let part = opc::resolve_rel_target("word/document.xml", &rel.target);
        if seen.insert(part.to_ascii_lowercase()) {
            targets.push(HeaderFooterTarget {
                part,
                root_local,
                content_type,
            });
        }
    }
    targets
}

#[cfg(feature = "docx")]
fn note_part_targets() -> [NotePartTarget; 2] {
    [
        NotePartTarget {
            part: "word/footnotes.xml",
            root_local: b"footnotes",
            note_local: b"footnote",
            content_type: CT_FOOTNOTES,
        },
        NotePartTarget {
            part: "word/endnotes.xml",
            root_local: b"endnotes",
            note_local: b"endnote",
            content_type: CT_ENDNOTES,
        },
    ]
}

#[cfg(feature = "docx")]
fn story_template_roots(
    tree: &xmltree::XmlTree,
    target: &StoryTemplateTarget,
    root: xmltree::NodeId,
) -> Vec<xmltree::NodeId> {
    target.note_local.map_or_else(
        || vec![root],
        |note_local| tree.wml_real_note_entries_under(root, note_local),
    )
}

#[cfg(feature = "docx")]
fn story_field_instructions(tree: &xmltree::XmlTree, roots: &[xmltree::NodeId]) -> Vec<String> {
    roots
        .iter()
        .flat_map(|&root| tree.wml_field_instructions_under(root))
        .collect()
}

#[cfg(feature = "docx")]
fn story_field_result_runs(
    tree: &xmltree::XmlTree,
    roots: &[xmltree::NodeId],
    field_index: usize,
) -> Option<Vec<xmltree::NodeId>> {
    story_field_result_text_group(tree, roots, field_index)
        .map(xmltree::WmlTextRunGroup::into_text_runs)
}

#[cfg(feature = "docx")]
fn story_field_result_text_group(
    tree: &xmltree::XmlTree,
    roots: &[xmltree::NodeId],
    mut field_index: usize,
) -> Option<xmltree::WmlTextRunGroup> {
    for &root in roots {
        let field_count = tree.wml_field_instructions_under(root).len();
        if field_index < field_count {
            return tree.wml_field_result_text_group_under(root, field_index);
        }
        field_index -= field_count;
    }
    None
}

#[cfg(feature = "docx")]
fn explicit_field_story_target(
    package: &opc::Package,
    part_name: &str,
    caller: &str,
) -> Result<FieldStoryTarget> {
    wml_xml_part_name(part_name, caller)?;
    if package.part(part_name).is_none() {
        return Err(Error::Docx(format!(
            "{caller}: editable field story part {part_name:?} does not exist"
        )));
    }

    let target = match part_name {
        "word/document.xml" => FieldStoryTarget::body(),
        "word/footnotes.xml" => FieldStoryTarget {
            part: part_name.to_string(),
            root_local: b"footnotes",
            note_local: Some(b"footnote"),
            content_type: CT_FOOTNOTES,
            body: false,
        },
        "word/endnotes.xml" => FieldStoryTarget {
            part: part_name.to_string(),
            root_local: b"endnotes",
            note_local: Some(b"endnote"),
            content_type: CT_ENDNOTES,
            body: false,
        },
        _ if package.part_resolves_as(part_name, CT_HEADER) => FieldStoryTarget {
            part: part_name.to_string(),
            root_local: b"hdr",
            note_local: None,
            content_type: CT_HEADER,
            body: false,
        },
        _ if package.part_resolves_as(part_name, CT_FOOTER) => FieldStoryTarget {
            part: part_name.to_string(),
            root_local: b"ftr",
            note_local: None,
            content_type: CT_FOOTER,
            body: false,
        },
        _ => {
            return Err(Error::Docx(format!(
                "{caller}: {part_name:?} is not an editable field story part"
            )));
        }
    };

    if !package.part_resolves_as(part_name, target.content_type) {
        return Err(Error::Docx(format!(
            "{caller}: {part_name:?} is not an editable field story part with the expected content type"
        )));
    }
    Ok(target)
}

#[cfg(feature = "docx")]
fn field_story_roots(
    tree: &xmltree::XmlTree,
    target: &FieldStoryTarget,
) -> Result<(xmltree::NodeId, Vec<xmltree::NodeId>)> {
    if target.body {
        let body = tree.wml_body_strict()?;
        return Ok((body, vec![body]));
    }

    let root = tree.wml_part_root_strict(&target.part, target.root_local)?;
    let roots = target.note_local.map_or_else(
        || vec![root],
        |note_local| tree.wml_real_note_entries_under(root, note_local),
    );
    Ok((root, roots))
}

#[cfg(feature = "docx")]
fn field_story_inventory(
    package: &opc::Package,
    target: &FieldStoryTarget,
    caller: &str,
) -> Result<Vec<Field>> {
    let raw = package
        .part(&target.part)
        .ok_or_else(|| Error::Docx(format!("{caller}: missing {}", target.part)))?;
    let tree = xmltree::XmlTree::parse(&raw)?;
    let (policy_root, roots) = field_story_roots(&tree, target)?;
    if !tree.wml_field_alternate_content_policy_matches_reader(policy_root) {
        return Err(Error::Docx(format!(
            "{caller}: editable field inventory for {:?} differs from the accepted-current read view",
            target.part
        )));
    }

    let editable_instructions: Vec<_> = story_field_instructions(&tree, &roots)
        .into_iter()
        .map(|instruction| annotation::normalized_field_instruction(&instruction))
        .collect();
    let mut fields = Vec::new();
    for root in roots {
        let xml = tree.serialize_subtree(root);
        fields.extend(docx::parse_fields(&String::from_utf8_lossy(&xml)));
    }
    for field in &mut fields {
        field.computed_result = None;
    }
    let read_instructions: Vec<_> = fields
        .iter()
        .map(|field| field.instruction.clone())
        .collect();
    if editable_instructions != read_instructions {
        return Err(Error::Docx(format!(
            "{caller}: editable field inventory for {:?} differs from the accepted-current read view",
            target.part
        )));
    }
    Ok(fields)
}

#[cfg(feature = "docx")]
fn set_field_result_in_story(
    state: &mut docx::DocxState,
    target: &FieldStoryTarget,
    field_index: usize,
    result: &str,
    caller: &str,
    range: &str,
) -> Result<()> {
    let fields = field_story_inventory(&state.package, target, caller)?;
    if field_index >= fields.len() {
        return Err(Error::Docx(format!(
            "field index {field_index} is outside {range}"
        )));
    }

    let mut package = state.package.clone();
    {
        let tree = package.part_tree_mut(&target.part)?;
        let (_, roots) = field_story_roots(tree, target)?;
        let runs = story_field_result_runs(tree, &roots, field_index)
            .ok_or_else(|| Error::Docx(format!("field index {field_index} out of range")))?;
        if runs.is_empty() {
            return Err(Error::Docx(format!(
                "field index {field_index} has no cached result text"
            )));
        }

        let needs_markers = result.contains('\t') || result.contains('\n');
        let new_nodes = if needs_markers {
            let first_replacement_nodes = xmltree::wml_text_run_content_node_count(result)?;
            first_replacement_nodes.saturating_add(
                runs.iter()
                    .skip(1)
                    .filter(|&&id| !tree.has_text_carrier(id))
                    .count(),
            )
        } else {
            runs.iter()
                .filter(|&&id| !tree.has_text_carrier(id))
                .count()
        };
        if tree.node_count().saturating_add(new_nodes) > xmltree::node_budget() {
            return Err(Error::Docx(format!(
                "{caller}: edit would exceed the node budget"
            )));
        }

        let needs_space = result != result.trim_matches([' ', '\t', '\n', '\r']);
        if !needs_markers && needs_space && !tree.can_set_attr(runs[0], b"xml:space") {
            return Err(Error::Docx(format!(
                "{caller}: edit would exceed an element's attribute budget"
            )));
        }

        for (index, id) in runs.into_iter().enumerate() {
            if index == 0 && needs_markers {
                tree.replace_wml_text_element_with_run_content(id, result)?;
            } else {
                tree.set_element_text(id, if index == 0 { result } else { "" })?;
            }
        }
    }
    package.ensure_content_type(&target.part, target.content_type);
    commit_docx_package(state, package)?;
    Ok(())
}

#[cfg(feature = "docx")]
fn collect_story_template_match(
    package: &opc::Package,
    target: StoryTemplateTarget,
    entries: &[(String, String)],
    caller: &str,
) -> Result<Option<StoryTemplateMatch>> {
    let Some(raw) = package.part(&target.part) else {
        return Ok(None);
    };
    let probe = xmltree::XmlTree::parse(&raw)?;
    let root = probe.wml_part_root_strict(&target.part, target.root_local)?;
    let roots = story_template_roots(&probe, &target, root);
    let field_instructions = story_field_instructions(&probe, &roots);
    let mut part_matches = Vec::new();
    let mut part_content_count = 0usize;
    let mut part_fields = Vec::new();

    for (entry_index, (name, _)) in entries.iter().enumerate() {
        for &story_root in &roots {
            for group in probe.wml_content_control_text_groups_by_tag_under(story_root, name) {
                if group.text_runs().is_empty() {
                    return Err(Error::Docx(format!(
                        "{caller}: template field {name:?} has no visible text"
                    )));
                }
                part_content_count += 1;
                part_matches.push((entry_index, group));
            }
        }
    }

    for (field_index, instruction) in field_instructions.iter().enumerate() {
        let Some(name) = merge_field_name(instruction) else {
            continue;
        };
        let Some(entry_index) = entries
            .iter()
            .position(|(entry_name, _)| entry_name == &name)
        else {
            continue;
        };
        let group =
            story_field_result_text_group(&probe, &roots, field_index).ok_or_else(|| {
                Error::Docx(format!(
                    "{caller}: merge field {name:?} has no cached result"
                ))
            })?;
        if group.text_runs().is_empty() {
            return Err(Error::Docx(format!(
                "{caller}: merge field {name:?} has no cached result text"
            )));
        }
        part_fields.push((field_index, entry_index));
        part_matches.push((entry_index, group));
    }

    if part_matches.is_empty() {
        return Ok(None);
    }

    let mut seen_runs = std::collections::HashSet::new();
    for (_, group) in &part_matches {
        for &id in group.text_runs().iter().chain(group.marker_nodes().iter()) {
            if !seen_runs.insert(id) {
                return Err(Error::Docx(format!(
                    "{caller}: requested template fields overlap"
                )));
            }
        }
    }

    let new_nodes = part_matches
        .iter()
        .try_fold(0usize, |total, (entry_index, group)| {
            wml_grouped_template_text_replacement_new_nodes(&probe, group, &entries[*entry_index].1)
                .map(|count| total.saturating_add(count))
        })?;
    let live_count = package
        .part_tree_ref(&target.part)
        .map_or(probe.node_count(), |t| t.node_count());
    if live_count.saturating_add(new_nodes) > xmltree::node_budget() {
        return Err(Error::Docx(format!(
            "{caller}: edit would exceed the node budget"
        )));
    }

    if part_matches.iter().any(|(entry_index, group)| {
        let text = &entries[*entry_index].1;
        wml_replacement_needs_space_attr_preflight(text)
            && group
                .text_runs()
                .first()
                .is_some_and(|&id| !probe.can_set_attr(id, b"xml:space"))
    }) {
        return Err(Error::Docx(format!(
            "{caller}: edit would exceed an element's attribute budget"
        )));
    }

    Ok(Some(StoryTemplateMatch {
        target,
        content_count: part_content_count,
        fields: part_fields,
    }))
}

#[cfg(feature = "docx")]
fn body_hyperlink_rids(package: &opc::Package) -> Result<Vec<String>> {
    if let Some(tree) = package.part_tree_ref("word/document.xml") {
        let body = tree.wml_body_strict()?;
        return Ok(tree.wml_hyperlink_rids_under(body));
    }

    let raw = package
        .part("word/document.xml")
        .ok_or_else(|| Error::Docx("missing word/document.xml".into()))?;
    let tree = xmltree::XmlTree::parse(&raw)?;
    let body = tree.wml_body_strict()?;
    Ok(tree.wml_hyperlink_rids_under(body))
}

#[cfg(feature = "docx")]
fn wml_xml_part_name(part_name: &str, op: &str) -> Result<()> {
    let valid_chars = part_name
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'.' | b'_' | b'-'));
    let valid_segments = part_name
        .split('/')
        .all(|s| !s.is_empty() && s != "." && s != ".." && !s.eq_ignore_ascii_case("_rels"));
    let valid = part_name.starts_with("word/")
        && part_name.ends_with(".xml")
        && valid_chars
        && valid_segments
        && part_name.len() <= opc::MAX_NAME_LEN;
    if valid {
        Ok(())
    } else {
        Err(Error::Docx(format!(
            "{op}: invalid WordprocessingML part name {part_name:?}: expected an existing word/*.xml part outside relationship directories"
        )))
    }
}

#[cfg(feature = "docx")]
#[derive(Clone, Copy, Debug)]
enum ImageMediaKind {
    Png,
    Jpeg,
    Gif,
    Bmp,
    Tiff,
    Webp,
}

#[cfg(feature = "docx")]
impl ImageMediaKind {
    fn label(self) -> &'static str {
        match self {
            ImageMediaKind::Png => "PNG",
            ImageMediaKind::Jpeg => "JPEG",
            ImageMediaKind::Gif => "GIF",
            ImageMediaKind::Bmp => "BMP",
            ImageMediaKind::Tiff => "TIFF",
            ImageMediaKind::Webp => "WebP",
        }
    }

    fn content_type(self) -> &'static str {
        match self {
            ImageMediaKind::Png => CT_IMAGE_PNG,
            ImageMediaKind::Jpeg => CT_IMAGE_JPEG,
            ImageMediaKind::Gif => CT_IMAGE_GIF,
            ImageMediaKind::Bmp => CT_IMAGE_BMP,
            ImageMediaKind::Tiff => CT_IMAGE_TIFF,
            ImageMediaKind::Webp => CT_IMAGE_WEBP,
        }
    }

    fn extensions(self) -> &'static [&'static str] {
        match self {
            ImageMediaKind::Png => &[".png"],
            ImageMediaKind::Jpeg => &[".jpg", ".jpeg"],
            ImageMediaKind::Gif => &[".gif"],
            ImageMediaKind::Bmp => &[".bmp"],
            ImageMediaKind::Tiff => &[".tif", ".tiff"],
            ImageMediaKind::Webp => &[".webp"],
        }
    }

    fn expected_extension(self) -> &'static str {
        match self {
            ImageMediaKind::Png => ".png",
            ImageMediaKind::Jpeg => ".jpg or .jpeg",
            ImageMediaKind::Gif => ".gif",
            ImageMediaKind::Bmp => ".bmp",
            ImageMediaKind::Tiff => ".tif or .tiff",
            ImageMediaKind::Webp => ".webp",
        }
    }

    fn is_valid(self, bytes: &[u8]) -> bool {
        match self {
            ImageMediaKind::Png => is_png(bytes),
            ImageMediaKind::Jpeg => jpeg_dimensions(bytes).is_some(),
            ImageMediaKind::Gif => gif_dimensions(bytes).is_some(),
            ImageMediaKind::Bmp => bmp_dimensions(bytes).is_some(),
            ImageMediaKind::Tiff => crate::image::dims(bytes, CT_IMAGE_TIFF).is_some(),
            ImageMediaKind::Webp => crate::image::dims(bytes, CT_IMAGE_WEBP).is_some(),
        }
    }

    fn extent_emu(self, bytes: &[u8]) -> (u32, u32) {
        match self {
            ImageMediaKind::Png => png_extent_emu(bytes),
            ImageMediaKind::Jpeg => jpeg_dimensions(bytes)
                .map(|(w, h)| extent_emu_from_pixels(w, h))
                .unwrap_or((FALLBACK_IMAGE_EMU, FALLBACK_IMAGE_EMU)),
            ImageMediaKind::Gif => gif_dimensions(bytes)
                .map(|(w, h)| extent_emu_from_pixels(w, h))
                .unwrap_or((FALLBACK_IMAGE_EMU, FALLBACK_IMAGE_EMU)),
            ImageMediaKind::Bmp => bmp_dimensions(bytes)
                .map(|(w, h)| extent_emu_from_pixels(w, h))
                .unwrap_or((FALLBACK_IMAGE_EMU, FALLBACK_IMAGE_EMU)),
            ImageMediaKind::Tiff => crate::image::dims(bytes, CT_IMAGE_TIFF)
                .map(|(w, h)| extent_emu_from_pixels(w, h))
                .unwrap_or((FALLBACK_IMAGE_EMU, FALLBACK_IMAGE_EMU)),
            ImageMediaKind::Webp => crate::image::dims(bytes, CT_IMAGE_WEBP)
                .map(|(w, h)| extent_emu_from_pixels(w, h))
                .unwrap_or((FALLBACK_IMAGE_EMU, FALLBACK_IMAGE_EMU)),
        }
    }
}

#[cfg(feature = "docx")]
fn image_media_part_name(name: &str, kind: ImageMediaKind, op: &str) -> Result<String> {
    // Restrict to a conservative, URI-safe segment so the name can be written
    // verbatim into relationship targets without OPC pack-URI escaping issues:
    // `[A-Za-z0-9._-]+` ending in the expected extension, no `..`.
    let lower = name.to_ascii_lowercase();
    let stem_ok = !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && !name.contains("..")
        && kind.extensions().iter().any(|ext| lower.ends_with(ext));
    if !stem_ok {
        return Err(Error::Docx(format!(
            "{op}: invalid image name {name:?}: expected a plain [A-Za-z0-9._-]+{} file name",
            kind.expected_extension()
        )));
    }
    let part = format!("word/media/{name}");
    if part.len() > opc::MAX_NAME_LEN {
        return Err(Error::Docx(format!("{op}: image part name too long")));
    }
    Ok(part)
}

#[cfg(feature = "docx")]
fn comments_part_skeleton() -> &'static [u8] {
    br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:comments>"#
}

#[cfg(feature = "docx")]
fn footnotes_part_skeleton() -> &'static [u8] {
    br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:footnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:footnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:footnote><w:footnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:footnote></w:footnotes>"#
}

#[cfg(feature = "docx")]
fn endnotes_part_skeleton() -> &'static [u8] {
    br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:endnotes xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:endnote w:type="separator" w:id="-1"><w:p><w:r><w:separator/></w:r></w:p></w:endnote><w:endnote w:type="continuationSeparator" w:id="0"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:endnote></w:endnotes>"#
}

#[cfg(feature = "docx")]
fn core_properties_skeleton() -> &'static [u8] {
    br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"></cp:coreProperties>"#
}

#[cfg(feature = "docx")]
fn next_comment_id(package: &opc::Package) -> Result<String> {
    let mut max_id = None;
    for part in ["word/document.xml", "word/comments.xml"] {
        if let Some(bytes) = package.part(part) {
            if let Some(found) = max_comment_id_in_xml(&bytes) {
                max_id = Some(max_id.map_or(found, |current: u64| current.max(found)));
            }
        }
    }
    let next = max_id.map_or(0, |id| id.saturating_add(1));
    if next == u64::MAX {
        return Err(Error::Docx(
            "add_comment_on_text: no available comment id".into(),
        ));
    }
    Ok(next.to_string())
}

#[cfg(feature = "docx")]
fn next_footnote_id(package: &opc::Package) -> Result<String> {
    let mut max_id = None;
    for part in ["word/document.xml", "word/footnotes.xml"] {
        if let Some(bytes) = package.part(part) {
            if let Some(found) = max_footnote_id_in_xml(&bytes) {
                max_id = Some(max_id.map_or(found, |current: u64| current.max(found)));
            }
        }
    }
    let next = max_id.map_or(1, |id| id.saturating_add(1));
    if next == u64::MAX {
        return Err(Error::Docx(
            "add_footnote_on_text: no available footnote id".into(),
        ));
    }
    Ok(next.to_string())
}

#[cfg(feature = "docx")]
fn next_endnote_id(package: &opc::Package) -> Result<String> {
    let mut max_id = None;
    for part in ["word/document.xml", "word/endnotes.xml"] {
        if let Some(bytes) = package.part(part) {
            if let Some(found) = max_endnote_id_in_xml(&bytes) {
                max_id = Some(max_id.map_or(found, |current: u64| current.max(found)));
            }
        }
    }
    let next = max_id.map_or(1, |id| id.saturating_add(1));
    if next == u64::MAX {
        return Err(Error::Docx(
            "add_endnote_on_text: no available endnote id".into(),
        ));
    }
    Ok(next.to_string())
}

#[cfg(feature = "docx")]
fn max_comment_id_in_xml(xml: &[u8]) -> Option<u64> {
    use quick_xml::events::{BytesStart, Event};
    use quick_xml::Reader;

    fn local(name: &[u8]) -> &[u8] {
        name.iter()
            .position(|&b| b == b':')
            .map_or(name, |i| &name[i + 1..])
    }

    fn attr_id(e: &BytesStart<'_>) -> Option<u64> {
        e.attributes().flatten().find_map(|attr| {
            (local(attr.key.as_ref()) == b"id")
                .then(|| {
                    std::str::from_utf8(attr.value.as_ref())
                        .ok()?
                        .trim()
                        .parse()
                        .ok()
                })
                .flatten()
        })
    }

    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut max_id = None;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if matches!(
                    local(e.name().as_ref()),
                    b"comment" | b"commentRangeStart" | b"commentRangeEnd" | b"commentReference"
                ) =>
            {
                if let Some(id) = attr_id(&e) {
                    max_id = Some(max_id.map_or(id, |current: u64| current.max(id)));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    max_id
}

#[cfg(feature = "docx")]
fn max_footnote_id_in_xml(xml: &[u8]) -> Option<u64> {
    use quick_xml::events::{BytesStart, Event};
    use quick_xml::Reader;

    fn local(name: &[u8]) -> &[u8] {
        name.iter()
            .position(|&b| b == b':')
            .map_or(name, |i| &name[i + 1..])
    }

    fn attr_id(e: &BytesStart<'_>) -> Option<u64> {
        e.attributes().flatten().find_map(|attr| {
            (local(attr.key.as_ref()) == b"id")
                .then(|| {
                    std::str::from_utf8(attr.value.as_ref())
                        .ok()?
                        .trim()
                        .parse()
                        .ok()
                })
                .flatten()
        })
    }

    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut max_id = None;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"footnote" | b"footnoteReference") =>
            {
                if let Some(id) = attr_id(&e) {
                    max_id = Some(max_id.map_or(id, |current: u64| current.max(id)));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    max_id
}

#[cfg(feature = "docx")]
fn max_endnote_id_in_xml(xml: &[u8]) -> Option<u64> {
    use quick_xml::events::{BytesStart, Event};
    use quick_xml::Reader;

    fn local(name: &[u8]) -> &[u8] {
        name.iter()
            .position(|&b| b == b':')
            .map_or(name, |i| &name[i + 1..])
    }

    fn attr_id(e: &BytesStart<'_>) -> Option<u64> {
        e.attributes().flatten().find_map(|attr| {
            (local(attr.key.as_ref()) == b"id")
                .then(|| {
                    std::str::from_utf8(attr.value.as_ref())
                        .ok()?
                        .trim()
                        .parse()
                        .ok()
                })
                .flatten()
        })
    }

    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut max_id = None;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"endnote" | b"endnoteReference") =>
            {
                if let Some(id) = attr_id(&e) {
                    max_id = Some(max_id.map_or(id, |current: u64| current.max(id)));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    max_id
}

/// CRC-32 (ISO-HDLC / the variant PNG uses) of `data`, computed bitwise so no lookup
/// table or dependency is needed. Used to verify each PNG chunk's integrity.
#[cfg(feature = "docx")]
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & (!(crc & 1)).wrapping_add(1));
        }
    }
    !crc
}

/// PNG validation by a bounded chunk walk. It enforces: the 8-byte signature; a single
/// leading `IHDR` (length 13, non-zero width/height ≤ 2²⁴, a legal `color_type`/
/// `bit_depth` pair, compression/filter = 0, interlace ≤ 1); well-formed chunk framing
/// (each `length(4) + type(4) + data + crc(4)`, no overrun/truncation) with a **correct
/// CRC-32** on every chunk; correct chunk ordering (`PLTE` required for indexed colour /
/// forbidden for greyscale, before any `IDAT`; `IDAT` chunks consecutive); **non-empty**
/// `IDAT` data carrying a well-formed **zlib header** (deflate method, valid `FCHECK`);
/// and a terminating `IEND` (length 0) with no trailing bytes. A forged or corrupt
/// payload — bad framing, wrong CRC, impossible header fields, misordered/empty/
/// non-zlib image data — is rejected, so the declared `image/png` is a structurally
/// well-formed **PNG container**.
///
/// This is a structural/container check, **not a full image decode**: the IDAT zlib
/// stream is header-validated but not inflated, so a container whose compressed body is
/// itself corrupt can still pass here and fail in a strict PNG decoder. (Full decode is
/// intentionally out of scope to avoid a decompressor dependency on this path.)
/// Panic-free and linear: every iteration advances a full chunk via checked math.
#[cfg(feature = "docx")]
fn is_png(bytes: &[u8]) -> bool {
    const SIG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    if bytes.len() < 8 || bytes[..8] != SIG {
        return false;
    }
    // PNG declares dimensions up to 2^31-1, but cap at a sane bound so a hostile header
    // can't claim an absurd size (and downstream EMU math stays comfortable).
    const MAX_DIM: u32 = 1 << 24;
    let mut i = 8usize;
    let mut color_type = 0u8;
    let mut idat_bytes = 0usize;
    let mut zlib_hdr = [0u8; 2]; // first two bytes of the concatenated IDAT stream
    let mut zlib_have = 0usize;
    let (mut seen_ihdr, mut seen_plte, mut seen_idat, mut idat_done) = (false, false, false, false);
    while i + 8 <= bytes.len() {
        let len = u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
        let typ = &bytes[i + 4..i + 8];
        // A PNG chunk type is four ASCII letters; anything else is not a real chunk.
        if !typ.iter().all(u8::is_ascii_alphabetic) {
            return false;
        }
        // Full chunk end = 4 (len) + 4 (type) + len (data) + 4 (crc), checked.
        let end = match i.checked_add(12).and_then(|x| x.checked_add(len)) {
            Some(e) if e <= bytes.len() => e,
            _ => return false,
        };
        // Verify the chunk CRC over type + data (stored in the final 4 bytes).
        let stored = u32::from_be_bytes([
            bytes[end - 4],
            bytes[end - 3],
            bytes[end - 2],
            bytes[end - 1],
        ]);
        if crc32(&bytes[i + 4..end - 4]) != stored {
            return false;
        }
        let data = &bytes[i + 8..end - 4];
        if !seen_ihdr {
            // The first chunk must be a 13-byte IHDR with non-zero, bounded dimensions
            // and valid header fields (an impossible color-type/bit-depth combo means the
            // bytes are not a real image even if every CRC checks out).
            if typ != b"IHDR" || len != 13 {
                return false;
            }
            let w = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
            let h = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
            let (bit_depth, ct) = (data[8], data[9]);
            let (compression, filter, interlace) = (data[10], data[11], data[12]);
            color_type = ct;
            // PNG spec: compression/filter methods are 0, interlace is 0 or 1, and only
            // these (color_type, bit_depth) pairs are legal.
            let depth_ok = match ct {
                0 => matches!(bit_depth, 1 | 2 | 4 | 8 | 16), // grayscale
                3 => matches!(bit_depth, 1 | 2 | 4 | 8),      // indexed
                2 | 4 | 6 => matches!(bit_depth, 8 | 16),     // truecolor / +alpha
                _ => false,
            };
            if w == 0 || h == 0 || w > MAX_DIM || h > MAX_DIM || !depth_ok {
                return false;
            }
            if compression != 0 || filter != 0 || interlace > 1 {
                return false;
            }
            seen_ihdr = true;
        } else if typ == b"IHDR" {
            return false; // duplicate IHDR
        } else if typ == b"PLTE" {
            // A palette is required for indexed images and forbidden for grayscale; it
            // must appear after IHDR and before any IDAT, and its length must be a whole
            // number of 1..=256 RGB triples.
            let entries = len / 3;
            if seen_plte
                || seen_idat
                || matches!(color_type, 0 | 4)
                || len % 3 != 0
                || !(1..=256).contains(&entries)
            {
                return false;
            }
            seen_plte = true;
        } else if typ == b"IDAT" {
            if idat_done {
                return false; // IDAT chunks must be consecutive
            }
            // Capture the leading zlib header bytes across the (possibly split) stream.
            for &b in data {
                if zlib_have < 2 {
                    zlib_hdr[zlib_have] = b;
                    zlib_have += 1;
                }
            }
            seen_idat = true;
            idat_bytes += len;
        } else if typ == b"IEND" {
            // IEND ends the file: empty, last (no trailing bytes), with a palette present
            // iff indexed, non-empty IDAT, and a well-formed zlib header on that stream.
            let zlib_ok = zlib_have == 2
                && (zlib_hdr[0] & 0x0F) == 8                       // deflate method
                && (zlib_hdr[0] >> 4) <= 7                          // window size
                && (zlib_hdr[1] & 0x20) == 0                        // no preset dictionary (FDICT)
                && (u16::from(zlib_hdr[0]) * 256 + u16::from(zlib_hdr[1])) % 31 == 0;
            let palette_ok = (color_type != 3) || seen_plte;
            return len == 0 && end == bytes.len() && idat_bytes > 0 && zlib_ok && palette_ok;
        } else if seen_idat {
            idat_done = true; // a non-IDAT chunk after IDAT closes the IDAT run
        }
        i = end;
    }
    false // ran out of bytes without a terminating IEND
}

#[cfg(feature = "docx")]
const EMU_PER_PX: u64 = 9525;
#[cfg(feature = "docx")]
const MAX_IMAGE_W_EMU: u64 = 5_486_400; // 6 in
#[cfg(feature = "docx")]
const FALLBACK_IMAGE_EMU: u32 = 1_828_800; // 2 in

#[cfg(feature = "docx")]
fn extent_emu_from_pixels(width_px: u32, height_px: u32) -> (u32, u32) {
    let (w, h) = (u64::from(width_px), u64::from(height_px));
    if w == 0 || h == 0 {
        return (FALLBACK_IMAGE_EMU, FALLBACK_IMAGE_EMU);
    }
    // u128 intermediates: a huge header can make `h * EMU_PER_PX * MAX_IMAGE_W_EMU`
    // overflow u64 even though the final clamped dimensions fit comfortably.
    let (mut cx, mut cy) = (w * EMU_PER_PX, h * EMU_PER_PX);
    if cx > MAX_IMAGE_W_EMU {
        cy = ((cy as u128 * MAX_IMAGE_W_EMU as u128) / cx as u128).max(1) as u64;
        cx = MAX_IMAGE_W_EMU;
    }
    (
        cx.min(u32::MAX as u64) as u32,
        cy.min(u32::MAX as u64) as u32,
    )
}

/// Inline-image extent in EMU from a PNG's `IHDR` dimensions (96 dpi → 9525
/// EMU/px), width clamped to ~6 in with aspect preserved; 2 in² fallback if the
/// PNG header can't be read.
#[cfg(feature = "docx")]
fn png_extent_emu(png: &[u8]) -> (u32, u32) {
    if png.len() >= 24
        && png[..8] == [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]
        && &png[12..16] == b"IHDR"
    {
        let w = u32::from_be_bytes([png[16], png[17], png[18], png[19]]) as u64;
        let h = u32::from_be_bytes([png[20], png[21], png[22], png[23]]) as u64;
        if w > 0 && h > 0 {
            return extent_emu_from_pixels(w as u32, h as u32);
        }
    }
    (FALLBACK_IMAGE_EMU, FALLBACK_IMAGE_EMU)
}

#[cfg(feature = "docx")]
fn gif_dimensions(gif: &[u8]) -> Option<(u32, u32)> {
    if gif.len() < 14
        || (&gif[..6] != b"GIF87a" && &gif[..6] != b"GIF89a")
        || gif.last().copied() != Some(0x3B)
        || !gif[13..gif.len() - 1].contains(&0x2C)
    {
        return None;
    }
    let w = u16::from_le_bytes([gif[6], gif[7]]) as u32;
    let h = u16::from_le_bytes([gif[8], gif[9]]) as u32;
    (w > 0 && h > 0).then_some((w, h))
}

#[cfg(feature = "docx")]
fn bmp_dimensions(bmp: &[u8]) -> Option<(u32, u32)> {
    if bmp.len() < 54 || &bmp[..2] != b"BM" {
        return None;
    }
    let file_size = u32::from_le_bytes([bmp[2], bmp[3], bmp[4], bmp[5]]) as usize;
    let pixel_offset = u32::from_le_bytes([bmp[10], bmp[11], bmp[12], bmp[13]]) as usize;
    let dib_size = u32::from_le_bytes([bmp[14], bmp[15], bmp[16], bmp[17]]) as usize;
    if file_size < pixel_offset
        || file_size > bmp.len()
        || dib_size < 40
        || 14 + dib_size > bmp.len()
        || pixel_offset < 14 + dib_size
        || pixel_offset > bmp.len()
    {
        return None;
    }
    let w = i32::from_le_bytes([bmp[18], bmp[19], bmp[20], bmp[21]]).unsigned_abs();
    let h = i32::from_le_bytes([bmp[22], bmp[23], bmp[24], bmp[25]]).unsigned_abs();
    let planes = u16::from_le_bytes([bmp[26], bmp[27]]);
    let bit_depth = u16::from_le_bytes([bmp[28], bmp[29]]);
    let compression = u32::from_le_bytes([bmp[30], bmp[31], bmp[32], bmp[33]]);
    if w == 0
        || h == 0
        || planes != 1
        || !matches!(bit_depth, 1 | 4 | 8 | 16 | 24 | 32)
        || compression != 0
    {
        return None;
    }
    Some((w, h))
}

/// JPEG validation and intrinsic dimensions from a bounded marker walk. It enforces:
/// SOI, well-framed pre-scan segments, one SOF marker with non-zero dimensions and
/// coherent component table, one SOS marker with coherent selector table, and a final
/// EOI with no trailing bytes. It intentionally does not decode entropy-coded scan
/// data.
#[cfg(feature = "docx")]
fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2usize;
    let mut dims = None;
    while i < bytes.len() {
        if bytes[i] != 0xFF {
            return None;
        }
        while i < bytes.len() && bytes[i] == 0xFF {
            i += 1;
        }
        if i >= bytes.len() {
            return None;
        }
        let marker = bytes[i];
        i += 1;
        match marker {
            0xD8 => return None,            // nested SOI
            0xD9 => return None,            // EOI before a scan
            0x01 | 0xD0..=0xD7 => continue, // standalone markers
            _ => {}
        }

        if i + 2 > bytes.len() {
            return None;
        }
        let seg_len = u16::from_be_bytes([bytes[i], bytes[i + 1]]) as usize;
        if seg_len < 2 {
            return None;
        }
        let data_start = i + 2;
        let data_end = i.checked_add(seg_len)?;
        if data_end > bytes.len() {
            return None;
        }
        let data = &bytes[data_start..data_end];

        if is_jpeg_sof(marker) {
            dims = Some(jpeg_sof_dimensions(data)?);
        } else if marker == 0xDA {
            jpeg_sos_is_well_formed(data)?;
            let (w, h) = dims?;
            return jpeg_scan_has_final_eoi(&bytes[data_end..]).then_some((w, h));
        }
        i = data_end;
    }
    None
}

#[cfg(feature = "docx")]
fn is_jpeg_sof(marker: u8) -> bool {
    (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC)
}

#[cfg(feature = "docx")]
fn jpeg_sof_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    if data.len() < 6 {
        return None;
    }
    let precision = data[0];
    let h = u16::from_be_bytes([data[1], data[2]]) as u32;
    let w = u16::from_be_bytes([data[3], data[4]]) as u32;
    let components = data[5] as usize;
    let precision_ok = matches!(precision, 8 | 12 | 16);
    let component_len = 6usize.checked_add(components.checked_mul(3)?)?;
    (precision_ok && w > 0 && h > 0 && (1..=4).contains(&components) && data.len() == component_len)
        .then_some((w, h))
}

#[cfg(feature = "docx")]
fn jpeg_sos_is_well_formed(data: &[u8]) -> Option<()> {
    let (&components, rest) = data.split_first()?;
    let components = components as usize;
    let expected = 1usize
        .checked_add(components.checked_mul(2)?)?
        .checked_add(3)?;
    ((1..=4).contains(&components) && rest.len() + 1 == expected).then_some(())
}

#[cfg(feature = "docx")]
fn jpeg_scan_has_final_eoi(scan: &[u8]) -> bool {
    scan.len() >= 3 && scan[scan.len() - 2..] == [0xFF, 0xD9]
}

/// A self-contained inline-image paragraph fragment referencing relationship `rid`,
/// with drawing/picture id `did`. It declares **all** prefixes it uses — including
/// `w` — on the root `w:p`, so it grafts correctly into any host `document.xml`
/// regardless of which prefix (or default namespace) the host bound for
/// WordprocessingML.
#[cfg(feature = "docx")]
fn image_paragraph_xml(rid: &str, cx: u32, cy: u32, did: u32) -> String {
    const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    const WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
    const A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
    const PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
    const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    format!(
        r#"<w:p xmlns:w="{W}"><w:r><w:drawing xmlns:wp="{WP}" xmlns:a="{A}" xmlns:pic="{PIC}" xmlns:r="{R}"><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{did}" name="image{did}"/><a:graphic><a:graphicData uri="{PIC}"><pic:pic><pic:nvPicPr><pic:cNvPr id="{did}" name="image{did}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rid}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#
    )
}

impl DocState {
    /// Open and decode a legacy `.doc` from its raw OLE2 bytes.
    fn open(bytes: &[u8]) -> Result<Self> {
        let mut container = ole::Container::open(bytes)?;
        let word = container.required("WordDocument")?;
        let fib = Fib::parse(&word)?;

        // Refuse encrypted/obfuscated docs (catdoc/POI behaviour) rather than
        // indexing scrambled bytes.
        if fib.encrypted {
            return Err(Error::Encrypted {
                obfuscated: fib.obfuscated,
            });
        }
        // Pre-Word-97 (Word 6/95) has an all-8-bit text model and a different
        // FIB/piece-table layout; route those to a fallback extractor.
        if fib.nfib < 0x00C1 {
            return Err(Error::UnsupportedVersion(fib.nfib));
        }

        // Prefer the table stream the FIB selects; fall back to the other since
        // some writers emit only one.
        let table = container
            .stream(fib.table_stream())?
            .or(container.stream(if fib.which_table_stream_one {
                "0Table"
            } else {
                "1Table"
            })?)
            .ok_or(Error::MissingStream("0Table/1Table"))?;

        let end = fib.fc_clx.saturating_add(fib.lcb_clx).min(table.len());
        let clx = table
            .get(fib.fc_clx..end)
            .ok_or_else(|| Error::PieceTable("CLX out of table bounds".into()))?;

        let clx::ParsedClx { pieces, prcs } = clx::parse(clx)?;
        if pieces.is_empty() {
            return Err(Error::PieceTable("empty piece table".into()));
        }
        let prm1_patches = chpx::compile_pcd_prm1_patches(&prcs);

        // Paragraph properties (best-effort) for table reconstruction; an empty
        // table degrades gracefully to plain-paragraph rendering.
        let papx = papx::parse(&word, &table, fib.fc_plcf_bte_papx, fib.lcb_plcf_bte_papx);
        // Character properties (bold/italic/…) for the rich model; unused by text().
        let chpx = chpx::parse(&word, &table, fib.fc_plcf_bte_chpx, fib.lcb_plcf_bte_chpx);
        // Style sheet (heading levels, style names) for the rich model.
        let stylesheet = stsh::StyleSheet::parse(&table, fib.fc_stshf, fib.lcb_stshf);
        // Font-name table, for resolving CHPX font indices to family names.
        let fonts = ffn::parse(&table, fib.fc_sttbf_ffn, fib.lcb_sttbf_ffn);
        let annotation_owners = annotation::legacy_doc_comment_owners(&fib, &table);
        let annotation_metadata = annotation::legacy_doc_comment_metadata(&fib, &table);
        // The Data stream holds inline picture bytes (absent in most text docs).
        let data = container.stream("Data")?.unwrap_or_default();
        // List tables for autonumber reconstruction.
        let lists = list::parse(
            &table,
            fib.fc_plf_lst,
            fib.lcb_plf_lst,
            fib.fc_plf_lfo,
            fib.lcb_plf_lfo,
        );

        let enc = text::encoding_for_codepage(fib.ansi_codepage());
        let decoded = {
            let mut numberer = list::Numberer::new(&lists);
            text::decode_pieces(&word, &pieces, enc, &papx, &mut numberer)
        };
        Ok(DocState {
            labeled: decoded.labeled,
            fib,
            word,
            table,
            pieces,
            papx,
            chpx,
            prm1_patches,
            stylesheet,
            lists,
            fonts,
            annotation_owners,
            annotation_metadata,
            data,
            enc,
        })
    }
}

#[cfg(feature = "docx")]
fn merge_field_name(instruction: &str) -> Option<String> {
    annotation::merge_field_name(instruction)
}

impl std::fmt::Debug for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Document")
            .field("char_count", &self.char_count())
            .field("is_complex", &self.is_complex())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "docx")]
    use std::io::Read;
    use std::io::{Cursor, Write};

    /// Build a minimal valid `.doc` in memory: one uncompressed (UTF-16LE)
    /// piece and one compressed (cp1252) piece, with a single-piece CLX in the
    /// `1Table` stream.
    fn synth_doc(text_utf16: &str, ansi_tail: &str) -> Vec<u8> {
        synth_doc_ex(text_utf16, ansi_tail, 0x00C1, 0, 0)
    }

    /// As [`synth_doc`] but with explicit `nFib`, `lid`, and extra FIB flag bits.
    fn synth_doc_ex(
        text_utf16: &str,
        ansi_tail: &str,
        nfib: u16,
        lid: u16,
        extra_flags: u16,
    ) -> Vec<u8> {
        let ccp_text = (text_utf16.chars().count() + ansi_tail.chars().count()) as u32;
        synth_doc_with_ccp(
            text_utf16,
            ansi_tail,
            nfib,
            lid,
            extra_flags,
            [ccp_text, 0, 0, 0, 0, 0],
        )
    }

    /// As [`synth_doc_ex`] but with explicit FIB `ccpText`, `ccpFtn`,
    /// `ccpHdd`, `ccpAtn`, `ccpEdn`, and `ccpTxbx` counts.
    fn synth_doc_with_ccp(
        text_utf16: &str,
        ansi_tail: &str,
        nfib: u16,
        lid: u16,
        extra_flags: u16,
        ccp: [u32; 6],
    ) -> Vec<u8> {
        synth_doc_with_ccp_and_plcfhdd(text_utf16, ansi_tail, nfib, lid, extra_flags, ccp, None)
    }

    fn synth_doc_with_ccp_and_plcfhdd(
        text_utf16: &str,
        ansi_tail: &str,
        nfib: u16,
        lid: u16,
        extra_flags: u16,
        ccp: [u32; 6],
        plcf_hdd_cps: Option<&[u32]>,
    ) -> Vec<u8> {
        synth_doc_with_ccp_and_tables(
            text_utf16,
            ansi_tail,
            nfib,
            lid,
            extra_flags,
            ccp,
            SyntheticDocTables {
                plcf_hdd_cps,
                ..SyntheticDocTables::default()
            },
        )
    }

    fn synth_doc_with_ccp_plcfhdd_and_plcfsed(
        text_utf16: &str,
        ccp: [u32; 6],
        plcf_hdd_cps: &[u32],
        plcf_sed_cps: &[u32],
        plcf_sed_lcb_override: Option<u32>,
    ) -> Vec<u8> {
        synth_doc_with_ccp_and_tables(
            text_utf16,
            "",
            0x00C1,
            0,
            0,
            ccp,
            SyntheticDocTables {
                plcf_hdd_cps: Some(plcf_hdd_cps),
                plcf_sed_cps: Some(plcf_sed_cps),
                plcf_sed_lcb_override,
                ..SyntheticDocTables::default()
            },
        )
    }

    fn synth_doc_with_annotation_tables(
        text_utf16: &str,
        ccp: [u32; 6],
        refs: &[(&str, u16)],
        owners: &[&str],
        owner_lcb_override: Option<u32>,
    ) -> Vec<u8> {
        synth_doc_with_ccp_and_tables(
            text_utf16,
            "",
            0x00C1,
            0,
            0,
            ccp,
            SyntheticDocTables {
                annotation_refs: Some(refs),
                annotation_owners: Some(owners),
                owner_lcb_override,
                ..SyntheticDocTables::default()
            },
        )
    }

    fn synth_doc_with_note_reference_tables(
        text_utf16: &str,
        ccp: [u32; 6],
        footnote_refs: Option<&[u32]>,
        footnote_ref_lcb_override: Option<u32>,
        endnote_refs: Option<&[u32]>,
        endnote_ref_lcb_override: Option<u32>,
    ) -> Vec<u8> {
        synth_doc_with_ccp_and_tables(
            text_utf16,
            "",
            0x00C1,
            0,
            0,
            ccp,
            SyntheticDocTables {
                footnote_ref_cps: footnote_refs,
                footnote_ref_lcb_override,
                endnote_ref_cps: endnote_refs,
                endnote_ref_lcb_override,
                ..SyntheticDocTables::default()
            },
        )
    }

    fn synth_doc_with_shape_anchor_table(
        text_utf16: &str,
        ccp: [u32; 6],
        shape_anchor_cps: Option<&[u32]>,
        shape_anchor_lcb_override: Option<u32>,
    ) -> Vec<u8> {
        synth_doc_with_ccp_and_tables(
            text_utf16,
            "",
            0x00C1,
            0,
            0,
            ccp,
            SyntheticDocTables {
                shape_anchor_cps,
                shape_anchor_lcb_override,
                ..SyntheticDocTables::default()
            },
        )
    }

    #[derive(Default)]
    struct SyntheticDocTables<'a> {
        stylesheet: Option<&'a [u8]>,
        plcf_hdd_cps: Option<&'a [u32]>,
        plcf_sed_cps: Option<&'a [u32]>,
        plcf_sed_sepx_grpprls: Option<&'a [&'a [u8]]>,
        plcf_sed_lcb_override: Option<u32>,
        annotation_refs: Option<&'a [(&'a str, u16)]>,
        annotation_owners: Option<&'a [&'a str]>,
        owner_lcb_override: Option<u32>,
        footnote_ref_cps: Option<&'a [u32]>,
        footnote_ref_lcb_override: Option<u32>,
        endnote_ref_cps: Option<&'a [u32]>,
        endnote_ref_lcb_override: Option<u32>,
        shape_anchor_cps: Option<&'a [u32]>,
        shape_anchor_lcb_override: Option<u32>,
        prcs: Option<&'a [&'a [u8]]>,
        piece_prms: [u16; 2],
        chpx_runs: Option<&'a [SyntheticChpxRun]>,
        papx_runs: Option<&'a [SyntheticPapxRun]>,
        list_definition: Option<(&'a [u8], &'a [u8])>,
        list_overrides: Option<&'a [u8]>,
    }

    struct SyntheticChpxRun {
        cp_lim: u32,
        grpprl: Vec<u8>,
    }

    struct SyntheticPapxRun {
        cp_lim: u32,
        grpprl: Vec<u8>,
    }

    fn synth_doc_with_ccp_and_tables(
        text_utf16: &str,
        ansi_tail: &str,
        nfib: u16,
        lid: u16,
        extra_flags: u16,
        ccp: [u32; 6],
        tables: SyntheticDocTables<'_>,
    ) -> Vec<u8> {
        // --- WordDocument stream ---
        let mut word = vec![0u8; 0x200];
        word[0] = 0xEC; // wIdent 0xA5EC
        word[1] = 0xA5;
        word[2..4].copy_from_slice(&nfib.to_le_bytes());
        // flags @ 0x0A: fWhichTblStm (bit 9) set -> use 1Table, plus extras.
        word[0x0A..0x0C].copy_from_slice(&(0x0200u16 | extra_flags).to_le_bytes());
        word[0x14..0x16].copy_from_slice(&lid.to_le_bytes());
        // csw @ 32 = 14, cslw @ 34+28 = 22 (standard Word 97 layout).
        word[32] = 14;
        word[34 + 28] = 22;
        let rglw = 34 + 28 + 2;
        let fclcb = rglw + 22 * 4 + 2;
        word.resize(fclcb + 75 * 8, 0);
        // Character counts partitioning the CP stream by subdocument.
        for (idx, count) in [
            (3usize, ccp[0]),
            (4, ccp[1]),
            (5, ccp[2]),
            (7, ccp[3]),
            (8, ccp[4]),
            (9, ccp[5]),
        ] {
            word[rglw + idx * 4..rglw + idx * 4 + 4].copy_from_slice(&count.to_le_bytes());
        }

        // Piece 1 text (UTF-16LE) at offset 0x200; piece 2 (cp1252) right after.
        let utf16: Vec<u8> = text_utf16
            .encode_utf16()
            .flat_map(|u| u.to_le_bytes())
            .collect();
        let fc1 = word.len();
        word.extend_from_slice(&utf16);
        let fc2 = word.len();
        word.extend_from_slice(ansi_tail.as_bytes());
        let sepx_offsets: Vec<i32> = tables
            .plcf_sed_sepx_grpprls
            .unwrap_or_default()
            .iter()
            .map(|grpprl| {
                let offset = i32::try_from(word.len()).expect("synthetic SEPX offset fits i32");
                let cb = i16::try_from(grpprl.len()).expect("synthetic SEPX fits i16");
                word.extend_from_slice(&cb.to_le_bytes());
                word.extend_from_slice(grpprl);
                offset
            })
            .collect();

        // --- 1Table stream: CLX = Pcdt(0x02) + lcb + PlcPcd(2 pieces) ---
        let cch1 = text_utf16.encode_utf16().count() as u32;
        let cch2 = ansi_tail.len() as u32;
        let mut plc = Vec::new();
        // CPs: [0, cch1, cch1+cch2]
        plc.extend_from_slice(&0u32.to_le_bytes());
        plc.extend_from_slice(&cch1.to_le_bytes());
        plc.extend_from_slice(&(cch1 + cch2).to_le_bytes());
        // PCD 1: uncompressed, fc = fc1
        plc.extend_from_slice(&0u16.to_le_bytes());
        plc.extend_from_slice(&(fc1 as u32).to_le_bytes());
        plc.extend_from_slice(&tables.piece_prms[0].to_le_bytes());
        // PCD 2: compressed, FcCompressed = bit30 | (fc2*2)
        plc.extend_from_slice(&0u16.to_le_bytes());
        plc.extend_from_slice(&(0x4000_0000u32 | (fc2 as u32 * 2)).to_le_bytes());
        plc.extend_from_slice(&tables.piece_prms[1].to_le_bytes());

        let mut clx = Vec::new();
        for grpprl in tables.prcs.unwrap_or_default() {
            assert!(grpprl.len() <= 0x3FA2);
            clx.push(0x01);
            clx.extend_from_slice(&(grpprl.len() as i16).to_le_bytes());
            clx.extend_from_slice(grpprl);
        }
        clx.push(0x02);
        clx.extend_from_slice(&(plc.len() as u32).to_le_bytes());
        clx.extend_from_slice(&plc);

        if let Some(stylesheet) = tables.stylesheet {
            let offset = clx.len() as u32;
            clx.extend_from_slice(stylesheet);
            word[fclcb + 8..fclcb + 12].copy_from_slice(&offset.to_le_bytes());
            word[fclcb + 12..fclcb + 16].copy_from_slice(&(stylesheet.len() as u32).to_le_bytes());
        }
        let plcf_hdd_offset = clx.len() as u32;
        if let Some(cps) = tables.plcf_hdd_cps {
            for cp in cps {
                clx.extend_from_slice(&cp.to_le_bytes());
            }
            word[fclcb + 11 * 8..fclcb + 11 * 8 + 4]
                .copy_from_slice(&plcf_hdd_offset.to_le_bytes());
            word[fclcb + 11 * 8 + 4..fclcb + 11 * 8 + 8]
                .copy_from_slice(&((cps.len() as u32) * 4).to_le_bytes());
        }

        if let Some(cps) = tables.plcf_sed_cps {
            let (offset, lcb) = append_plcf_sed(
                &mut clx,
                cps,
                (!sepx_offsets.is_empty()).then_some(sepx_offsets.as_slice()),
                tables.plcf_sed_lcb_override,
            );
            word[fclcb + 6 * 8..fclcb + 6 * 8 + 4].copy_from_slice(&offset.to_le_bytes());
            word[fclcb + 6 * 8 + 4..fclcb + 6 * 8 + 8].copy_from_slice(&lcb.to_le_bytes());
        }

        if let Some(refs) = tables.annotation_refs {
            let plcfand_ref_offset = clx.len() as u32;
            for cp in 0..=refs.len() as u32 {
                clx.extend_from_slice(&cp.to_le_bytes());
            }
            for (initials, ibst) in refs {
                push_lpx_char_buffer9(&mut clx, initials);
                clx.extend_from_slice(&ibst.to_le_bytes());
                clx.extend_from_slice(&0u16.to_le_bytes());
                clx.extend_from_slice(&0u16.to_le_bytes());
                clx.extend_from_slice(&0i32.to_le_bytes());
            }
            let plcfand_ref_lcb = (refs.len() * 34 + 4) as u32;
            word[fclcb + 4 * 8..fclcb + 4 * 8 + 4]
                .copy_from_slice(&plcfand_ref_offset.to_le_bytes());
            word[fclcb + 4 * 8 + 4..fclcb + 4 * 8 + 8]
                .copy_from_slice(&plcfand_ref_lcb.to_le_bytes());
        }

        if let Some(refs) = tables.footnote_ref_cps {
            let (offset, lcb) = append_plc_with_u16_records(
                &mut clx,
                refs,
                ccp[0],
                tables.footnote_ref_lcb_override,
            );
            word[fclcb + 2 * 8..fclcb + 2 * 8 + 4].copy_from_slice(&offset.to_le_bytes());
            word[fclcb + 2 * 8 + 4..fclcb + 2 * 8 + 8].copy_from_slice(&lcb.to_le_bytes());
        }

        if let Some(refs) = tables.endnote_ref_cps {
            let (offset, lcb) = append_plc_with_u16_records(
                &mut clx,
                refs,
                ccp[0],
                tables.endnote_ref_lcb_override,
            );
            word[fclcb + 46 * 8..fclcb + 46 * 8 + 4].copy_from_slice(&offset.to_le_bytes());
            word[fclcb + 46 * 8 + 4..fclcb + 46 * 8 + 8].copy_from_slice(&lcb.to_le_bytes());
        }

        if let Some(refs) = tables.shape_anchor_cps {
            let (offset, lcb) = append_plc_with_spa_records(
                &mut clx,
                refs,
                ccp[0],
                tables.shape_anchor_lcb_override,
            );
            word[fclcb + 40 * 8..fclcb + 40 * 8 + 4].copy_from_slice(&offset.to_le_bytes());
            word[fclcb + 40 * 8 + 4..fclcb + 40 * 8 + 8].copy_from_slice(&lcb.to_le_bytes());
        }

        if let Some(owners) = tables.annotation_owners {
            let owners_offset = clx.len() as u32;
            for owner in owners {
                push_xst(&mut clx, owner);
            }
            let owners_lcb = tables
                .owner_lcb_override
                .unwrap_or(clx.len() as u32 - owners_offset);
            word[fclcb + 36 * 8..fclcb + 36 * 8 + 4].copy_from_slice(&owners_offset.to_le_bytes());
            word[fclcb + 36 * 8 + 4..fclcb + 36 * 8 + 8].copy_from_slice(&owners_lcb.to_le_bytes());
        }

        if let Some(runs) = tables.chpx_runs {
            let (offset, lcb) = append_synthetic_chpx(&mut word, &mut clx, fc1, runs);
            word[fclcb + 12 * 8..fclcb + 12 * 8 + 4].copy_from_slice(&offset.to_le_bytes());
            word[fclcb + 12 * 8 + 4..fclcb + 12 * 8 + 8].copy_from_slice(&lcb.to_le_bytes());
        }

        if let Some(runs) = tables.papx_runs {
            let (offset, lcb) = append_synthetic_papx(&mut word, &mut clx, fc1, runs);
            word[fclcb + 13 * 8..fclcb + 13 * 8 + 4].copy_from_slice(&offset.to_le_bytes());
            word[fclcb + 13 * 8 + 4..fclcb + 13 * 8 + 8].copy_from_slice(&lcb.to_le_bytes());
        }

        if let Some((header, levels)) = tables.list_definition {
            let offset = clx.len() as u32;
            clx.extend_from_slice(header);
            clx.extend_from_slice(levels);
            word[fclcb + 73 * 8..fclcb + 73 * 8 + 4].copy_from_slice(&offset.to_le_bytes());
            word[fclcb + 73 * 8 + 4..fclcb + 73 * 8 + 8]
                .copy_from_slice(&(header.len() as u32).to_le_bytes());
        }

        if let Some(overrides) = tables.list_overrides {
            let offset = clx.len() as u32;
            clx.extend_from_slice(overrides);
            word[fclcb + 74 * 8..fclcb + 74 * 8 + 4].copy_from_slice(&offset.to_le_bytes());
            word[fclcb + 74 * 8 + 4..fclcb + 74 * 8 + 8]
                .copy_from_slice(&(overrides.len() as u32).to_le_bytes());
        }

        // fcClx = 0, lcbClx = clx.len() (CLX at start of 1Table).
        word[fclcb + 33 * 8..fclcb + 33 * 8 + 4].copy_from_slice(&0u32.to_le_bytes());
        word[fclcb + 33 * 8 + 4..fclcb + 33 * 8 + 8]
            .copy_from_slice(&(clx.len() as u32).to_le_bytes());

        // --- assemble compound file ---
        let mut comp = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
        comp.create_stream("/WordDocument")
            .unwrap()
            .write_all(&word)
            .unwrap();
        comp.create_stream("/1Table")
            .unwrap()
            .write_all(&clx)
            .unwrap();
        comp.flush().unwrap();
        comp.into_inner().into_inner()
    }

    fn append_synthetic_chpx(
        word: &mut Vec<u8>,
        table: &mut Vec<u8>,
        text_fc: usize,
        runs: &[SyntheticChpxRun],
    ) -> (u32, u32) {
        assert!(!runs.is_empty());
        assert!(runs.len() <= 0x65);

        let page_number = word.len().div_ceil(512);
        word.resize(page_number * 512, 0);
        let mut page = [0u8; 512];
        let rgb_base = 4 * (runs.len() + 1);
        let mut chpx_offset = (rgb_base + runs.len() + 1) & !1;

        page[..4].copy_from_slice(&(text_fc as u32).to_le_bytes());
        for (index, run) in runs.iter().enumerate() {
            let fc_lim = (text_fc as u32).saturating_add(run.cp_lim.saturating_mul(2));
            page[4 * (index + 1)..4 * (index + 2)].copy_from_slice(&fc_lim.to_le_bytes());

            if run.grpprl.is_empty() {
                continue;
            }
            assert!(run.grpprl.len() <= u8::MAX as usize);
            assert!(chpx_offset + 1 + run.grpprl.len() < 511);
            page[rgb_base + index] = (chpx_offset / 2) as u8;
            page[chpx_offset] = run.grpprl.len() as u8;
            page[chpx_offset + 1..chpx_offset + 1 + run.grpprl.len()].copy_from_slice(&run.grpprl);
            chpx_offset = (chpx_offset + 1 + run.grpprl.len() + 1) & !1;
        }
        page[511] = runs.len() as u8;
        word.extend_from_slice(&page);

        let offset = table.len() as u32;
        table.extend_from_slice(&(text_fc as u32).to_le_bytes());
        let last_cp = runs.last().expect("non-empty CHPX runs").cp_lim;
        let fc_lim = (text_fc as u32).saturating_add(last_cp.saturating_mul(2));
        table.extend_from_slice(&fc_lim.to_le_bytes());
        table.extend_from_slice(&(page_number as u32).to_le_bytes());
        (offset, 12)
    }

    fn append_synthetic_papx(
        word: &mut Vec<u8>,
        table: &mut Vec<u8>,
        text_fc: usize,
        runs: &[SyntheticPapxRun],
    ) -> (u32, u32) {
        assert!(!runs.is_empty());
        assert!(runs.len() <= u8::MAX as usize);

        let page_number = word.len().div_ceil(512);
        word.resize(page_number * 512, 0);
        let mut page = [0u8; 512];
        let bx_base = 4 * (runs.len() + 1);
        let mut papx_offset = (bx_base + 13 * runs.len() + 1) & !1;

        page[..4].copy_from_slice(&(text_fc as u32).to_le_bytes());
        for (index, run) in runs.iter().enumerate() {
            let fc_lim = (text_fc as u32).saturating_add(run.cp_lim.saturating_mul(2));
            page[4 * (index + 1)..4 * (index + 2)].copy_from_slice(&fc_lim.to_le_bytes());

            let data_len = 2 + run.grpprl.len();
            let encoded_len = if data_len % 2 == 1 {
                page[papx_offset] = data_len.div_ceil(2) as u8;
                page[papx_offset + 1..papx_offset + 3].copy_from_slice(&0u16.to_le_bytes());
                page[papx_offset + 3..papx_offset + 3 + run.grpprl.len()]
                    .copy_from_slice(&run.grpprl);
                1 + data_len
            } else {
                page[papx_offset] = 0;
                page[papx_offset + 1] = (data_len / 2) as u8;
                page[papx_offset + 2..papx_offset + 4].copy_from_slice(&0u16.to_le_bytes());
                page[papx_offset + 4..papx_offset + 4 + run.grpprl.len()]
                    .copy_from_slice(&run.grpprl);
                2 + data_len
            };
            assert!(papx_offset + encoded_len < 511);
            page[bx_base + index * 13] = (papx_offset / 2) as u8;
            papx_offset = (papx_offset + encoded_len + 1) & !1;
        }
        page[511] = runs.len() as u8;
        word.extend_from_slice(&page);

        let offset = table.len() as u32;
        table.extend_from_slice(&(text_fc as u32).to_le_bytes());
        let last_cp = runs.last().expect("non-empty PAPX runs").cp_lim;
        let fc_lim = (text_fc as u32).saturating_add(last_cp.saturating_mul(2));
        table.extend_from_slice(&fc_lim.to_le_bytes());
        table.extend_from_slice(&(page_number as u32).to_le_bytes());
        (offset, 12)
    }

    fn append_plc_with_u16_records(
        clx: &mut Vec<u8>,
        refs: &[u32],
        last_cp: u32,
        lcb_override: Option<u32>,
    ) -> (u32, u32) {
        let offset = clx.len() as u32;
        for cp in refs {
            clx.extend_from_slice(&cp.to_le_bytes());
        }
        clx.extend_from_slice(&last_cp.to_le_bytes());
        for _ in refs {
            clx.extend_from_slice(&0u16.to_le_bytes());
        }
        let actual_lcb = (refs.len() as u32) * 6 + 4;
        (offset, lcb_override.unwrap_or(actual_lcb))
    }

    fn append_plc_with_spa_records(
        clx: &mut Vec<u8>,
        refs: &[u32],
        last_cp: u32,
        lcb_override: Option<u32>,
    ) -> (u32, u32) {
        let offset = clx.len() as u32;
        for cp in refs {
            clx.extend_from_slice(&cp.to_le_bytes());
        }
        clx.extend_from_slice(&last_cp.to_le_bytes());
        for index in 0..refs.len() {
            clx.extend_from_slice(&(index as u32 + 1).to_le_bytes());
            clx.extend_from_slice(&0i32.to_le_bytes());
            clx.extend_from_slice(&0i32.to_le_bytes());
            clx.extend_from_slice(&0i32.to_le_bytes());
            clx.extend_from_slice(&0i32.to_le_bytes());
            clx.extend_from_slice(&0u16.to_le_bytes());
            clx.extend_from_slice(&0u32.to_le_bytes());
        }
        let actual_lcb = (refs.len() as u32) * 30 + 4;
        (offset, lcb_override.unwrap_or(actual_lcb))
    }

    fn append_plcf_sed(
        clx: &mut Vec<u8>,
        cps: &[u32],
        sepx_offsets: Option<&[i32]>,
        lcb_override: Option<u32>,
    ) -> (u32, u32) {
        let offset = clx.len() as u32;
        for cp in cps {
            clx.extend_from_slice(&cp.to_le_bytes());
        }
        let section_count = cps.len().saturating_sub(1);
        if let Some(offsets) = sepx_offsets {
            assert_eq!(offsets.len(), section_count);
        }
        for index in 0..section_count {
            clx.extend_from_slice(&0u16.to_le_bytes());
            clx.extend_from_slice(
                &sepx_offsets
                    .and_then(|offsets| offsets.get(index))
                    .copied()
                    .unwrap_or_default()
                    .to_le_bytes(),
            );
            clx.extend_from_slice(&0u16.to_le_bytes());
            clx.extend_from_slice(&0i32.to_le_bytes());
        }
        let actual_lcb = (cps.len() as u32)
            .saturating_mul(4)
            .saturating_add((cps.len().saturating_sub(1) as u32).saturating_mul(12));
        (offset, lcb_override.unwrap_or(actual_lcb))
    }

    fn two_section_legacy_header_footer_doc(plcf_sed_lcb_override: Option<u32>) -> Vec<u8> {
        let plcf_hdd = [
            0, 0, 0, 0, 0, 0, 0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 24,
        ];
        let plcf_sed = [0, 5, 10];
        synth_doc_with_ccp_plcfhdd_and_plcfsed(
            "ABCDEabcdeE0O0e0o0F0f0E1O1e1o1F1f1",
            [10, 0, 24, 0, 0, 0],
            &plcf_hdd,
            &plcf_sed,
            plcf_sed_lcb_override,
        )
    }

    #[cfg(feature = "docx")]
    fn two_section_legacy_running_column_break_doc() -> Vec<u8> {
        let mut text = "ABCDEabcde".to_string();
        let main_len = text.encode_utf16().count() as u32;
        let mut story_ends = Vec::with_capacity(12);
        for marker in [
            "E0", "O0", "e0", "o0", "F0", "f0", "E1", "O1", "e1", "o1", "F1", "f1",
        ] {
            text.push_str(marker);
            text.push('A');
            text.push('\u{000e}');
            text.push_str(marker);
            text.push('B');
            text.push('\r');
            story_ends.push(text.encode_utf16().count() as u32 - main_len);
        }

        let mut plcf_hdd = vec![0u32; 7];
        plcf_hdd.extend_from_slice(&story_ends);
        plcf_hdd.push(*story_ends.last().unwrap());
        synth_doc_with_ccp_plcfhdd_and_plcfsed(
            &text,
            [main_len, 0, *story_ends.last().unwrap(), 0, 0, 0],
            &plcf_hdd,
            &[0, 5, 10],
            None,
        )
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn legacy_running_surface_absolute_spacing_doc(
        story_position: usize,
        line_spacing: (u16, u16),
    ) -> Vec<u8> {
        assert!(story_position < 6);
        let mut text = "PAGE1\u{c}PAGE2\u{c}PAGE3\r".to_string();
        let main_len = text.encode_utf16().count() as u32;
        let mut runs = vec![SyntheticPapxRun {
            cp_lim: main_len,
            grpprl: Vec::new(),
        }];
        for (index, label) in ["EH", "OH", "EF", "OF", "FH", "FF"].into_iter().enumerate() {
            text.push_str(label);
            text.push('\r');
            let mut grpprl = Vec::new();
            if index == story_position {
                push_paragraph_line_spacing(&mut grpprl, line_spacing.0, line_spacing.1);
            }
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl,
            });
        }
        let plcf_hdd = [0, 0, 0, 0, 0, 0, 0, 3, 6, 9, 12, 15, 18, 18];

        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [main_len, 0, 18, 0, 0, 0],
            SyntheticDocTables {
                plcf_hdd_cps: Some(&plcf_hdd),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(feature = "render")]
    fn legacy_running_surface_distance_doc(
        header_twips: Option<u16>,
        footer_twips: Option<u16>,
    ) -> Vec<u8> {
        let mut text = "PAGE1\u{c}PAGE2\u{c}PAGE3\r".to_string();
        let main_len = text.encode_utf16().count() as u32;
        for label in ["EH", "OH", "EF", "OF", "FH", "FF"] {
            text.push_str(label);
            text.push('\r');
        }
        let plcf_hdd = [0, 0, 0, 0, 0, 0, 0, 3, 6, 9, 12, 15, 18, 18];
        let plcf_sed = [0, main_len];
        let mut grpprl = Vec::new();
        if let Some(value) = header_twips {
            grpprl.extend_from_slice(&0xB017u16.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        }
        if let Some(value) = footer_twips {
            grpprl.extend_from_slice(&0xB018u16.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        }
        let sepx_grpprls = [&grpprl[..]];

        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [main_len, 0, 18, 0, 0, 0],
            SyntheticDocTables {
                plcf_hdd_cps: Some(&plcf_hdd),
                plcf_sed_cps: Some(&plcf_sed),
                plcf_sed_sepx_grpprls: Some(&sepx_grpprls),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn legacy_running_table_absolute_spacing_doc(
        story_position: usize,
        line_spacing: (u16, u16),
    ) -> Vec<u8> {
        assert!(story_position < 6);
        let mut text = "PAGE1\u{c}PAGE2\u{c}PAGE3\r".to_string();
        let main_len = text.encode_utf16().count() as u32;
        let mut runs = vec![SyntheticPapxRun {
            cp_lim: main_len,
            grpprl: Vec::new(),
        }];
        for (index, label) in ["EH", "OH", "EF", "OF", "FH", "FF"].into_iter().enumerate() {
            if index == story_position {
                text.push('T');
                text.push('\u{7}');
                let mut cell_grpprl = vec![
                    0x16, 0x24, 0x01, // sprmPFInTable
                ];
                push_paragraph_line_spacing(&mut cell_grpprl, line_spacing.0, line_spacing.1);
                runs.push(SyntheticPapxRun {
                    cp_lim: text.encode_utf16().count() as u32,
                    grpprl: cell_grpprl,
                });

                text.push('\u{7}');
                let mut row_grpprl = vec![
                    0x16, 0x24, 0x01, // sprmPFInTable
                    0x17, 0x24, 0x01, // sprmPFTtp
                    0x08, 0xD6, 0x1A, 0x00, // sprmTDefTable, cb=26
                    0x01, // one cell
                    0x00, 0x00, 0xD0, 0x07, // cell boundaries 0..2000 twips
                ];
                row_grpprl.extend_from_slice(&[0u8; 20]);
                runs.push(SyntheticPapxRun {
                    cp_lim: text.encode_utf16().count() as u32,
                    grpprl: row_grpprl,
                });
            } else {
                text.push_str(label);
                text.push('\r');
                runs.push(SyntheticPapxRun {
                    cp_lim: text.encode_utf16().count() as u32,
                    grpprl: Vec::new(),
                });
            }
        }
        let plcf_hdd = [0, 0, 0, 0, 0, 0, 0, 3, 6, 9, 12, 15, 18, 18];

        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [main_len, 0, 18, 0, 0, 0],
            SyntheticDocTables {
                plcf_hdd_cps: Some(&plcf_hdd),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn legacy_running_tabs_doc(
        story_position: usize,
        in_table: bool,
        include_tabs: bool,
    ) -> Vec<u8> {
        assert!(story_position < 6);
        let mut text = "PAGE1\u{c}PAGE2\u{c}PAGE3\r".to_string();
        let main_len = text.encode_utf16().count() as u32;
        let mut runs = vec![SyntheticPapxRun {
            cp_lim: main_len,
            grpprl: Vec::new(),
        }];
        let mut story_ends = Vec::with_capacity(6);

        for (index, label) in ["EH", "OH", "EF", "OF", "FH", "FF"].into_iter().enumerate() {
            if index == story_position && in_table {
                text.push_str("T\tX");
                text.push('\u{7}');
                let mut cell_grpprl = vec![
                    0x16, 0x24, 0x01, // sprmPFInTable
                ];
                if include_tabs {
                    push_legacy_tab_change(&mut cell_grpprl, 0xC60D, &[], &[(1200, 0x0A)]);
                }
                runs.push(SyntheticPapxRun {
                    cp_lim: text.encode_utf16().count() as u32,
                    grpprl: cell_grpprl,
                });

                text.push('\u{7}');
                let mut row_grpprl = vec![
                    0x16, 0x24, 0x01, // sprmPFInTable
                    0x17, 0x24, 0x01, // sprmPFTtp
                    0x08, 0xD6, 0x1A, 0x00, // sprmTDefTable, cb=26
                    0x01, // one cell
                    0x00, 0x00, 0xD0, 0x07, // cell boundaries 0..2000 twips
                ];
                row_grpprl.extend_from_slice(&[0u8; 20]);
                runs.push(SyntheticPapxRun {
                    cp_lim: text.encode_utf16().count() as u32,
                    grpprl: row_grpprl,
                });
            } else {
                text.push_str(label);
                if index == story_position {
                    text.push('\t');
                    text.push('X');
                }
                text.push('\r');
                let mut grpprl = Vec::new();
                if index == story_position && include_tabs {
                    push_legacy_tab_change(&mut grpprl, 0xC615, &[], &[(1080, 0x1B)]);
                }
                runs.push(SyntheticPapxRun {
                    cp_lim: text.encode_utf16().count() as u32,
                    grpprl,
                });
            }
            story_ends.push(text.encode_utf16().count() as u32 - main_len);
        }

        let mut plcf_hdd = vec![0u32; 7];
        plcf_hdd.extend_from_slice(&story_ends);
        plcf_hdd.push(*story_ends.last().unwrap());
        let ccp_hdd = *story_ends.last().unwrap();
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [main_len, 0, ccp_hdd, 0, 0, 0],
            SyntheticDocTables {
                plcf_hdd_cps: Some(&plcf_hdd),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(feature = "docx")]
    fn legacy_running_pagination_doc(
        story_position: usize,
        in_table: bool,
        include_pagination: bool,
    ) -> Vec<u8> {
        assert!(story_position < 6);
        let mut text = "PAGE1\u{c}PAGE2\u{c}PAGE3\r".to_string();
        let main_len = text.encode_utf16().count() as u32;
        let mut runs = vec![SyntheticPapxRun {
            cp_lim: main_len,
            grpprl: Vec::new(),
        }];

        for (index, label) in ["EH", "OH", "EF", "OF", "FH", "FF"].into_iter().enumerate() {
            if index == story_position && in_table {
                text.push('T');
                text.push('\u{7}');
                let mut cell_grpprl = vec![
                    0x16, 0x24, 0x01, // sprmPFInTable
                ];
                if include_pagination {
                    cell_grpprl.extend_from_slice(&[
                        0x05, 0x24, 0x01, // sprmPFKeep
                        0x06, 0x24, 0x01, // sprmPFKeepFollow
                        0x31, 0x24, 0x00, // sprmPFWidowControl
                    ]);
                }
                runs.push(SyntheticPapxRun {
                    cp_lim: text.encode_utf16().count() as u32,
                    grpprl: cell_grpprl,
                });

                text.push('\u{7}');
                let mut row_grpprl = vec![
                    0x16, 0x24, 0x01, // sprmPFInTable
                    0x17, 0x24, 0x01, // sprmPFTtp
                ];
                if include_pagination {
                    row_grpprl.extend_from_slice(&[
                        0x66, 0x34, 0x01, // sprmTFCantSplit
                    ]);
                }
                row_grpprl.extend_from_slice(&[
                    0x08, 0xD6, 0x1A, 0x00, // sprmTDefTable, cb=26
                    0x01, // one cell
                    0x00, 0x00, 0xD0, 0x07, // cell boundaries 0..2000 twips
                ]);
                row_grpprl.extend_from_slice(&[0u8; 20]);
                runs.push(SyntheticPapxRun {
                    cp_lim: text.encode_utf16().count() as u32,
                    grpprl: row_grpprl,
                });
            } else {
                text.push_str(label);
                text.push('\r');
                let mut grpprl = Vec::new();
                if index == story_position && include_pagination {
                    grpprl.extend_from_slice(&[
                        0x05, 0x24, 0x01, // sprmPFKeep
                        0x06, 0x24, 0x01, // sprmPFKeepFollow
                        0x31, 0x24, 0x00, // sprmPFWidowControl
                    ]);
                }
                runs.push(SyntheticPapxRun {
                    cp_lim: text.encode_utf16().count() as u32,
                    grpprl,
                });
            }
        }
        let plcf_hdd = [0, 0, 0, 0, 0, 0, 0, 3, 6, 9, 12, 15, 18, 18];

        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [main_len, 0, 18, 0, 0, 0],
            SyntheticDocTables {
                plcf_hdd_cps: Some(&plcf_hdd),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(feature = "render")]
    fn legacy_note_tabs_doc(kind: SourceRegionKind, in_table: bool, include_tabs: bool) -> Vec<u8> {
        assert!(matches!(
            kind,
            SourceRegionKind::Footnote | SourceRegionKind::Endnote
        ));
        let mut text = "BODY\r".to_string();
        let main_len = text.encode_utf16().count() as u32;
        let mut runs = vec![SyntheticPapxRun {
            cp_lim: main_len,
            grpprl: Vec::new(),
        }];

        if in_table {
            text.push_str("T\tX");
            text.push('\u{7}');
            let mut cell_grpprl = vec![
                0x16, 0x24, 0x01, // sprmPFInTable
            ];
            if include_tabs {
                push_legacy_tab_change(&mut cell_grpprl, 0xC60D, &[], &[(1200, 0x0A)]);
            }
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl: cell_grpprl,
            });

            text.push('\u{7}');
            let mut row_grpprl = vec![
                0x16, 0x24, 0x01, // sprmPFInTable
                0x17, 0x24, 0x01, // sprmPFTtp
                0x08, 0xD6, 0x1A, 0x00, // sprmTDefTable, cb=26
                0x01, // one cell
                0x00, 0x00, 0xD0, 0x07, // cell boundaries 0..2000 twips
            ];
            row_grpprl.extend_from_slice(&[0u8; 20]);
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl: row_grpprl,
            });
        } else {
            text.push_str("N\tX\r");
            let mut grpprl = Vec::new();
            if include_tabs {
                push_legacy_tab_change(&mut grpprl, 0xC615, &[], &[(1080, 0x1B)]);
            }
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl,
            });
        }

        let note_len = text.encode_utf16().count() as u32 - main_len;
        let ccp = match kind {
            SourceRegionKind::Footnote => [main_len, note_len, 0, 0, 0, 0],
            SourceRegionKind::Endnote => [main_len, 0, 0, 0, note_len, 0],
            _ => unreachable!(),
        };
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            ccp,
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(feature = "render")]
    fn render_opened_document_without_body_line_spacing(
        document: &Document,
        fonts: &[Vec<u8>],
    ) -> Vec<u8> {
        let features = document.report().features;
        let shapes = document.floating_shapes();
        document.with_render_model_and_hints(|model, mut source_hints| {
            source_hints.line_spacing = &[];
            render::to_pdf_with_fonts_and_features_and_shapes(
                model,
                fonts,
                features,
                &shapes,
                source_hints,
            )
        })
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn legacy_running_spacing_at(
        hints: &model::RunningSurfaceLineSpacingHints,
        story_position: usize,
    ) -> &[Option<crate::model::LineSpacingHint>] {
        match story_position {
            0 => &hints.even_header,
            1 => &hints.header,
            2 => &hints.even_footer,
            3 => &hints.footer,
            4 => &hints.first_header,
            _ => &hints.first_footer,
        }
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn legacy_running_table_spacing_at(
        hints: &model::RunningSurfaceLineSpacingHints,
        story_position: usize,
    ) -> &[crate::model::TableCellLineSpacingHints] {
        match story_position {
            0 => &hints.even_header_table_cells,
            1 => &hints.header_table_cells,
            2 => &hints.even_footer_table_cells,
            3 => &hints.footer_table_cells,
            4 => &hints.first_header_table_cells,
            _ => &hints.first_footer_table_cells,
        }
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn legacy_running_tabs_at(
        hints: &model::RunningSurfaceTabStopHints,
        story_position: usize,
    ) -> &[Vec<crate::model::TabStop>] {
        match story_position {
            0 => &hints.even_header,
            1 => &hints.header,
            2 => &hints.even_footer,
            3 => &hints.footer,
            4 => &hints.first_header,
            _ => &hints.first_footer,
        }
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn legacy_running_table_tabs_at(
        hints: &model::RunningSurfaceTableCellTabStopHints,
        story_position: usize,
    ) -> &[crate::model::TableCellTabStopHints] {
        match story_position {
            0 => &hints.even_header,
            1 => &hints.header,
            2 => &hints.even_footer,
            3 => &hints.footer,
            4 => &hints.first_header,
            _ => &hints.first_footer,
        }
    }

    #[cfg(feature = "render")]
    fn assert_legacy_note_tab_overlay(document: &Document, kind: SourceRegionKind, in_table: bool) {
        let state = match &document.backend {
            Backend::Doc(state) => state,
            #[cfg(feature = "docx")]
            Backend::Docx(_) => panic!("synthetic legacy document must use the DOC backend"),
        };
        let assembled = legacy_build_output_from_doc_state(state);
        assert_eq!(assembled.tab_stops.len(), assembled.model.blocks.len());
        assert_eq!(
            assembled.table_cell_tab_stops.len(),
            assembled.model.blocks.len()
        );
        assert!(assembled.tab_stops.iter().all(Vec::is_empty));
        assert!(assembled.table_cell_tab_stops.iter().all(Vec::is_empty));
        assert_eq!(
            assembled.render_tab_stops.len(),
            assembled.model.blocks.len()
        );
        assert_eq!(
            assembled.render_table_cell_tab_stops.len(),
            assembled.model.blocks.len()
        );

        let region = assembled.model.source_regions(kind).next().unwrap();
        assert_eq!(region.block_end, region.block_start + 1);
        if in_table {
            assert!(assembled.render_tab_stops.iter().all(Vec::is_empty));
            assert_eq!(
                assembled
                    .render_table_cell_tab_stops
                    .iter()
                    .filter(|tabs| !tabs.is_empty())
                    .count(),
                1
            );
            assert_eq!(
                assembled.render_table_cell_tab_stops[region.block_start],
                vec![vec![vec![vec![crate::model::TabStop {
                    position_pt: 60.0,
                    alignment: crate::model::TabAlignment::Right,
                    leader: crate::model::TabLeader::Dot,
                }]]]]
            );
        } else {
            assert!(assembled
                .render_table_cell_tab_stops
                .iter()
                .all(Vec::is_empty));
            assert_eq!(
                assembled
                    .render_tab_stops
                    .iter()
                    .filter(|tabs| !tabs.is_empty())
                    .count(),
                1
            );
            assert_eq!(
                assembled.render_tab_stops[region.block_start],
                vec![crate::model::TabStop {
                    position_pt: 54.0,
                    alignment: crate::model::TabAlignment::Decimal,
                    leader: crate::model::TabLeader::Underscore,
                }]
            );
        }
    }

    fn section_page_grpprl(
        width_twips: u16,
        height_twips: u16,
        left_twips: u16,
        right_twips: u16,
        top_twips: i16,
        bottom_twips: i16,
        landscape: bool,
    ) -> Vec<u8> {
        let mut grpprl = Vec::new();
        grpprl.extend_from_slice(&0x301Du16.to_le_bytes());
        grpprl.push(if landscape { 2 } else { 1 });
        for (sprm, value) in [
            (0xB01Fu16, width_twips),
            (0xB020, height_twips),
            (0xB021, left_twips),
            (0xB022, right_twips),
        ] {
            grpprl.extend_from_slice(&sprm.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        }
        for (sprm, value) in [(0x9023u16, top_twips), (0x9024, bottom_twips)] {
            grpprl.extend_from_slice(&sprm.to_le_bytes());
            grpprl.extend_from_slice(&value.to_le_bytes());
        }
        grpprl
    }

    fn push_section_break_kind(grpprl: &mut Vec<u8>, kind: u8) {
        grpprl.extend_from_slice(&0x3009u16.to_le_bytes());
        grpprl.push(kind);
    }

    fn push_section_column_count(grpprl: &mut Vec<u8>, columns_minus_one: u16) {
        grpprl.extend_from_slice(&0x500Bu16.to_le_bytes());
        grpprl.extend_from_slice(&columns_minus_one.to_le_bytes());
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn push_section_column_spacing(grpprl: &mut Vec<u8>, spacing_twips: u16) {
        grpprl.extend_from_slice(&0x900Cu16.to_le_bytes());
        grpprl.extend_from_slice(&spacing_twips.to_le_bytes());
    }

    fn push_section_evenly_spaced(grpprl: &mut Vec<u8>, evenly_spaced: u8) {
        grpprl.extend_from_slice(&0x3005u16.to_le_bytes());
        grpprl.push(evenly_spaced);
    }

    fn push_section_column_width(grpprl: &mut Vec<u8>, index: u8, width_twips: u16) {
        grpprl.extend_from_slice(&0xF203u16.to_le_bytes());
        grpprl.push(index);
        grpprl.extend_from_slice(&width_twips.to_le_bytes());
    }

    fn push_section_column_custom_spacing(grpprl: &mut Vec<u8>, index: u8, spacing_twips: u16) {
        grpprl.extend_from_slice(&0xF204u16.to_le_bytes());
        grpprl.push(index);
        grpprl.extend_from_slice(&spacing_twips.to_le_bytes());
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn push_section_column_separator(grpprl: &mut Vec<u8>, separator: u8) {
        grpprl.extend_from_slice(&0x3019u16.to_le_bytes());
        grpprl.push(separator);
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn push_section_column_rtl(grpprl: &mut Vec<u8>, rtl: u8) {
        grpprl.extend_from_slice(&0x3228u16.to_le_bytes());
        grpprl.push(rtl);
    }

    fn push_section_title_page(grpprl: &mut Vec<u8>, title_page: u8) {
        grpprl.extend_from_slice(&0x300Au16.to_le_bytes());
        grpprl.push(title_page);
    }

    fn push_section_page_number_format(grpprl: &mut Vec<u8>, format: u8) {
        grpprl.extend_from_slice(&0x300Eu16.to_le_bytes());
        grpprl.push(format);
    }

    fn push_section_page_number_restart(grpprl: &mut Vec<u8>, restart: u8) {
        grpprl.extend_from_slice(&0x3011u16.to_le_bytes());
        grpprl.push(restart);
    }

    fn push_section_page_number_start97(grpprl: &mut Vec<u8>, start: u16) {
        grpprl.extend_from_slice(&0x501Cu16.to_le_bytes());
        grpprl.extend_from_slice(&start.to_le_bytes());
    }

    fn push_section_page_number_start(grpprl: &mut Vec<u8>, start: u32) {
        grpprl.extend_from_slice(&0x7044u16.to_le_bytes());
        grpprl.extend_from_slice(&start.to_le_bytes());
    }

    fn push_section_document_grid_mode(grpprl: &mut Vec<u8>, mode: u16) {
        grpprl.extend_from_slice(&0x5032u16.to_le_bytes());
        grpprl.extend_from_slice(&mode.to_le_bytes());
    }

    fn push_section_document_grid_line_pitch(grpprl: &mut Vec<u8>, line_pitch: u16) {
        grpprl.extend_from_slice(&0x9031u16.to_le_bytes());
        grpprl.extend_from_slice(&line_pitch.to_le_bytes());
    }

    fn push_section_document_grid_character_space(grpprl: &mut Vec<u8>, character_space: i32) {
        grpprl.extend_from_slice(&0x7030u16.to_le_bytes());
        grpprl.extend_from_slice(&character_space.to_le_bytes());
    }

    fn push_section_text_direction(grpprl: &mut Vec<u8>, text_flow: u16) {
        grpprl.extend_from_slice(&0x5033u16.to_le_bytes());
        grpprl.extend_from_slice(&text_flow.to_le_bytes());
    }

    fn legacy_doc_with_section_page_grpprls(
        text: &str,
        section_cps: &[u32],
        sepx_grpprls: &[&[u8]],
    ) -> Vec<u8> {
        synth_doc_with_ccp_and_tables(
            text,
            "",
            0x00C1,
            0,
            0,
            [text.encode_utf16().count() as u32, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                plcf_sed_cps: Some(section_cps),
                plcf_sed_sepx_grpprls: Some(sepx_grpprls),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn single_paragraph_text(blocks: &[Block]) -> String {
        let [Block::Paragraph(paragraph)] = blocks else {
            panic!("expected exactly one paragraph block, got {blocks:?}");
        };
        paragraph.text()
    }

    #[cfg(feature = "docx")]
    fn docx_part(bytes: &[u8], name: &str) -> String {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut file = zip.by_name(name).unwrap();
        let mut out = String::new();
        file.read_to_string(&mut out).unwrap();
        out
    }

    #[cfg(feature = "docx")]
    fn docx_running_parts(bytes: &[u8]) -> Vec<(String, String)> {
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut parts = Vec::new();
        for index in 0..zip.len() {
            let mut file = zip.by_index(index).unwrap();
            let name = file.name().to_string();
            if (name.starts_with("word/header") || name.starts_with("word/footer"))
                && name.ends_with(".xml")
            {
                let mut xml = String::new();
                file.read_to_string(&mut xml).unwrap();
                parts.push((name, xml));
            }
        }
        parts
    }

    #[cfg(feature = "docx")]
    fn assert_single_running_line_rule(bytes: &[u8], marker: &str, expected: &str) {
        let parts = docx_running_parts(bytes);
        let selected = parts
            .iter()
            .filter(|(_, xml)| xml.contains(marker))
            .collect::<Vec<_>>();
        assert_eq!(
            selected.len(),
            1,
            "expected one generated running part containing {marker:?}: {parts:?}"
        );
        assert!(
            selected[0].1.contains(expected),
            "missing {expected:?} in {}: {}",
            selected[0].0,
            selected[0].1
        );
        assert_eq!(
            parts
                .iter()
                .map(|(_, xml)| {
                    xml.matches(r#"w:lineRule="exact""#).count()
                        + xml.matches(r#"w:lineRule="atLeast""#).count()
                })
                .sum::<usize>(),
            1,
            "unexpected generated running absolute line rule: {parts:?}"
        );
    }

    #[cfg(feature = "docx")]
    fn assert_single_running_tabs(bytes: &[u8], marker: &str, expected: &str) {
        let parts = docx_running_parts(bytes);
        let selected = parts
            .iter()
            .filter(|(_, xml)| xml.contains(marker))
            .collect::<Vec<_>>();
        assert_eq!(
            selected.len(),
            1,
            "expected one generated running part containing {marker:?}: {parts:?}"
        );
        assert!(
            selected[0].1.contains(expected),
            "missing {expected:?} in {}: {}",
            selected[0].0,
            selected[0].1
        );
        assert_eq!(
            parts
                .iter()
                .map(|(_, xml)| xml.matches("<w:tabs>").count())
                .sum::<usize>(),
            1,
            "expected exactly one running tab definition: {parts:?}"
        );
    }

    #[cfg(feature = "docx")]
    fn docx_page_margin_tags(document_xml: &str) -> Vec<&str> {
        document_xml
            .match_indices("<w:pgMar ")
            .map(|(start, _)| {
                let end = document_xml[start..]
                    .find("/>")
                    .map(|offset| start + offset + 2)
                    .expect("closed page-margin element");
                &document_xml[start..end]
            })
            .collect()
    }

    #[cfg(feature = "docx")]
    fn docx_paragraph_with_text<'a>(document_xml: &'a str, text: &str) -> &'a str {
        let marker = format!(">{text}</w:t>");
        let text_start = document_xml
            .find(&marker)
            .unwrap_or_else(|| panic!("missing paragraph text {text:?}: {document_xml}"));
        let start = document_xml[..text_start]
            .rfind("<w:p>")
            .expect("paragraph start");
        let end = document_xml[text_start..]
            .find("</w:p>")
            .map(|offset| text_start + offset + "</w:p>".len())
            .expect("paragraph end");
        &document_xml[start..end]
    }

    fn push_lpx_char_buffer9(out: &mut Vec<u8>, text: &str) {
        let units: Vec<u16> = text.encode_utf16().collect();
        assert!(units.len() <= 9);
        out.extend_from_slice(&(units.len() as u16).to_le_bytes());
        for unit in units.iter().copied() {
            out.extend_from_slice(&unit.to_le_bytes());
        }
        for _ in units.len()..9 {
            out.extend_from_slice(&0u16.to_le_bytes());
        }
    }

    fn push_xst(out: &mut Vec<u8>, text: &str) {
        let units: Vec<u16> = text.encode_utf16().collect();
        assert!(units.len() < 56);
        out.extend_from_slice(&(units.len() as u16).to_le_bytes());
        for unit in units {
            out.extend_from_slice(&unit.to_le_bytes());
        }
    }

    #[test]
    fn extracts_utf16_and_cp1252_pieces() {
        let bytes = synth_doc("안녕 rwml\r세계", " ABC");
        let text = extract_text(&bytes).unwrap();
        assert!(text.contains("안녕 rwml"), "{text:?}");
        assert!(text.contains("세계"), "{text:?}");
        assert!(text.contains("ABC"), "{text:?}");
        // 0x0D became a line break.
        assert_eq!(text, "안녕 rwml\n세계 ABC");
    }

    #[test]
    fn main_text_excludes_nothing_when_all_main() {
        let bytes = synth_doc("본문", "X");
        let doc = Document::open(&bytes).unwrap();
        assert_eq!(doc.main_text(), "본문X");
        assert_eq!(doc.char_count(), 3);
        assert!(!doc.is_complex());
    }

    #[test]
    fn report_warns_when_doc_subdocuments_are_flattened_into_model() {
        let bytes = synth_doc_with_ccp("MAINHEAD", "", 0x00C1, 0, 0, [4, 0, 4, 0, 0, 0]);
        let doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.main_text(), "MAIN");
        assert_eq!(doc.header_text(), "HEAD");
        let model = doc.model();
        assert_eq!(model.regions.len(), 2);
        assert_eq!(model.regions[0].kind, SourceRegionKind::Main);
        assert_eq!(model.regions[1].kind, SourceRegionKind::HeaderFooter);
        let Block::Paragraph(main) = &model.blocks[model.regions[0].block_start] else {
            panic!("expected main paragraph");
        };
        let Block::Paragraph(header) = &model.blocks[model.regions[1].block_start] else {
            panic!("expected header/footer paragraph");
        };
        assert_eq!(main.text(), "MAIN");
        assert_eq!(header.text(), "HEAD");

        let report = doc.report();
        assert!(report.warnings.iter().any(|warning| matches!(
            warning,
            DocumentWarning::LegacyDocFlattenedSubdocuments {
                footnotes: 0,
                headers_footers: 4,
                annotations: 0,
                endnotes: 0,
                text_boxes: 0,
            }
        )));
        assert!(report.to_json().contains(
            r#"{"kind":"LegacyDocFlattenedSubdocuments","footnotes":0,"headers_footers":4,"annotations":0,"endnotes":0,"text_boxes":0}"#
        ));
    }

    #[test]
    fn doc_region_text_uses_exact_fib_subdocument_boundaries() {
        let bytes =
            synth_doc_with_ccp("BODYFTNHEADANNENDBOX", "", 0x00C1, 0, 0, [4, 3, 4, 3, 3, 3]);
        let doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.main_text(), "BODY");
        assert_eq!(doc.footnote_text(), "FTNEND");
        assert_eq!(doc.header_text(), "HEAD");
        assert_eq!(doc.annotation_text(), "ANN");
        assert_eq!(doc.endnote_text(), "END");
        assert_eq!(doc.text_box_text(), "BOX");
    }

    #[test]
    fn doc_model_exposes_legacy_subdocument_regions() {
        let bytes =
            synth_doc_with_ccp("BODYFTNHEADANNENDBOX", "", 0x00C1, 0, 0, [4, 3, 4, 3, 3, 3]);
        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();

        assert_eq!(model.regions.len(), 6);
        let expected = [
            (SourceRegionKind::Main, "BODY", 0, 4),
            (SourceRegionKind::Footnote, "FTN", 4, 3),
            (SourceRegionKind::HeaderFooter, "HEAD", 7, 4),
            (SourceRegionKind::Annotation, "ANN", 11, 3),
            (SourceRegionKind::Endnote, "END", 14, 3),
            (SourceRegionKind::TextBox, "BOX", 17, 3),
        ];
        for (region, (kind, text, source_start_cp, source_len_cp)) in
            model.regions.iter().zip(expected)
        {
            assert_eq!(region.kind, kind);
            assert_eq!(region.source_start_cp, source_start_cp);
            assert_eq!(region.source_len_cp, source_len_cp);
            assert_eq!(region.block_end, region.block_start + 1);
            let Block::Paragraph(paragraph) = &model.blocks[region.block_start] else {
                panic!("expected region paragraph for {kind:?}");
            };
            assert_eq!(paragraph.text(), text);
            assert_eq!(region.text_len, text.chars().count());
        }
    }

    #[test]
    fn doc_model_promotes_legacy_header_footer_region_into_setup_header() {
        let bytes = synth_doc_with_ccp("BODYHEAD", "", 0x00C1, 0, 0, [4, 0, 4, 0, 0, 0]);
        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();

        assert_eq!(doc.main_text(), "BODY");
        assert_eq!(doc.header_text(), "HEAD");
        assert_eq!(model.setup.footer.len(), 0);
        assert_eq!(model.setup.header.len(), 1);
        let Block::Paragraph(header) = &model.setup.header[0] else {
            panic!("expected promoted header paragraph");
        };
        assert_eq!(header.text(), "HEAD");
        assert_eq!(
            model
                .source_regions(SourceRegionKind::HeaderFooter)
                .next()
                .map(|region| model.source_region_text(region)),
            Some("HEAD".to_string())
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_fresh_conversion_projects_only_promoted_running_regions_from_body() {
        let bytes =
            synth_doc_with_ccp("BODYFTNHEADANNENDBOX", "", 0x00C1, 0, 0, [4, 3, 4, 3, 3, 3]);
        let doc = Document::open(&bytes).unwrap();
        let public_model = doc.model();

        let converted = doc.to_docx();
        assert_eq!(converted, doc.to_docx());
        assert_eq!(public_model, doc.model());
        let document_xml = docx_part(&converted, "word/document.xml");
        let mut previous = 0;
        for marker in ["BODY", "FTN", "ANN", "END", "BOX"] {
            let position = document_xml
                .find(&format!(">{marker}</w:t>"))
                .unwrap_or_else(|| panic!("missing retained fallback {marker:?}: {document_xml}"));
            assert!(
                position >= previous,
                "fallback order changed: {document_xml}"
            );
            previous = position;
        }
        assert!(
            !document_xml.contains(">HEAD</w:t>"),
            "promoted header leaked into the generated body: {document_xml}"
        );

        let running_parts = docx_running_parts(&converted);
        assert_eq!(
            running_parts
                .iter()
                .filter(|(_, xml)| xml.contains(">HEAD</w:t>"))
                .count(),
            1,
            "promoted header missing or duplicated: {running_parts:?}"
        );

        let standalone_body = docx_part(&write_docx(&public_model), "word/document.xml");
        assert!(
            standalone_body.contains(">HEAD</w:t>"),
            "standalone model writing must remain model-only: {standalone_body}"
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_running_projection_remaps_all_block_sidecars_and_regions() {
        let bytes =
            synth_doc_with_ccp("BODYFTNHEADANNENDBOX", "", 0x00C1, 0, 0, [4, 3, 4, 3, 3, 3]);
        let doc = Document::open(&bytes).unwrap();
        let Backend::Doc(state) = &doc.backend else {
            panic!("synthetic fixture must use the legacy backend");
        };
        let mut assembled = legacy_build_output_from_doc_state(state);

        assert_eq!(assembled.promoted_running_block_ranges, vec![(2, 3)]);
        assert!(legacy_project_promoted_running_surfaces(&mut assembled));
        assert!(assembled.promoted_running_block_ranges.is_empty());
        assert_eq!(assembled.model.setup.header.len(), 1);
        assert_eq!(
            assembled
                .model
                .blocks
                .iter()
                .map(legacy_doc_block_text)
                .collect::<Vec<_>>(),
            ["BODY", "FTN", "ANN", "END", "BOX"]
        );
        let block_count = assembled.model.blocks.len();
        assert_eq!(assembled.pagination_hints.len(), block_count);
        assert_eq!(assembled.line_spacing_hints.len(), block_count);
        assert_eq!(assembled.tab_stops.len(), block_count);
        assert_eq!(assembled.note_reference_anchors.len(), block_count);
        assert_eq!(assembled.column_break_offsets.len(), block_count);
        assert_eq!(assembled.table_cell_column_break_offsets.len(), block_count);
        assert_eq!(assembled.section_column_gap_pt.len(), block_count);
        assert_eq!(assembled.section_column_layouts.len(), block_count);
        assert_eq!(assembled.section_column_separators.len(), block_count);
        assert_eq!(assembled.section_column_rtl.len(), block_count);
        assert_eq!(assembled.table_row_pagination.len(), block_count);
        assert_eq!(assembled.table_cell_pagination.len(), block_count);
        assert_eq!(assembled.table_cell_line_spacing.len(), block_count);
        assert_eq!(assembled.table_cell_tab_stops.len(), block_count);
        #[cfg(feature = "render")]
        {
            assert_eq!(assembled.render_tab_stops.len(), block_count);
            assert_eq!(assembled.render_table_cell_tab_stops.len(), block_count);
        }

        let expected = [
            (SourceRegionKind::Main, 0, 4),
            (SourceRegionKind::Footnote, 1, 3),
            (SourceRegionKind::Annotation, 2, 3),
            (SourceRegionKind::Endnote, 3, 3),
            (SourceRegionKind::TextBox, 4, 3),
        ];
        let mut text_start = 0;
        for (region, (kind, block_start, text_len)) in assembled.model.regions.iter().zip(expected)
        {
            assert_eq!(region.kind, kind);
            assert_eq!(region.block_start, block_start);
            assert_eq!(region.block_end, block_start + 1);
            assert_eq!(region.text_start, text_start);
            assert_eq!(region.text_len, text_len);
            text_start += text_len;
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_running_projection_rejects_corrupt_ranges_and_sidecars_without_mutation() {
        let bytes = synth_doc_with_ccp("BODYHEAD", "", 0x00C1, 0, 0, [4, 0, 4, 0, 0, 0]);
        let doc = Document::open(&bytes).unwrap();
        let Backend::Doc(state) = &doc.backend else {
            panic!("synthetic fixture must use the legacy backend");
        };

        let mut corrupt_range = legacy_build_output_from_doc_state(state);
        corrupt_range.promoted_running_block_ranges[0].1 = corrupt_range.model.blocks.len() + 1;
        let range_model = corrupt_range.model.clone();
        let range_sidecars = corrupt_range.pagination_hints.clone();
        assert!(!legacy_project_promoted_running_surfaces(
            &mut corrupt_range
        ));
        assert_eq!(corrupt_range.model, range_model);
        assert_eq!(corrupt_range.pagination_hints, range_sidecars);

        let mut overlapping_region = legacy_build_output_from_doc_state(state);
        overlapping_region.model.regions[0].block_end =
            overlapping_region.promoted_running_block_ranges[0].1;
        let overlapping_model = overlapping_region.model.clone();
        assert!(!legacy_project_promoted_running_surfaces(
            &mut overlapping_region
        ));
        assert_eq!(overlapping_region.model, overlapping_model);

        let mut misaligned = legacy_build_output_from_doc_state(state);
        misaligned.table_cell_pagination.pop();
        let misaligned_model = misaligned.model.clone();
        let misaligned_ranges = misaligned.promoted_running_block_ranges.clone();
        assert!(!legacy_project_promoted_running_surfaces(&mut misaligned));
        assert_eq!(misaligned.model, misaligned_model);
        assert_eq!(misaligned.promoted_running_block_ranges, misaligned_ranges);

        let mut misaligned_breaks = legacy_build_output_from_doc_state(state);
        misaligned_breaks.table_cell_column_break_offsets.pop();
        let break_model = misaligned_breaks.model.clone();
        let break_ranges = misaligned_breaks.promoted_running_block_ranges.clone();
        assert!(!legacy_project_promoted_running_surfaces(
            &mut misaligned_breaks
        ));
        assert_eq!(misaligned_breaks.model, break_model);
        assert_eq!(
            misaligned_breaks.promoted_running_block_ranges,
            break_ranges
        );
    }

    #[test]
    fn doc_model_queries_legacy_source_regions() {
        let bytes =
            synth_doc_with_ccp("BODYFTNHEADANNENDBOX", "", 0x00C1, 0, 0, [4, 3, 4, 3, 3, 3]);
        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();

        let header = model
            .source_regions(SourceRegionKind::HeaderFooter)
            .next()
            .expect("header/footer region");
        assert_eq!(model.source_region_text(header), "HEAD");
        assert_eq!(model.source_region_blocks(header).len(), 1);
        assert_eq!(model.source_regions(SourceRegionKind::Footnote).count(), 1);
        assert_eq!(model.source_regions(SourceRegionKind::TextBox).count(), 1);
    }

    #[test]
    fn doc_region_text_apis_use_model_region_text() {
        let bytes =
            synth_doc_with_ccp("BODYFTNHEADANNENDBOX", "", 0x00C1, 0, 0, [4, 3, 4, 3, 3, 3]);
        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();

        assert_eq!(
            model.source_region_kind_text(SourceRegionKind::Main),
            "BODY"
        );
        assert_eq!(
            model.source_region_kind_text(SourceRegionKind::HeaderFooter),
            "HEAD"
        );
        assert_eq!(
            model.source_region_kind_text(SourceRegionKind::Annotation),
            "ANN"
        );
        assert_eq!(
            model.source_region_kind_text(SourceRegionKind::Endnote),
            "END"
        );
        assert_eq!(
            model.source_region_kind_text(SourceRegionKind::TextBox),
            "BOX"
        );
        assert_eq!(
            doc.main_text(),
            model.source_region_kind_text(SourceRegionKind::Main)
        );
        assert_eq!(
            doc.footnote_text(),
            format!(
                "{}{}",
                model.source_region_kind_text(SourceRegionKind::Footnote),
                model.source_region_kind_text(SourceRegionKind::Endnote)
            )
        );
        assert_eq!(
            doc.header_text(),
            model.source_region_kind_text(SourceRegionKind::HeaderFooter)
        );
        assert_eq!(
            doc.annotation_text(),
            model.source_region_kind_text(SourceRegionKind::Annotation)
        );
        assert_eq!(
            doc.endnote_text(),
            model.source_region_kind_text(SourceRegionKind::Endnote)
        );
        assert_eq!(
            doc.text_box_text(),
            model.source_region_kind_text(SourceRegionKind::TextBox)
        );
    }

    #[test]
    fn legacy_doc_annotation_region_is_exposed_as_comment_side_table() {
        let bytes = synth_doc_with_ccp("BODYANN", "", 0x00C1, 0, 0, [4, 0, 0, 3, 0, 0]);
        let doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.main_text(), "BODY");
        assert_eq!(doc.annotation_text(), "ANN");
        let comments = doc.comments();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].id, "legacy-doc-annotation-0");
        assert_eq!(comments[0].text, "ANN");
        assert_eq!(comments[0].author, None);
        assert_eq!(comments[0].initials, None);
        assert_eq!(
            comments[0].anchor.as_ref().map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-annotation-0@cp4+3")
        );
        assert_eq!(
            comments[0]
                .anchor
                .as_ref()
                .map(|anchor| anchor.text.as_str()),
            Some("ANN")
        );
    }

    #[test]
    fn legacy_doc_comment_author_metadata_uses_annotation_tables() {
        let bytes = synth_doc_with_annotation_tables(
            "BODYONE\rTWO",
            [4, 0, 0, 7, 0, 0],
            &[("R1", 0), ("R2", 1)],
            &["Reviewer One", "Reviewer Two"],
            None,
        );
        let doc = Document::open(&bytes).unwrap();

        let comments = doc.comments();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].text, "ONE");
        assert_eq!(comments[0].author.as_deref(), Some("Reviewer One"));
        assert_eq!(comments[0].initials.as_deref(), Some("R1"));
        assert_eq!(comments[1].text, "TWO");
        assert_eq!(comments[1].author.as_deref(), Some("Reviewer Two"));
        assert_eq!(comments[1].initials.as_deref(), Some("R2"));
    }

    #[test]
    fn legacy_doc_comment_author_metadata_ignores_malformed_owner_table() {
        let bytes = synth_doc_with_annotation_tables(
            "BODYANN",
            [4, 0, 0, 3, 0, 0],
            &[("R1", 0)],
            &["Reviewer One"],
            Some(1),
        );
        let doc = Document::open(&bytes).unwrap();

        let comments = doc.comments();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "ANN");
        assert_eq!(comments[0].author, None);
        assert_eq!(comments[0].initials.as_deref(), Some("R1"));
    }

    #[test]
    fn legacy_doc_comment_author_metadata_ignores_out_of_range_owner_index() {
        let bytes = synth_doc_with_annotation_tables(
            "BODYANN",
            [4, 0, 0, 3, 0, 0],
            &[("R1", 7)],
            &["Reviewer One"],
            None,
        );
        let doc = Document::open(&bytes).unwrap();

        let comments = doc.comments();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].text, "ANN");
        assert_eq!(comments[0].author, None);
        assert_eq!(comments[0].initials.as_deref(), Some("R1"));
    }

    #[test]
    fn legacy_doc_note_regions_are_exposed_as_note_side_table() {
        let bytes = synth_doc_with_ccp("BODYFTNHEADEND", "", 0x00C1, 0, 0, [4, 3, 4, 0, 3, 0]);
        let doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.main_text(), "BODY");
        assert_eq!(doc.footnote_text(), "FTNEND");
        assert_eq!(doc.endnote_text(), "END");
        let notes = doc.notes();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].id, "legacy-doc-footnote-0");
        assert_eq!(notes[0].kind, NoteKind::Footnote);
        assert_eq!(notes[0].text, "FTN");
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-footnote-0@cp4+3")
        );
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.text.as_str()),
            Some("FTN")
        );
        assert_eq!(notes[1].id, "legacy-doc-endnote-0");
        assert_eq!(notes[1].kind, NoteKind::Endnote);
        assert_eq!(notes[1].text, "END");
        assert_eq!(
            notes[1].anchor.as_ref().map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-endnote-0@cp11+3")
        );
        assert_eq!(
            notes[1].anchor.as_ref().map(|anchor| anchor.text.as_str()),
            Some("END")
        );
    }

    #[test]
    fn legacy_doc_single_footnote_marker_anchors_note_to_body_text() {
        let bytes = synth_doc_with_ccp("BO\u{0002}DYFTN", "", 0x00C1, 0, 0, [5, 3, 0, 0, 0, 0]);
        let doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.main_text(), "BODY");
        let notes = doc.notes();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "legacy-doc-footnote-0");
        assert_eq!(notes[0].kind, NoteKind::Footnote);
        assert_eq!(notes[0].text, "FTN");
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-footnote-0@body-cp2")
        );
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.text.as_str()),
            Some("BODY")
        );
        #[cfg(feature = "docx")]
        {
            let body = docx_part(&doc.to_docx(), "word/document.xml");
            assert!(!body.contains("footnoteReference"));
            assert!(body.contains("FTN"));
        }
    }

    #[test]
    fn legacy_doc_single_endnote_marker_anchors_note_to_body_text() {
        let bytes = synth_doc_with_ccp("BO\u{0002}DYEND", "", 0x00C1, 0, 0, [5, 0, 0, 0, 3, 0]);
        let doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.main_text(), "BODY");
        let notes = doc.notes();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "legacy-doc-endnote-0");
        assert_eq!(notes[0].kind, NoteKind::Endnote);
        assert_eq!(notes[0].text, "END");
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-endnote-0@body-cp2")
        );
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.text.as_str()),
            Some("BODY")
        );
    }

    #[test]
    fn legacy_doc_plcffnd_ref_anchors_each_footnote_to_body_reference_cp() {
        let bytes = synth_doc_with_note_reference_tables(
            "A\u{0002}A\rB\u{0002}BONE\rTWO",
            [7, 7, 0, 0, 0, 0],
            Some(&[1, 5]),
            None,
            None,
            None,
        );
        let doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.main_text(), "AABB");
        let notes = doc.notes();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].id, "legacy-doc-footnote-0");
        assert_eq!(notes[0].kind, NoteKind::Footnote);
        assert_eq!(notes[0].text, "ONE");
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-footnote-0@body-cp1")
        );
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.text.as_str()),
            Some("AA")
        );
        assert_eq!(notes[1].id, "legacy-doc-footnote-1");
        assert_eq!(notes[1].kind, NoteKind::Footnote);
        assert_eq!(notes[1].text, "TWO");
        assert_eq!(
            notes[1].anchor.as_ref().map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-footnote-1@body-cp5")
        );
        assert_eq!(
            notes[1].anchor.as_ref().map(|anchor| anchor.text.as_str()),
            Some("BB")
        );
    }

    #[test]
    fn legacy_doc_plcfend_ref_anchors_each_endnote_to_body_reference_cp() {
        let bytes = synth_doc_with_note_reference_tables(
            "A\u{0002}A\rB\u{0002}BONE\rTWO",
            [7, 0, 0, 0, 7, 0],
            None,
            None,
            Some(&[1, 5]),
            None,
        );
        let doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.main_text(), "AABB");
        let notes = doc.notes();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].id, "legacy-doc-endnote-0");
        assert_eq!(notes[0].kind, NoteKind::Endnote);
        assert_eq!(notes[0].text, "ONE");
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-endnote-0@body-cp1")
        );
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.text.as_str()),
            Some("AA")
        );
        assert_eq!(notes[1].id, "legacy-doc-endnote-1");
        assert_eq!(notes[1].kind, NoteKind::Endnote);
        assert_eq!(notes[1].text, "TWO");
        assert_eq!(
            notes[1].anchor.as_ref().map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-endnote-1@body-cp5")
        );
        assert_eq!(
            notes[1].anchor.as_ref().map(|anchor| anchor.text.as_str()),
            Some("BB")
        );
    }

    #[test]
    fn legacy_doc_truncated_plcffnd_ref_keeps_marker_fallback() {
        let bytes = synth_doc_with_note_reference_tables(
            "BO\u{0002}DYFTN",
            [5, 3, 0, 0, 0, 0],
            Some(&[2]),
            Some(5),
            None,
            None,
        );
        let doc = Document::open(&bytes).unwrap();

        let notes = doc.notes();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].id, "legacy-doc-footnote-0");
        assert_eq!(notes[0].text, "FTN");
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-footnote-0@body-cp2")
        );
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.text.as_str()),
            Some("BODY")
        );
        #[cfg(feature = "docx")]
        {
            let body = docx_part(&doc.to_docx(), "word/document.xml");
            assert!(!body.contains("footnoteReference"));
            assert!(body.contains("FTN"));
        }
    }

    #[test]
    fn legacy_doc_mixed_note_marker_keeps_source_region_anchor_when_kind_is_ambiguous() {
        let bytes = synth_doc_with_ccp("BO\u{0002}DYFTNEND", "", 0x00C1, 0, 0, [5, 3, 0, 0, 3, 0]);
        let doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.main_text(), "BODY");
        let notes = doc.notes();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0].kind, NoteKind::Footnote);
        assert_eq!(
            notes[0].anchor.as_ref().map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-footnote-0@cp5+3")
        );
        assert_eq!(notes[1].kind, NoteKind::Endnote);
        assert_eq!(
            notes[1].anchor.as_ref().map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-endnote-0@cp8+3")
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_exact_notes_roundtrip_through_fresh_conversion() {
        let main = "AA\u{0002}BB\rCC\u{0002}DD\r";
        let bytes = synth_doc_with_note_reference_tables(
            &format!("{main}FOOT\rEND\r"),
            [12, 5, 0, 0, 4, 0],
            Some(&[2]),
            None,
            Some(&[8]),
            None,
        );
        let document = Document::open(&bytes).unwrap();
        let public_model = document.model();
        assert_eq!(document.main_text(), "AABBCCDD");
        let state = match &document.backend {
            Backend::Doc(state) => state,
            Backend::Docx(_) => panic!("synthetic legacy document must use the DOC backend"),
        };
        let assembled = legacy_build_output_from_doc_state(state);
        assert_eq!(
            assembled.note_reference_anchors.len(),
            assembled.model.blocks.len()
        );
        assert_eq!(
            assembled
                .note_reference_anchors
                .iter()
                .enumerate()
                .flat_map(|(block_index, anchors)| anchors
                    .iter()
                    .map(move |anchor| { (block_index, anchor.source_cp, anchor.text_offset) }))
                .collect::<Vec<_>>(),
            vec![(0, 2, 2), (1, 8, 2)]
        );
        assert_eq!(
            document
                .notes()
                .into_iter()
                .map(|note| (note.kind, note.text))
                .collect::<Vec<_>>(),
            vec![
                (NoteKind::Footnote, "FOOT".to_string()),
                (NoteKind::Endnote, "END".to_string()),
            ]
        );

        let converted = document.to_docx();
        assert_eq!(converted, document.to_docx());
        let body = docx_part(&converted, "word/document.xml");
        assert!(
            body.contains(r#"<w:footnoteReference w:id="1"/>"#),
            "missing promoted footnote reference: {body}"
        );
        assert!(
            body.contains(r#"<w:endnoteReference w:id="1"/>"#),
            "missing promoted endnote reference: {body}"
        );
        let mut cursor = 0;
        for needle in [
            ">AA</w:t>",
            "<w:footnoteReference",
            ">BB</w:t>",
            ">CC</w:t>",
            "<w:endnoteReference",
            ">DD</w:t>",
        ] {
            let relative = body[cursor..]
                .find(needle)
                .unwrap_or_else(|| panic!("missing ordered fragment {needle:?}: {body}"));
            cursor += relative + needle.len();
        }
        assert!(!body.contains("FOOT"));
        assert!(!body.contains("END"));
        assert!(docx_part(&converted, "word/footnotes.xml").contains("FOOT"));
        assert!(docx_part(&converted, "word/endnotes.xml").contains("END"));

        let reopened = Document::open(&converted).unwrap();
        assert_eq!(reopened.main_text(), "AABB\nCCDD");
        assert_eq!(
            reopened
                .notes()
                .into_iter()
                .map(|note| (note.kind, note.text))
                .collect::<Vec<_>>(),
            vec![
                (NoteKind::Footnote, "FOOT".to_string()),
                (NoteKind::Endnote, "END".to_string()),
            ]
        );
        assert!(
            !docx_part(&write_docx(&reopened.model()), "word/document.xml")
                .contains("footnoteReference")
        );
        assert_eq!(converted, reopened.to_docx());
        assert_eq!(document.model(), public_model);
        assert!(!docx_part(&write_docx(&public_model), "word/document.xml")
            .contains("footnoteReference"));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn reopened_docx_custom_note_mark_keeps_flattened_fresh_conversion_fallback() {
        let bytes = synth_doc_with_note_reference_tables(
            "A\u{0002}B\rNOTE\r",
            [4, 5, 0, 0, 0, 0],
            Some(&[1]),
            None,
            None,
            None,
        );
        let converted = Document::open(&bytes).unwrap().to_docx();
        let mut package = opc::Package::from_zip(&converted).unwrap();
        let document_xml = String::from_utf8(package.part("word/document.xml").unwrap())
            .unwrap()
            .replace(
                r#"<w:footnoteReference w:id="1"/>"#,
                r#"<w:footnoteReference w:id="1" w:customMarkFollows="1"/>"#,
            );
        package.set_part(
            "word/document.xml",
            document_xml.into_bytes(),
            Some(CT_DOCUMENT_MAIN),
        );

        let custom_mark = Document::open(&package.to_zip().unwrap()).unwrap();
        assert_eq!(custom_mark.notes().len(), 1);
        let body = docx_part(&custom_mark.to_docx(), "word/document.xml");
        assert!(!body.contains("footnoteReference"));
        assert!(body.contains("NOTE"));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_note_conversion_fails_closed_across_note_families() {
        let main = "AA\u{0002}BB\rCC\u{0002}DD\r";
        let bytes = synth_doc_with_note_reference_tables(
            &format!("{main}FOOT\rEND\r"),
            [12, 5, 0, 0, 4, 0],
            Some(&[2]),
            None,
            None,
            None,
        );
        let document = Document::open(&bytes).unwrap();
        assert_eq!(document.notes().len(), 2);

        let converted = document.to_docx();
        let body = docx_part(&converted, "word/document.xml");
        assert!(!body.contains("footnoteReference"));
        assert!(!body.contains("endnoteReference"));
        assert!(body.contains("FOOT"));
        assert!(body.contains("END"));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_note_conversion_rejects_marker_and_record_mismatches() {
        let extra_marker = synth_doc_with_note_reference_tables(
            "A\u{0002}B\u{0002}C\rNOTE\r",
            [6, 5, 0, 0, 0, 0],
            Some(&[1]),
            None,
            None,
            None,
        );
        let duplicate_reference = synth_doc_with_note_reference_tables(
            "A\u{0002}B\rONE\r",
            [4, 4, 0, 0, 0, 0],
            Some(&[1, 1]),
            None,
            None,
            None,
        );

        for (label, bytes, note_text) in [
            ("extra marker", extra_marker, "NOTE"),
            (
                "duplicate reference and record-count mismatch",
                duplicate_reference,
                "ONE",
            ),
        ] {
            let body = docx_part(
                &Document::open(&bytes).unwrap().to_docx(),
                "word/document.xml",
            );
            assert!(
                !body.contains("footnoteReference"),
                "{label} must preserve the complete flattened fallback: {body}"
            );
            assert!(body.contains(note_text), "{label}: {body}");
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_note_conversion_rejects_unsupported_body_contexts() {
        let note = "NOTE\r";

        let manual_main = "A\u{0002}B\u{000c}C\r";
        let manual = synth_doc_with_note_reference_tables(
            &format!("{manual_main}{note}"),
            [
                manual_main.encode_utf16().count() as u32,
                note.encode_utf16().count() as u32,
                0,
                0,
                0,
                0,
            ],
            Some(&[1]),
            None,
            None,
            None,
        );

        let side_table_main = "A\u{0002}B\r";
        let annotation = "ANN\r";
        let text_box = "BOX\r";
        let side_tables = synth_doc_with_note_reference_tables(
            &format!("{side_table_main}{note}{annotation}{text_box}"),
            [
                side_table_main.encode_utf16().count() as u32,
                note.encode_utf16().count() as u32,
                0,
                annotation.encode_utf16().count() as u32,
                0,
                text_box.encode_utf16().count() as u32,
            ],
            Some(&[1]),
            None,
            None,
            None,
        );

        let table_main = "A\u{0002}B\u{0007}\u{0007}";
        let table_main_len = table_main.encode_utf16().count() as u32;
        let mut row_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
            0x17, 0x24, 0x01, // sprmPFTtp
            0x08, 0xD6, 0x1A, 0x00, // sprmTDefTable, cb=26
            0x01, // one cell
            0x00, 0x00, 0xD0, 0x07, // cell boundaries 0..2000 twips
        ];
        row_grpprl.extend_from_slice(&[0u8; 20]);
        let table_text = format!("{table_main}{note}");
        let table_runs = vec![
            SyntheticPapxRun {
                cp_lim: 4,
                grpprl: vec![0x16, 0x24, 0x01],
            },
            SyntheticPapxRun {
                cp_lim: table_main_len,
                grpprl: row_grpprl,
            },
            SyntheticPapxRun {
                cp_lim: table_text.encode_utf16().count() as u32,
                grpprl: Vec::new(),
            },
        ];
        let table_cell = synth_doc_with_ccp_and_tables(
            &table_text,
            "",
            0x00C1,
            0,
            0,
            [
                table_main_len,
                note.encode_utf16().count() as u32,
                0,
                0,
                0,
                0,
            ],
            SyntheticDocTables {
                footnote_ref_cps: Some(&[1]),
                papx_runs: Some(&table_runs),
                ..SyntheticDocTables::default()
            },
        );

        for (label, bytes, extra_text) in [
            ("manual page break", manual, None),
            ("annotation and text box", side_tables, Some(("ANN", "BOX"))),
            ("table cell", table_cell, None),
        ] {
            let body = docx_part(
                &Document::open(&bytes).unwrap().to_docx(),
                "word/document.xml",
            );
            assert!(
                !body.contains("footnoteReference"),
                "{label} must preserve the complete flattened fallback: {body}"
            );
            assert!(body.contains("NOTE"), "{label}: {body}");
            if let Some((first, second)) = extra_text {
                assert!(body.contains(first), "{label}: {body}");
                assert!(body.contains(second), "{label}: {body}");
            }
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_note_run_splitting_preserves_order_and_rejects_ambiguity() {
        let authored = |kind, text: &str| AuthoredNote {
            kind,
            text: text.to_string(),
        };
        let mut paragraph = Paragraph {
            runs: vec![Run {
                text: "AABBCC".to_string(),
                ..Run::default()
            }],
            ..Paragraph::default()
        };
        assert!(attach_authored_notes_to_paragraph(
            &mut paragraph,
            vec![
                (2, authored(NoteKind::Footnote, "FOOT")),
                (4, authored(NoteKind::Endnote, "END")),
            ]
        ));
        assert_eq!(
            paragraph
                .runs
                .iter()
                .map(|run| {
                    (
                        run.text.as_str(),
                        run.note
                            .as_ref()
                            .map(|note| (note.kind, note.text.as_str())),
                    )
                })
                .collect::<Vec<_>>(),
            vec![
                ("AA", Some((NoteKind::Footnote, "FOOT"))),
                ("BB", Some((NoteKind::Endnote, "END"))),
                ("CC", None),
            ]
        );

        let duplicate_original = Paragraph {
            runs: vec![Run {
                text: "AB".to_string(),
                ..Run::default()
            }],
            ..Paragraph::default()
        };
        let mut duplicate = duplicate_original.clone();
        assert!(!attach_authored_notes_to_paragraph(
            &mut duplicate,
            vec![
                (1, authored(NoteKind::Footnote, "ONE")),
                (1, authored(NoteKind::Endnote, "TWO")),
            ]
        ));
        assert_eq!(duplicate, duplicate_original);

        let wrapped_original = Paragraph {
            runs: vec![Run {
                text: "AB".to_string(),
                field: FieldRole::Other,
                ..Run::default()
            }],
            ..Paragraph::default()
        };
        let mut wrapped = wrapped_original.clone();
        assert!(!attach_authored_notes_to_paragraph(
            &mut wrapped,
            vec![(1, authored(NoteKind::Footnote, "NOTE"))]
        ));
        assert_eq!(wrapped, wrapped_original);
    }

    #[test]
    fn legacy_doc_text_box_region_is_exposed_as_text_box_side_table() {
        let bytes = synth_doc_with_ccp("BODYBOX", "", 0x00C1, 0, 0, [4, 0, 0, 0, 0, 3]);
        let doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.main_text(), "BODY");
        assert_eq!(doc.text_box_text(), "BOX");
        let text_boxes = doc.text_boxes();
        assert_eq!(text_boxes.len(), 1);
        assert_eq!(text_boxes[0].id, "legacy-doc-text-box-0");
        assert_eq!(text_boxes[0].text, "BOX");
        assert_eq!(
            text_boxes[0]
                .anchor
                .as_ref()
                .map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-text-box-0@cp4+3")
        );
        assert_eq!(
            text_boxes[0]
                .anchor
                .as_ref()
                .map(|anchor| anchor.text.as_str()),
            Some("BOX")
        );
    }

    #[test]
    fn legacy_doc_plcspa_mom_anchors_each_text_box_to_body_anchor_cp() {
        let bytes = synth_doc_with_shape_anchor_table(
            "A\u{0008}A\rB\u{0008}BONE\rTWO",
            [7, 0, 0, 0, 0, 7],
            Some(&[1, 5]),
            None,
        );
        let doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.main_text(), "AABB");
        let text_boxes = doc.text_boxes();
        assert_eq!(text_boxes.len(), 2);
        assert_eq!(text_boxes[0].id, "legacy-doc-text-box-0");
        assert_eq!(text_boxes[0].text, "ONE");
        assert_eq!(
            text_boxes[0]
                .anchor
                .as_ref()
                .map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-text-box-0@body-cp1")
        );
        assert_eq!(
            text_boxes[0]
                .anchor
                .as_ref()
                .map(|anchor| anchor.text.as_str()),
            Some("AA")
        );
        assert_eq!(text_boxes[1].id, "legacy-doc-text-box-1");
        assert_eq!(text_boxes[1].text, "TWO");
        assert_eq!(
            text_boxes[1]
                .anchor
                .as_ref()
                .map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-text-box-1@body-cp5")
        );
        assert_eq!(
            text_boxes[1]
                .anchor
                .as_ref()
                .map(|anchor| anchor.text.as_str()),
            Some("BB")
        );
    }

    #[test]
    fn legacy_doc_plcspa_mom_count_mismatch_keeps_source_region_anchor() {
        let bytes = synth_doc_with_shape_anchor_table(
            "BO\u{0008}DYBOX",
            [5, 0, 0, 0, 0, 3],
            Some(&[2, 3]),
            None,
        );
        let doc = Document::open(&bytes).unwrap();

        let text_boxes = doc.text_boxes();
        assert_eq!(text_boxes.len(), 1);
        assert_eq!(
            text_boxes[0]
                .anchor
                .as_ref()
                .map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-text-box-0@cp5+3")
        );
        assert_eq!(
            text_boxes[0]
                .anchor
                .as_ref()
                .map(|anchor| anchor.text.as_str()),
            Some("BOX")
        );
    }

    #[test]
    fn legacy_doc_truncated_plcspa_mom_keeps_source_region_anchor() {
        let bytes = synth_doc_with_shape_anchor_table(
            "BO\u{0008}DYBOX",
            [5, 0, 0, 0, 0, 3],
            Some(&[2]),
            Some(10),
        );
        let doc = Document::open(&bytes).unwrap();

        let text_boxes = doc.text_boxes();
        assert_eq!(text_boxes.len(), 1);
        assert_eq!(
            text_boxes[0]
                .anchor
                .as_ref()
                .map(|anchor| anchor.id.as_str()),
            Some("legacy-doc-text-box-0@cp5+3")
        );
        assert_eq!(
            text_boxes[0]
                .anchor
                .as_ref()
                .map(|anchor| anchor.text.as_str()),
            Some("BOX")
        );
    }

    #[test]
    fn legacy_doc_header_footer_region_is_exposed_as_header_footer_side_table() {
        let bytes = synth_doc_with_ccp("BODYHEAD", "", 0x00C1, 0, 0, [4, 0, 4, 0, 0, 0]);
        let doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.main_text(), "BODY");
        assert_eq!(doc.header_text(), "HEAD");
        let header_footers = doc.header_footers();
        assert_eq!(header_footers.len(), 1);
        assert_eq!(header_footers[0].id, "legacy-doc-header-footer-0");
        assert_eq!(header_footers[0].kind, HeaderFooterKind::Unknown);
        assert_eq!(header_footers[0].text, "HEAD");
    }

    #[test]
    fn legacy_doc_plcfhdd_splits_header_footer_stories() {
        // First six PlcfHdd stories are footnote/endnote separators. In the
        // first section group, story 7 is odd-page header and story 9 is
        // odd-page footer.
        let plcf_hdd = [0, 0, 0, 0, 0, 0, 0, 0, 4, 4, 8, 8, 8, 8];
        let bytes = synth_doc_with_ccp_and_plcfhdd(
            "BODYHEADFOOT",
            "",
            0x00C1,
            0,
            0,
            [4, 0, 8, 0, 0, 0],
            Some(&plcf_hdd),
        );
        let doc = Document::open(&bytes).unwrap();

        let header_footers = doc.header_footers();
        assert_eq!(header_footers.len(), 2);
        assert_eq!(header_footers[0].kind, HeaderFooterKind::OddPageHeader);
        assert_eq!(header_footers[0].text, "HEAD");
        assert_eq!(header_footers[1].kind, HeaderFooterKind::OddPageFooter);
        assert_eq!(header_footers[1].text, "FOOT");

        let model = doc.model();
        let regions: Vec<_> = model
            .source_regions(SourceRegionKind::HeaderFooter)
            .collect();
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].source_start_cp, 4);
        assert_eq!(regions[0].source_len_cp, 4);
        assert_eq!(regions[0].source_story_index, Some(7));
        assert_eq!(regions[1].source_start_cp, 8);
        assert_eq!(regions[1].source_len_cp, 4);
        assert_eq!(regions[1].source_story_index, Some(9));
        assert_eq!(model.source_region_text(regions[0]), "HEAD");
    }

    #[test]
    fn legacy_doc_header_footer_side_table_ids_skip_empty_stories() {
        // Story 6 is an empty even-page header paragraph; story 7 is the first
        // visible header/footer record. Public side-table ids should stay dense.
        let plcf_hdd = [0, 0, 0, 0, 0, 0, 0, 1, 5, 5, 5, 5, 5, 5];
        let bytes = synth_doc_with_ccp_and_plcfhdd(
            "BODY\rFOOT",
            "",
            0x00C1,
            0,
            0,
            [4, 0, 5, 0, 0, 0],
            Some(&plcf_hdd),
        );
        let doc = Document::open(&bytes).unwrap();

        let header_footers = doc.header_footers();
        assert_eq!(header_footers.len(), 1);
        assert_eq!(header_footers[0].id, "legacy-doc-header-footer-0");
        assert_eq!(header_footers[0].kind, HeaderFooterKind::OddPageHeader);
        assert_eq!(header_footers[0].text, "FOOT");
    }

    #[test]
    fn legacy_doc_plcfhdd_maps_all_header_footer_story_variants() {
        let plcf_hdd = [0, 0, 0, 0, 0, 0, 0, 2, 4, 6, 8, 10, 12, 12];
        let bytes = synth_doc_with_ccp_and_plcfhdd(
            "BODYEHOHEFOFFHFF",
            "",
            0x00C1,
            0,
            0,
            [4, 0, 12, 0, 0, 0],
            Some(&plcf_hdd),
        );
        let doc = Document::open(&bytes).unwrap();

        let header_footers = doc.header_footers();
        let variants: Vec<_> = header_footers
            .iter()
            .map(|record| (record.kind, record.text.as_str()))
            .collect();
        assert_eq!(
            variants,
            vec![
                (HeaderFooterKind::EvenPageHeader, "EH"),
                (HeaderFooterKind::OddPageHeader, "OH"),
                (HeaderFooterKind::EvenPageFooter, "EF"),
                (HeaderFooterKind::OddPageFooter, "OF"),
                (HeaderFooterKind::FirstPageHeader, "FH"),
                (HeaderFooterKind::FirstPageFooter, "FF"),
            ]
        );
        let model = doc.model();
        assert_eq!(
            model.source_region_text(
                model
                    .source_regions(SourceRegionKind::HeaderFooter)
                    .next()
                    .unwrap()
            ),
            "EH"
        );

        let Block::Paragraph(default_header) = &model.setup.header[0] else {
            panic!("expected default legacy header paragraph");
        };
        let Block::Paragraph(even_header) = &model.setup.even_header[0] else {
            panic!("expected even legacy header paragraph");
        };
        let Block::Paragraph(first_header) = &model.setup.first_header[0] else {
            panic!("expected first-page legacy header paragraph");
        };
        let Block::Paragraph(default_footer) = &model.setup.footer[0] else {
            panic!("expected default legacy footer paragraph");
        };
        let Block::Paragraph(even_footer) = &model.setup.even_footer[0] else {
            panic!("expected even legacy footer paragraph");
        };
        let Block::Paragraph(first_footer) = &model.setup.first_footer[0] else {
            panic!("expected first-page legacy footer paragraph");
        };
        assert_eq!(default_header.text(), "OH");
        assert_eq!(even_header.text(), "EH");
        assert_eq!(first_header.text(), "FH");
        assert_eq!(default_footer.text(), "OF");
        assert_eq!(even_footer.text(), "EF");
        assert_eq!(first_footer.text(), "FF");
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_running_surfaces_roundtrip_absolute_line_spacing_to_docx() {
        const EXACT_FIVE_POINTS: u16 = 0xFF9C;
        let variants = ["EH", "OH", "EF", "OF", "FH", "FF"];

        for (story_position, marker) in variants.into_iter().enumerate() {
            let exact = Document::open(&legacy_running_surface_absolute_spacing_doc(
                story_position,
                (EXACT_FIVE_POINTS, 0),
            ))
            .unwrap();
            let minimum = Document::open(&legacy_running_surface_absolute_spacing_doc(
                story_position,
                (800, 0),
            ))
            .unwrap();
            let exact_model = exact.model();
            let minimum_model = minimum.model();

            assert_eq!(
                exact_model, minimum_model,
                "legacy {marker} absolute spacing must remain outside the public model"
            );

            let exact_converted = exact.to_docx();
            let minimum_converted = minimum.to_docx();
            assert_eq!(exact_converted, exact.to_docx());
            assert_eq!(minimum_converted, minimum.to_docx());
            assert_ne!(exact_converted, minimum_converted);
            assert!(
                !docx_part(&exact_converted, "word/document.xml").contains(r#"w:lineRule="exact""#)
            );
            assert!(!docx_part(&minimum_converted, "word/document.xml")
                .contains(r#"w:lineRule="atLeast""#));
            assert_single_running_line_rule(
                &exact_converted,
                &format!(">{marker}</w:t>"),
                r#"w:line="100" w:lineRule="exact""#,
            );
            assert_single_running_line_rule(
                &minimum_converted,
                &format!(">{marker}</w:t>"),
                r#"w:line="800" w:lineRule="atLeast""#,
            );

            let exact_reopened = Document::open(&exact_converted).unwrap();
            let minimum_reopened = Document::open(&minimum_converted).unwrap();
            let Backend::Docx(exact_state) = &exact_reopened.backend else {
                panic!("converted document must use the DOCX backend");
            };
            let Backend::Docx(minimum_state) = &minimum_reopened.backend else {
                panic!("converted document must use the DOCX backend");
            };
            assert_eq!(exact_state.running_line_spacing_hints.len(), 1);
            assert_eq!(minimum_state.running_line_spacing_hints.len(), 1);
            assert_eq!(
                legacy_running_spacing_at(
                    &exact_state.running_line_spacing_hints[0],
                    story_position,
                ),
                &[Some(crate::model::LineSpacingHint::Exact(5.0))]
            );
            assert_eq!(
                legacy_running_spacing_at(
                    &minimum_state.running_line_spacing_hints[0],
                    story_position,
                ),
                &[Some(crate::model::LineSpacingHint::AtLeast(40.0))]
            );

            assert!(docx_running_parts(&write_docx(&exact_model))
                .iter()
                .all(|(_, xml)| !xml.contains(r#"w:lineRule="exact""#)
                    && !xml.contains(r#"w:lineRule="atLeast""#)));
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_running_surfaces_roundtrip_pagination_to_docx() {
        let variants = ["EH", "OH", "EF", "OF", "FH", "FF"];

        for in_table in [false, true] {
            for (story_position, marker) in variants.into_iter().enumerate() {
                let paginated = Document::open(&legacy_running_pagination_doc(
                    story_position,
                    in_table,
                    true,
                ))
                .unwrap();
                let control = Document::open(&legacy_running_pagination_doc(
                    story_position,
                    in_table,
                    false,
                ))
                .unwrap();
                let model = paginated.model();
                assert_eq!(model, control.model());

                let converted = paginated.to_docx();
                let control_converted = control.to_docx();
                assert_eq!(converted, paginated.to_docx());
                assert_ne!(converted, control_converted);
                let document_xml = docx_part(&converted, "word/document.xml");
                assert!(!document_xml.contains("<w:keep"), "{document_xml}");

                let text = if in_table { "T" } else { marker };
                let parts = docx_running_parts(&converted);
                let selected = parts
                    .iter()
                    .filter(|(_, xml)| xml.contains(&format!(">{text}</w:t>")))
                    .collect::<Vec<_>>();
                assert_eq!(selected.len(), 1, "{story_position} {in_table}: {parts:?}");
                let paragraph = docx_paragraph_with_text(&selected[0].1, text);
                assert!(paragraph.contains("<w:keepNext/>"), "{paragraph}");
                assert!(paragraph.contains("<w:keepLines/>"), "{paragraph}");
                assert!(
                    paragraph.contains(r#"<w:widowControl w:val="0"/>"#),
                    "{paragraph}"
                );
                assert_eq!(
                    parts
                        .iter()
                        .map(|(_, xml)| xml.matches("<w:keepNext/>").count())
                        .sum::<usize>(),
                    1
                );
                assert_eq!(
                    parts
                        .iter()
                        .map(|(_, xml)| xml.matches("<w:keepLines/>").count())
                        .sum::<usize>(),
                    1
                );
                assert_eq!(
                    parts
                        .iter()
                        .map(|(_, xml)| xml.matches("<w:widowControl").count())
                        .sum::<usize>(),
                    1
                );
                assert_eq!(
                    parts
                        .iter()
                        .map(|(_, xml)| xml.matches("<w:cantSplit/>").count())
                        .sum::<usize>(),
                    usize::from(in_table)
                );

                assert_eq!(
                    Document::open(&converted).unwrap().model(),
                    Document::open(&control_converted).unwrap().model()
                );
                assert!(docx_running_parts(&write_docx(&model))
                    .iter()
                    .all(|(_, xml)| !xml.contains("<w:keepNext")
                        && !xml.contains("<w:keepLines")
                        && !xml.contains("<w:widowControl")
                        && !xml.contains("<w:cantSplit")));
            }
        }
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_running_surfaces_consume_absolute_line_spacing() {
        const EXACT_FIVE_POINTS: u16 = 0xFF9C;
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let variants = [
            "even header",
            "default header",
            "even footer",
            "default footer",
            "first header",
            "first footer",
        ];

        for (story_position, variant) in variants.into_iter().enumerate() {
            let exact = Document::open(&legacy_running_surface_absolute_spacing_doc(
                story_position,
                (EXACT_FIVE_POINTS, 0),
            ))
            .unwrap();
            let minimum = Document::open(&legacy_running_surface_absolute_spacing_doc(
                story_position,
                (800, 0),
            ))
            .unwrap();

            assert_eq!(
                exact.model(),
                minimum.model(),
                "legacy {variant} absolute spacing must remain outside the public model"
            );
            let Backend::Doc(exact_state) = &exact.backend else {
                panic!("synthetic legacy document must use the DOC backend");
            };
            let Backend::Doc(minimum_state) = &minimum.backend else {
                panic!("synthetic legacy document must use the DOC backend");
            };
            let exact_running =
                legacy_build_output_from_doc_state(exact_state).running_line_spacing_hints;
            let minimum_running =
                legacy_build_output_from_doc_state(minimum_state).running_line_spacing_hints;
            assert_eq!(exact_running.len(), 1);
            assert_eq!(minimum_running.len(), 1);
            assert_eq!(
                legacy_running_spacing_at(&exact_running[0], story_position),
                &[Some(crate::model::LineSpacingHint::Exact(5.0))]
            );
            assert_eq!(
                legacy_running_spacing_at(&minimum_running[0], story_position),
                &[Some(crate::model::LineSpacingHint::AtLeast(40.0))]
            );
            assert_eq!(exact.layout_pages_with_fonts(&fonts).unwrap().pages, 3);
            assert_eq!(minimum.layout_pages_with_fonts(&fonts).unwrap().pages, 3);
            let exact_pdf = render_opened_document_without_body_line_spacing(&exact, &fonts);
            let minimum_pdf = render_opened_document_without_body_line_spacing(&minimum, &fonts);
            assert_ne!(exact_pdf, minimum_pdf, "legacy {variant} hint was ignored");
            assert_eq!(
                exact_pdf,
                render_opened_document_without_body_line_spacing(&exact, &fonts)
            );
            assert_eq!(
                minimum_pdf,
                render_opened_document_without_body_line_spacing(&minimum, &fonts)
            );
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_running_table_cells_roundtrip_absolute_line_spacing_to_docx() {
        const EXACT_FIVE_POINTS: u16 = 0xFF9C;

        for story_position in 0..6 {
            let exact = Document::open(&legacy_running_table_absolute_spacing_doc(
                story_position,
                (EXACT_FIVE_POINTS, 0),
            ))
            .unwrap();
            let minimum = Document::open(&legacy_running_table_absolute_spacing_doc(
                story_position,
                (800, 0),
            ))
            .unwrap();
            let exact_model = exact.model();
            let minimum_model = minimum.model();

            assert_eq!(
                exact_model, minimum_model,
                "legacy running table {story_position} absolute spacing must remain outside the public model"
            );

            let exact_converted = exact.to_docx();
            let minimum_converted = minimum.to_docx();
            assert_eq!(exact_converted, exact.to_docx());
            assert_eq!(minimum_converted, minimum.to_docx());
            assert_ne!(exact_converted, minimum_converted);
            assert!(
                !docx_part(&exact_converted, "word/document.xml").contains(r#"w:lineRule="exact""#)
            );
            assert!(!docx_part(&minimum_converted, "word/document.xml")
                .contains(r#"w:lineRule="atLeast""#));
            assert_single_running_line_rule(
                &exact_converted,
                ">T</w:t>",
                r#"w:line="100" w:lineRule="exact""#,
            );
            assert_single_running_line_rule(
                &minimum_converted,
                ">T</w:t>",
                r#"w:line="800" w:lineRule="atLeast""#,
            );

            let exact_reopened = Document::open(&exact_converted).unwrap();
            let minimum_reopened = Document::open(&minimum_converted).unwrap();
            let Backend::Docx(exact_state) = &exact_reopened.backend else {
                panic!("converted document must use the DOCX backend");
            };
            let Backend::Docx(minimum_state) = &minimum_reopened.backend else {
                panic!("converted document must use the DOCX backend");
            };
            assert_eq!(exact_state.running_line_spacing_hints.len(), 1);
            assert_eq!(minimum_state.running_line_spacing_hints.len(), 1);
            assert_eq!(
                legacy_running_table_spacing_at(
                    &exact_state.running_line_spacing_hints[0],
                    story_position,
                ),
                &[vec![vec![vec![Some(
                    crate::model::LineSpacingHint::Exact(5.0)
                )]]]]
            );
            assert_eq!(
                legacy_running_table_spacing_at(
                    &minimum_state.running_line_spacing_hints[0],
                    story_position,
                ),
                &[vec![vec![vec![Some(
                    crate::model::LineSpacingHint::AtLeast(40.0)
                )]]]]
            );

            assert!(docx_running_parts(&write_docx(&exact_model))
                .iter()
                .all(|(_, xml)| !xml.contains(r#"w:lineRule="exact""#)
                    && !xml.contains(r#"w:lineRule="atLeast""#)));
        }
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_running_table_cells_consume_absolute_line_spacing() {
        const EXACT_FIVE_POINTS: u16 = 0xFF9C;
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let variants = [
            "even header table",
            "default header table",
            "even footer table",
            "default footer table",
            "first header table",
            "first footer table",
        ];

        for (story_position, variant) in variants.into_iter().enumerate() {
            let exact = Document::open(&legacy_running_table_absolute_spacing_doc(
                story_position,
                (EXACT_FIVE_POINTS, 0),
            ))
            .unwrap();
            let minimum = Document::open(&legacy_running_table_absolute_spacing_doc(
                story_position,
                (800, 0),
            ))
            .unwrap();

            assert_eq!(
                exact.model(),
                minimum.model(),
                "legacy {variant} absolute spacing must remain outside the public model"
            );
            let Backend::Doc(exact_state) = &exact.backend else {
                panic!("synthetic legacy document must use the DOC backend");
            };
            let Backend::Doc(minimum_state) = &minimum.backend else {
                panic!("synthetic legacy document must use the DOC backend");
            };
            let exact_running =
                legacy_build_output_from_doc_state(exact_state).running_line_spacing_hints;
            let minimum_running =
                legacy_build_output_from_doc_state(minimum_state).running_line_spacing_hints;
            assert_eq!(exact_running.len(), 1);
            assert_eq!(minimum_running.len(), 1);
            assert_eq!(
                legacy_running_table_spacing_at(&exact_running[0], story_position),
                &[vec![vec![vec![Some(
                    crate::model::LineSpacingHint::Exact(5.0)
                )]]]]
            );
            assert_eq!(
                legacy_running_table_spacing_at(&minimum_running[0], story_position),
                &[vec![vec![vec![Some(
                    crate::model::LineSpacingHint::AtLeast(40.0)
                )]]]]
            );
            assert_eq!(exact.layout_pages_with_fonts(&fonts).unwrap().pages, 3);
            assert_eq!(minimum.layout_pages_with_fonts(&fonts).unwrap().pages, 3);
            let exact_pdf = render_opened_document_without_body_line_spacing(&exact, &fonts);
            let minimum_pdf = render_opened_document_without_body_line_spacing(&minimum, &fonts);
            assert_ne!(exact_pdf, minimum_pdf, "legacy {variant} hint was ignored");
            assert_eq!(
                exact_pdf,
                render_opened_document_without_body_line_spacing(&exact, &fonts)
            );
            assert_eq!(
                minimum_pdf,
                render_opened_document_without_body_line_spacing(&minimum, &fonts)
            );
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_running_surfaces_roundtrip_custom_tabs_to_docx() {
        let variants = ["EH", "OH", "EF", "OF", "FH", "FF"];
        let expected_xml = concat!(
            r#"<w:tabs><w:tab w:val="decimal" w:pos="1080" "#,
            r#"w:leader="underscore"/></w:tabs>"#,
        );
        let expected_stop = crate::model::TabStop {
            position_pt: 54.0,
            alignment: crate::model::TabAlignment::Decimal,
            leader: crate::model::TabLeader::Underscore,
        };

        for (story_position, marker) in variants.into_iter().enumerate() {
            let tabbed =
                Document::open(&legacy_running_tabs_doc(story_position, false, true)).unwrap();
            let control =
                Document::open(&legacy_running_tabs_doc(story_position, false, false)).unwrap();
            let model = tabbed.model();
            assert_eq!(model, control.model());

            let converted = tabbed.to_docx();
            assert_eq!(converted, tabbed.to_docx());
            assert_ne!(converted, control.to_docx());
            assert!(!docx_part(&converted, "word/document.xml").contains("<w:tabs>"));
            assert_single_running_tabs(&converted, &format!(">{marker}</w:t>"), expected_xml);

            let reopened = Document::open(&converted).unwrap();
            let Backend::Docx(state) = &reopened.backend else {
                panic!("converted document must use the DOCX backend");
            };
            assert_eq!(state.running_tab_stops.len(), 1);
            assert_eq!(
                legacy_running_tabs_at(&state.running_tab_stops[0], story_position),
                &[vec![expected_stop]]
            );
            assert!(docx_running_parts(&write_docx(&model))
                .iter()
                .all(|(_, xml)| !xml.contains("<w:tabs>")));
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_running_table_cells_roundtrip_custom_tabs_to_docx() {
        let expected_xml = r#"<w:tabs><w:tab w:val="right" w:pos="1200" w:leader="dot"/></w:tabs>"#;
        let expected_stop = crate::model::TabStop {
            position_pt: 60.0,
            alignment: crate::model::TabAlignment::Right,
            leader: crate::model::TabLeader::Dot,
        };

        for story_position in 0..6 {
            let tabbed =
                Document::open(&legacy_running_tabs_doc(story_position, true, true)).unwrap();
            let control =
                Document::open(&legacy_running_tabs_doc(story_position, true, false)).unwrap();
            let model = tabbed.model();
            assert_eq!(model, control.model());

            let converted = tabbed.to_docx();
            assert_eq!(converted, tabbed.to_docx());
            assert_ne!(converted, control.to_docx());
            assert!(!docx_part(&converted, "word/document.xml").contains("<w:tabs>"));
            assert_single_running_tabs(&converted, ">T</w:t>", expected_xml);

            let reopened = Document::open(&converted).unwrap();
            let Backend::Docx(state) = &reopened.backend else {
                panic!("converted document must use the DOCX backend");
            };
            assert_eq!(state.running_table_cell_tab_stops.len(), 1);
            assert_eq!(
                legacy_running_table_tabs_at(
                    &state.running_table_cell_tab_stops[0],
                    story_position,
                ),
                &[vec![vec![vec![vec![expected_stop]]]]]
            );
            assert!(docx_running_parts(&write_docx(&model))
                .iter()
                .all(|(_, xml)| !xml.contains("<w:tabs>")));
        }
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_running_tabs_change_preview_deterministically() {
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        for in_table in [false, true] {
            for story_position in 0..6 {
                let tabbed =
                    Document::open(&legacy_running_tabs_doc(story_position, in_table, true))
                        .unwrap();
                let control =
                    Document::open(&legacy_running_tabs_doc(story_position, in_table, false))
                        .unwrap();
                assert_eq!(tabbed.model(), control.model());
                let Backend::Doc(state) = &tabbed.backend else {
                    panic!("synthetic legacy document must use the DOC backend");
                };
                let assembled = legacy_build_output_from_doc_state(state);
                assert!(assembled.tab_stops.iter().all(Vec::is_empty));
                assert!(assembled.table_cell_tab_stops.iter().all(Vec::is_empty));
                if in_table {
                    assert_eq!(
                        legacy_running_table_tabs_at(
                            &assembled.running_table_cell_tab_stops[0],
                            story_position,
                        ),
                        &[vec![vec![vec![vec![crate::model::TabStop {
                            position_pt: 60.0,
                            alignment: crate::model::TabAlignment::Right,
                            leader: crate::model::TabLeader::Dot,
                        }]]]]]
                    );
                } else {
                    assert_eq!(
                        legacy_running_tabs_at(&assembled.running_tab_stops[0], story_position),
                        &[vec![crate::model::TabStop {
                            position_pt: 54.0,
                            alignment: crate::model::TabAlignment::Decimal,
                            leader: crate::model::TabLeader::Underscore,
                        }]]
                    );
                }

                let rendered = tabbed.to_pdf_with_fonts(&fonts);
                let control_rendered = control.to_pdf_with_fonts(&fonts);
                assert!(rendered.starts_with(b"%PDF-"));
                assert_ne!(
                    rendered, control_rendered,
                    "legacy running tabs were ignored for story {story_position}, table={in_table}"
                );
                assert_eq!(rendered, tabbed.to_pdf_with_fonts(&fonts));
            }
        }
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_note_paragraph_tabs_change_preview_without_conversion() {
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        for kind in [SourceRegionKind::Footnote, SourceRegionKind::Endnote] {
            let tabbed = Document::open(&legacy_note_tabs_doc(kind, false, true)).unwrap();
            let control = Document::open(&legacy_note_tabs_doc(kind, false, false)).unwrap();
            assert_eq!(tabbed.model(), control.model());
            assert_legacy_note_tab_overlay(&tabbed, kind, false);
            #[cfg(feature = "docx")]
            assert_eq!(tabbed.to_docx(), control.to_docx());

            let rendered = tabbed.to_pdf_with_fonts(&fonts);
            let control_rendered = control.to_pdf_with_fonts(&fonts);
            assert!(rendered.starts_with(b"%PDF-"));
            assert_ne!(
                rendered, control_rendered,
                "legacy {kind:?} paragraph tabs were ignored"
            );
            assert_eq!(rendered, tabbed.to_pdf_with_fonts(&fonts));
        }
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_note_table_tabs_change_preview_without_conversion() {
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        for kind in [SourceRegionKind::Footnote, SourceRegionKind::Endnote] {
            let tabbed = Document::open(&legacy_note_tabs_doc(kind, true, true)).unwrap();
            let control = Document::open(&legacy_note_tabs_doc(kind, true, false)).unwrap();
            assert_eq!(tabbed.model(), control.model());
            assert_legacy_note_tab_overlay(&tabbed, kind, true);
            #[cfg(feature = "docx")]
            assert_eq!(tabbed.to_docx(), control.to_docx());

            let rendered = tabbed.to_pdf_with_fonts(&fonts);
            let control_rendered = control.to_pdf_with_fonts(&fonts);
            assert!(rendered.starts_with(b"%PDF-"));
            assert_ne!(
                rendered, control_rendered,
                "legacy {kind:?} table-cell tabs were ignored"
            );
            assert_eq!(rendered, tabbed.to_pdf_with_fonts(&fonts));
        }
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_running_surface_distances_change_preview_only() {
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let baseline = Document::open(&legacy_running_surface_distance_doc(None, None)).unwrap();
        let header =
            Document::open(&legacy_running_surface_distance_doc(Some(1_000), None)).unwrap();
        let footer =
            Document::open(&legacy_running_surface_distance_doc(None, Some(1_000))).unwrap();
        let both = Document::open(&legacy_running_surface_distance_doc(
            Some(1_000),
            Some(1_000),
        ))
        .unwrap();
        let invalid =
            Document::open(&legacy_running_surface_distance_doc(Some(u16::MAX), None)).unwrap();

        for document in [&header, &footer, &both, &invalid] {
            assert_eq!(
                baseline.model(),
                document.model(),
                "legacy running distances must remain outside the public model"
            );
            assert_eq!(
                baseline.layout_pages_with_fonts(&fonts).unwrap().pages,
                document.layout_pages_with_fonts(&fonts).unwrap().pages,
                "legacy running distances must not repaginate the body"
            );
        }

        let baseline_pdf = baseline.to_pdf_with_fonts(&fonts);
        let header_pdf = header.to_pdf_with_fonts(&fonts);
        let footer_pdf = footer.to_pdf_with_fonts(&fonts);
        let both_pdf = both.to_pdf_with_fonts(&fonts);
        assert_ne!(
            header_pdf, baseline_pdf,
            "legacy header distance was ignored"
        );
        assert_ne!(
            footer_pdf, baseline_pdf,
            "legacy footer distance was ignored"
        );
        assert_ne!(both_pdf, header_pdf);
        assert_ne!(both_pdf, footer_pdf);
        assert_eq!(invalid.to_pdf_with_fonts(&fonts), baseline_pdf);
        assert_eq!(header_pdf, header.to_pdf_with_fonts(&fonts));
        assert_eq!(footer_pdf, footer.to_pdf_with_fonts(&fonts));
        assert_eq!(both_pdf, both.to_pdf_with_fonts(&fonts));
    }

    #[test]
    fn legacy_doc_plcfhdd_disambiguates_header_footer_sections() {
        let plcf_hdd = [
            0, 0, 0, 0, 0, 0, 0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 24,
        ];
        let bytes = synth_doc_with_ccp_and_plcfhdd(
            "BODYEHOHEFOFFHFFehohefoffhff",
            "",
            0x00C1,
            0,
            0,
            [4, 0, 24, 0, 0, 0],
            Some(&plcf_hdd),
        );
        let doc = Document::open(&bytes).unwrap();

        let records: Vec<_> = doc
            .header_footers()
            .iter()
            .map(|record| (record.kind, record.text.clone(), record.section))
            .collect();
        assert_eq!(
            records,
            vec![
                (HeaderFooterKind::EvenPageHeader, "EH".to_string(), Some(0)),
                (HeaderFooterKind::OddPageHeader, "OH".to_string(), Some(0)),
                (HeaderFooterKind::EvenPageFooter, "EF".to_string(), Some(0)),
                (HeaderFooterKind::OddPageFooter, "OF".to_string(), Some(0)),
                (HeaderFooterKind::FirstPageHeader, "FH".to_string(), Some(0)),
                (HeaderFooterKind::FirstPageFooter, "FF".to_string(), Some(0)),
                (HeaderFooterKind::EvenPageHeader, "eh".to_string(), Some(1)),
                (HeaderFooterKind::OddPageHeader, "oh".to_string(), Some(1)),
                (HeaderFooterKind::EvenPageFooter, "ef".to_string(), Some(1)),
                (HeaderFooterKind::OddPageFooter, "of".to_string(), Some(1)),
                (HeaderFooterKind::FirstPageHeader, "fh".to_string(), Some(1)),
                (HeaderFooterKind::FirstPageFooter, "ff".to_string(), Some(1)),
            ]
        );
    }

    #[test]
    fn legacy_doc_plcfsed_applies_plcfhdd_groups_to_section_setups() {
        let bytes = two_section_legacy_header_footer_doc(None);
        let doc = Document::open(&bytes).unwrap();

        let records: Vec<_> = doc
            .header_footers()
            .iter()
            .map(|record| (record.kind, record.text.clone(), record.section))
            .collect();
        assert!(records.contains(&(HeaderFooterKind::OddPageHeader, "O0".to_string(), Some(0))));
        assert!(records.contains(&(HeaderFooterKind::OddPageHeader, "O1".to_string(), Some(1))));

        let model = doc.model();
        let sections: Vec<_> = model
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::SectionBreak(setup) => Some(setup),
                _ => None,
            })
            .collect();
        assert_eq!(sections.len(), 1);
        let first_section = sections[0];
        assert_eq!(
            first_section.section_break,
            Some(SectionBreakKind::NextPage)
        );
        assert_eq!(single_paragraph_text(&first_section.header), "O0");
        assert_eq!(single_paragraph_text(&first_section.even_header), "E0");
        assert_eq!(single_paragraph_text(&first_section.footer), "o0");
        assert_eq!(single_paragraph_text(&first_section.even_footer), "e0");
        assert_eq!(single_paragraph_text(&first_section.first_header), "F0");
        assert_eq!(single_paragraph_text(&first_section.first_footer), "f0");

        assert_eq!(single_paragraph_text(&model.setup.header), "O1");
        assert_eq!(single_paragraph_text(&model.setup.even_header), "E1");
        assert_eq!(single_paragraph_text(&model.setup.footer), "o1");
        assert_eq!(single_paragraph_text(&model.setup.even_footer), "e1");
        assert_eq!(single_paragraph_text(&model.setup.first_header), "F1");
        assert_eq!(single_paragraph_text(&model.setup.first_footer), "f1");
    }

    #[test]
    fn legacy_doc_sepx_preserves_section_page_geometry() {
        let section_cps = [0, 5, 10];
        let first = section_page_grpprl(12_240, 15_840, 1_440, 1_800, 720, 900, false);
        let second = section_page_grpprl(15_840, 12_240, 2_160, 1_080, 1_440, 720, true);
        let sepx_grpprls = [first.as_slice(), second.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("FIRSTFINAL", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();
        let first_page = model
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::SectionBreak(setup) => Some(setup.page),
                _ => None,
            })
            .expect("SEPX section boundary");

        assert_eq!(first_page.width_pt, 612.0);
        assert_eq!(first_page.height_pt, 792.0);
        assert_eq!(first_page.margin_left_pt, Some(72.0));
        assert_eq!(first_page.margin_right_pt, Some(90.0));
        assert_eq!(first_page.margin_top_pt, Some(36.0));
        assert_eq!(first_page.margin_bottom_pt, Some(45.0));
        assert!(!first_page.landscape);

        let final_page = model.setup.page;
        assert_eq!(final_page.width_pt, 792.0);
        assert_eq!(final_page.height_pt, 612.0);
        assert_eq!(final_page.margin_left_pt, Some(108.0));
        assert_eq!(final_page.margin_right_pt, Some(54.0));
        assert_eq!(final_page.margin_top_pt, Some(72.0));
        assert_eq!(final_page.margin_bottom_pt, Some(36.0));
        assert!(final_page.landscape);
    }

    #[cfg(feature = "render")]
    #[test]
    fn legacy_doc_sepx_aligns_running_surface_distances_and_isolates_malformed_sections() {
        let section_cps = [0, 5, 10, 15];
        let mut first = Vec::new();
        first.extend_from_slice(&0xB017u16.to_le_bytes());
        first.extend_from_slice(&0u16.to_le_bytes());
        first.extend_from_slice(&0xB018u16.to_le_bytes());
        first.extend_from_slice(&31_680u16.to_le_bytes());
        let malformed = [0x17];
        let mut final_section = Vec::new();
        final_section.extend_from_slice(&0xB017u16.to_le_bytes());
        final_section.extend_from_slice(&2_000u16.to_le_bytes());
        final_section.extend_from_slice(&0xB018u16.to_le_bytes());
        final_section.extend_from_slice(&3_000u16.to_le_bytes());
        let sepx_grpprls = [
            first.as_slice(),
            malformed.as_slice(),
            final_section.as_slice(),
        ];
        let bytes =
            legacy_doc_with_section_page_grpprls("AAAAABBBBBCCCCC", &section_cps, &sepx_grpprls);

        let document = Document::open(&bytes).unwrap();
        let Backend::Doc(state) = &document.backend else {
            panic!("synthetic legacy document must use the DOC backend");
        };
        let hints = legacy_build_output_from_doc_state(state).running_surface_distances;

        assert_eq!(
            hints,
            vec![
                crate::model::RunningSurfaceDistanceHints {
                    header_pt: Some(0.0),
                    footer_pt: Some(1_584.0),
                },
                crate::model::RunningSurfaceDistanceHints::default(),
                crate::model::RunningSurfaceDistanceHints {
                    header_pt: Some(100.0),
                    footer_pt: Some(150.0),
                },
            ]
        );
    }

    #[test]
    fn legacy_doc_sepx_preserves_title_page_state_in_source_order() {
        let section_cps = [0, 5, 10, 15];
        let mut first = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        let mut second = first.clone();
        let mut final_section = first.clone();
        push_section_title_page(&mut first, 1);
        push_section_title_page(&mut first, 2);
        push_section_title_page(&mut second, 1);
        push_section_title_page(&mut second, 0);
        push_section_title_page(&mut final_section, 1);
        let sepx_grpprls = [
            first.as_slice(),
            second.as_slice(),
            final_section.as_slice(),
        ];
        let bytes =
            legacy_doc_with_section_page_grpprls("AAAAABBBBBCCCCC", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();
        let section_title_pages = model
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::SectionBreak(setup) => Some(setup.title_page),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(section_title_pages, vec![true, false]);
        assert!(model.setup.title_page);

        #[cfg(feature = "docx")]
        {
            let docx = doc.to_docx();
            let document_xml = docx_part(&docx, "word/document.xml");
            assert_eq!(document_xml.matches("<w:titlePg/>").count(), 2);

            let reopened = Document::open(&docx).unwrap().model();
            let reopened_title_pages = reopened
                .blocks
                .iter()
                .filter_map(|block| match block {
                    Block::SectionBreak(setup) => Some(setup.title_page),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(reopened_title_pages, section_title_pages);
            assert!(reopened.setup.title_page);
        }
    }

    #[test]
    fn legacy_doc_sepx_preserves_single_section_title_page_state() {
        let section_cps = [0, 4];
        let mut only = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_title_page(&mut only, 1);
        let sepx_grpprls = [only.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("ONLY", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();

        assert!(model
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::SectionBreak(_))));
        assert!(model.setup.title_page);

        #[cfg(feature = "docx")]
        {
            let docx = doc.to_docx();
            assert_eq!(
                docx_part(&docx, "word/document.xml")
                    .matches("<w:titlePg/>")
                    .count(),
                1
            );
            assert!(Document::open(&docx).unwrap().model().setup.title_page);
        }
    }

    #[test]
    fn legacy_doc_sepx_preserves_page_number_state_in_source_order() {
        let section_cps = [0, 5, 10, 15];
        let mut first = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        let mut second = first.clone();
        let mut final_section = first.clone();

        push_section_page_number_format(&mut first, 0x01);
        push_section_page_number_format(&mut first, 0x3C);
        push_section_page_number_restart(&mut first, 1);
        push_section_page_number_start97(&mut first, 4);
        push_section_page_number_start(&mut first, 70_000);
        push_section_page_number_start(&mut first, u32::MAX);
        push_section_page_number_restart(&mut first, 2);

        push_section_page_number_format(&mut second, 0x39);
        push_section_page_number_restart(&mut second, 1);
        push_section_page_number_start97(&mut second, 9);
        push_section_page_number_restart(&mut second, 0);

        push_section_page_number_format(&mut final_section, 0x17);
        push_section_page_number_restart(&mut final_section, 1);

        let sepx_grpprls = [
            first.as_slice(),
            second.as_slice(),
            final_section.as_slice(),
        ];
        let bytes =
            legacy_doc_with_section_page_grpprls("AAAAABBBBBCCCCC", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();
        let section_page_numbers = model
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::SectionBreak(setup) => {
                    Some((setup.page_number_start, setup.page_number_format))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            section_page_numbers,
            vec![
                (Some(70_000), Some(PageNumberFormat::UpperRoman)),
                (None, Some(PageNumberFormat::NumberInDash)),
            ]
        );
        assert_eq!(model.setup.page_number_start, Some(1));
        assert_eq!(
            model.setup.page_number_format,
            Some(PageNumberFormat::Decimal)
        );

        #[cfg(feature = "docx")]
        {
            let docx = doc.to_docx();
            let document_xml = docx_part(&docx, "word/document.xml");
            assert!(document_xml.contains(r#"<w:pgNumType w:start="70000" w:fmt="upperRoman"/>"#));
            assert!(document_xml.contains(r#"<w:pgNumType w:fmt="numberInDash"/>"#));
            assert!(document_xml.contains(r#"<w:pgNumType w:start="1" w:fmt="decimal"/>"#));

            let reopened = Document::open(&docx).unwrap();
            let reopened_model = reopened.model();
            let reopened_page_numbers = reopened_model
                .blocks
                .iter()
                .filter_map(|block| match block {
                    Block::SectionBreak(setup) => {
                        Some((setup.page_number_start, setup.page_number_format))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(reopened_page_numbers, section_page_numbers);
            assert_eq!(
                reopened_model.setup.page_number_start,
                model.setup.page_number_start
            );
            assert_eq!(
                reopened_model.setup.page_number_format,
                model.setup.page_number_format
            );
        }
    }

    #[test]
    fn legacy_doc_sepx_preserves_single_section_page_number_state() {
        let section_cps = [0, 4];
        let mut only = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_page_number_format(&mut only, 0x16);
        push_section_page_number_restart(&mut only, 1);
        push_section_page_number_start97(&mut only, 0);
        let sepx_grpprls = [only.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("ONLY", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();

        assert!(model
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::SectionBreak(_))));
        assert_eq!(model.setup.page_number_start, Some(1));
        assert_eq!(
            model.setup.page_number_format,
            Some(PageNumberFormat::DecimalZero)
        );

        #[cfg(feature = "docx")]
        {
            let docx = doc.to_docx();
            assert!(docx_part(&docx, "word/document.xml")
                .contains(r#"<w:pgNumType w:start="1" w:fmt="decimalZero"/>"#));
            let reopened = Document::open(&docx).unwrap();
            assert_eq!(reopened.model().setup.page_number_start, Some(1));
            assert_eq!(
                reopened.model().setup.page_number_format,
                Some(PageNumberFormat::DecimalZero)
            );
        }
    }

    #[test]
    fn legacy_doc_sepx_preserves_document_grid_state_in_source_order() {
        let section_cps = [0, 5, 10, 15];
        let mut first = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        let mut disabled = first.clone();
        let mut final_section = first.clone();

        push_section_document_grid_mode(&mut first, 1);
        push_section_document_grid_line_pitch(&mut first, 360);
        push_section_document_grid_character_space(&mut first, 40_960);

        push_section_document_grid_mode(&mut disabled, 1);
        push_section_document_grid_line_pitch(&mut disabled, 480);
        push_section_document_grid_mode(&mut disabled, 0);

        push_section_document_grid_mode(&mut final_section, 3);
        push_section_document_grid_line_pitch(&mut final_section, 720);
        push_section_document_grid_character_space(&mut final_section, 20_480);
        push_section_document_grid_character_space(&mut final_section, -4_096);

        let sepx_grpprls = [
            first.as_slice(),
            disabled.as_slice(),
            final_section.as_slice(),
        ];
        let bytes =
            legacy_doc_with_section_page_grpprls("AAAAABBBBBCCCCC", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();
        let section_grids = model
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::SectionBreak(setup) => Some(setup.doc_grid),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            section_grids,
            vec![
                Some(DocGrid {
                    grid_type: DocGridType::LinesAndChars,
                    line_pitch: Some(360),
                    character_space: Some(40_960),
                }),
                None,
            ]
        );
        assert_eq!(
            model.setup.doc_grid,
            Some(DocGrid {
                grid_type: DocGridType::SnapToChars,
                line_pitch: Some(720),
                character_space: None,
            })
        );

        #[cfg(feature = "docx")]
        {
            let docx = doc.to_docx();
            let document_xml = docx_part(&docx, "word/document.xml");
            assert_eq!(document_xml.matches("<w:docGrid").count(), 2);
            assert!(document_xml.contains(
                r#"<w:docGrid w:type="linesAndChars" w:linePitch="360" w:charSpace="40960"/>"#
            ));
            assert!(document_xml.contains(r#"<w:docGrid w:type="snapToChars" w:linePitch="720"/>"#));

            let reopened = Document::open(&docx).unwrap();
            let reopened_model = reopened.model();
            let reopened_section_grids = reopened_model
                .blocks
                .iter()
                .filter_map(|block| match block {
                    Block::SectionBreak(setup) => Some(setup.doc_grid),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(reopened_section_grids, section_grids);
            assert_eq!(reopened_model.setup.doc_grid, model.setup.doc_grid);
        }
    }

    #[test]
    fn legacy_doc_sepx_preserves_single_section_line_grid() {
        let section_cps = [0, 4];
        let mut only = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_document_grid_mode(&mut only, 2);
        push_section_document_grid_line_pitch(&mut only, 480);
        let sepx_grpprls = [only.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("ONLY", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();
        let expected = Some(DocGrid {
            grid_type: DocGridType::Lines,
            line_pitch: Some(480),
            character_space: None,
        });

        assert!(model
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::SectionBreak(_))));
        assert_eq!(model.setup.doc_grid, expected);

        #[cfg(feature = "docx")]
        {
            let docx = doc.to_docx();
            assert!(docx_part(&docx, "word/document.xml")
                .contains(r#"<w:docGrid w:type="lines" w:linePitch="480"/>"#));
            assert_eq!(
                Document::open(&docx).unwrap().model().setup.doc_grid,
                expected
            );
        }
    }

    #[test]
    fn legacy_doc_sepx_preserves_all_section_text_directions() {
        let section_cps = [0, 5, 10, 15, 20, 25, 30];
        let expected = [
            TextDirection::LeftToRightTopToBottom,
            TextDirection::TopToBottomRightToLeft,
            TextDirection::BottomToTopLeftToRight,
            TextDirection::TopToBottomRightToLeftVertical,
            TextDirection::LeftToRightTopToBottomVertical,
            TextDirection::TopToBottomLeftToRightVertical,
        ];
        let mut sections = Vec::new();
        for text_flow in 0..=5 {
            let mut section =
                section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
            push_section_text_direction(&mut section, text_flow);
            sections.push(section);
        }
        let sepx_grpprls = sections.iter().map(Vec::as_slice).collect::<Vec<_>>();
        let bytes = legacy_doc_with_section_page_grpprls(
            "AAAAABBBBBCCCCCDDDDDEEEEEFFFFF",
            &section_cps,
            &sepx_grpprls,
        );

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();
        let section_directions = model
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::SectionBreak(setup) => Some(setup.text_direction),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            section_directions,
            expected[..5].iter().copied().map(Some).collect::<Vec<_>>()
        );
        assert_eq!(model.setup.text_direction, Some(expected[5]));

        #[cfg(feature = "docx")]
        {
            let docx = doc.to_docx();
            let document_xml = docx_part(&docx, "word/document.xml");
            for value in ["lrTb", "tbRl", "btLr", "tbRlV", "lrTbV", "tbLrV"] {
                assert!(
                    document_xml.contains(&format!(r#"<w:textDirection w:val="{value}"/>"#)),
                    "missing text direction {value}: {document_xml}"
                );
            }

            let reopened = Document::open(&docx).unwrap().model();
            let reopened_directions = reopened
                .blocks
                .iter()
                .filter_map(|block| match block {
                    Block::SectionBreak(setup) => Some(setup.text_direction),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(reopened_directions, section_directions);
            assert_eq!(reopened.setup.text_direction, model.setup.text_direction);
        }
    }

    #[test]
    fn legacy_doc_sepx_preserves_single_section_text_direction() {
        let section_cps = [0, 4];
        let mut only = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_text_direction(&mut only, 3);
        push_section_text_direction(&mut only, 7);
        let sepx_grpprls = [only.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("ONLY", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();
        let expected = Some(TextDirection::TopToBottomRightToLeftVertical);

        assert!(model
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::SectionBreak(_))));
        assert_eq!(model.setup.text_direction, expected);

        #[cfg(feature = "docx")]
        {
            let docx = doc.to_docx();
            assert!(docx_part(&docx, "word/document.xml")
                .contains(r#"<w:textDirection w:val="tbRlV"/>"#));
            assert_eq!(
                Document::open(&docx).unwrap().model().setup.text_direction,
                expected
            );
        }
    }

    #[test]
    fn legacy_doc_sepx_preserves_equal_width_section_columns() {
        let section_cps = [0, 5, 10, 15];
        let mut first = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        let mut unequal = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        let mut final_section =
            section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut first, 1);
        push_section_column_count(&mut unequal, 2);
        push_section_evenly_spaced(&mut unequal, 0);
        push_section_column_count(&mut final_section, 43);
        let sepx_grpprls = [
            first.as_slice(),
            unequal.as_slice(),
            final_section.as_slice(),
        ];
        let bytes =
            legacy_doc_with_section_page_grpprls("AAAAABBBBBCCCCC", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();
        let section_columns = model
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::SectionBreak(setup) => Some(setup.columns),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(section_columns, vec![Some(2), None]);
        assert_eq!(model.setup.columns, Some(44));

        #[cfg(feature = "docx")]
        {
            let docx = doc.to_docx();
            let document_xml = docx_part(&docx, "word/document.xml");
            assert_eq!(document_xml.matches("<w:cols").count(), 2);
            let first_columns = document_xml
                .find(r#"<w:cols w:num="2"/>"#)
                .expect("first section columns");
            let final_columns = document_xml
                .find(r#"<w:cols w:num="44"/>"#)
                .expect("final section columns");
            assert!(
                first_columns < final_columns,
                "column counts must preserve section order: {document_xml}"
            );

            let reopened = Document::open(&docx).unwrap();
            let reopened_model = reopened.model();
            let reopened_section_columns = reopened_model
                .blocks
                .iter()
                .filter_map(|block| match block {
                    Block::SectionBreak(setup) => Some(setup.columns),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(reopened_section_columns, section_columns);
            assert_eq!(reopened_model.setup.columns, Some(44));
        }
    }

    #[test]
    fn legacy_doc_sepx_preserves_complete_unequal_section_column_counts() {
        let section_cps = [0, 5, 10];
        let mut first = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut first, 1);
        push_section_column_width(&mut first, 0, 2_000);
        push_section_column_custom_spacing(&mut first, 0, 400);
        push_section_column_width(&mut first, 1, 4_000);
        push_section_evenly_spaced(&mut first, 0);

        let mut final_section =
            section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut final_section, 2);
        for (index, width) in [(0, 1_500), (1, 2_500), (2, 3_500)] {
            push_section_column_width(&mut final_section, index, width);
        }
        push_section_evenly_spaced(&mut final_section, 0);

        let bytes = legacy_doc_with_section_page_grpprls(
            "FIRSTFINAL",
            &section_cps,
            &[first.as_slice(), final_section.as_slice()],
        );
        let model = Document::open(&bytes).unwrap().model();
        let first_columns = model
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::SectionBreak(setup) => Some(setup.columns),
                _ => None,
            })
            .expect("section boundary");

        assert_eq!(first_columns, Some(2));
        assert_eq!(model.setup.columns, Some(3));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_to_docx_preserves_private_unequal_column_semantics() {
        let text = "CUSTOM";
        let section_cps = [0, text.encode_utf16().count() as u32];
        let mut section = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut section, 1);
        push_section_column_width(&mut section, 0, 2_000);
        push_section_column_custom_spacing(&mut section, 0, 400);
        push_section_column_width(&mut section, 1, 4_000);
        push_section_evenly_spaced(&mut section, 0);
        push_section_column_separator(&mut section, 1);
        push_section_column_rtl(&mut section, 1);
        let document = Document::open(&legacy_doc_with_section_page_grpprls(
            text,
            &section_cps,
            &[section.as_slice()],
        ))
        .unwrap();

        let converted = document.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");

        assert!(
            document_xml.contains(
                r#"<w:cols w:num="2" w:equalWidth="0" w:sep="1"><w:col w:w="2000" w:space="400"/><w:col w:w="4000"/></w:cols>"#
            ),
            "custom legacy column geometry was not preserved: {document_xml}"
        );
        assert!(
            document_xml.contains("<w:bidi/>"),
            "legacy section bidi was not preserved: {document_xml}"
        );
        assert_eq!(converted, document.to_docx());

        let reopened = Document::open(&converted).unwrap();
        assert_eq!(reopened.model().setup.columns, Some(2));
        #[cfg(feature = "render")]
        reopened.with_render_model_and_hints(|_, hints| {
            let layout = hints
                .final_section_column_layout
                .expect("converted custom column geometry");
            assert_eq!(layout.columns[0].width_pt, 100.0);
            assert_eq!(layout.columns[0].space_after_pt, 20.0);
            assert_eq!(layout.columns[1].width_pt, 200.0);
            assert!(hints.final_section_column_separator);
            assert!(hints.final_section_column_rtl);
        });

        let model_only = write_docx(&document.model());
        let model_only_xml = docx_part(&model_only, "word/document.xml");
        assert!(model_only_xml.contains(r#"<w:cols w:num="2"/>"#));
        assert!(!model_only_xml.contains("w:equalWidth"));
        assert!(!model_only_xml.contains("<w:bidi/>"));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_to_docx_preserves_equal_column_hints_by_section() {
        let section_cps = [0, 5, 10];
        let mut first = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut first, 1);
        push_section_column_spacing(&mut first, 720);
        push_section_column_separator(&mut first, 1);
        push_section_column_rtl(&mut first, 1);

        let mut final_section =
            section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut final_section, 2);
        push_section_column_spacing(&mut final_section, 0);

        let document = Document::open(&legacy_doc_with_section_page_grpprls(
            "FIRSTFINAL",
            &section_cps,
            &[first.as_slice(), final_section.as_slice()],
        ))
        .unwrap();
        let converted = document.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");

        let first_columns = document_xml
            .find(r#"<w:cols w:num="2" w:space="720" w:sep="1"/>"#)
            .expect("ending-section equal columns");
        let final_columns = document_xml
            .find(r#"<w:cols w:num="3" w:space="0"/>"#)
            .expect("final-section equal columns");
        assert!(first_columns < final_columns, "{document_xml}");
        assert_eq!(document_xml.matches("<w:bidi/>").count(), 1);

        #[cfg(feature = "render")]
        Document::open(&converted)
            .unwrap()
            .with_render_model_and_hints(|model, hints| {
                let boundary = model
                    .blocks
                    .iter()
                    .position(|block| matches!(block, Block::SectionBreak(_)))
                    .expect("converted section boundary");
                assert_eq!(hints.section_column_gap_pt[boundary], Some(36.0));
                assert!(hints.section_column_separators[boundary]);
                assert!(hints.section_column_rtl[boundary]);
                assert_eq!(hints.final_section_column_gap_pt, Some(0.0));
                assert!(!hints.final_section_column_separator);
                assert!(!hints.final_section_column_rtl);
            });
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_aligns_unequal_column_geometry_and_isolates_malformed_sepx() {
        let section_cps = [0, 5, 10, 15];
        let mut first = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut first, 1);
        push_section_column_width(&mut first, 0, 2_000);
        push_section_column_custom_spacing(&mut first, 0, 400);
        push_section_column_width(&mut first, 1, 4_000);
        push_section_evenly_spaced(&mut first, 0);
        let malformed = [0x03];
        let mut final_section =
            section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut final_section, 1);
        push_section_column_width(&mut final_section, 0, 3_000);
        push_section_column_custom_spacing(&mut final_section, 0, 200);
        push_section_column_width(&mut final_section, 1, 5_000);
        push_section_evenly_spaced(&mut final_section, 0);

        let bytes = legacy_doc_with_section_page_grpprls(
            "AAAAABBBBBCCCCC",
            &section_cps,
            &[
                first.as_slice(),
                malformed.as_slice(),
                final_section.as_slice(),
            ],
        );
        let document = Document::open(&bytes).unwrap();

        document.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 5);
            assert!(hints.section_column_layouts[0].is_none());
            let first_layout = hints.section_column_layouts[1]
                .as_ref()
                .expect("first ending-section geometry");
            assert_eq!(first_layout.columns[0].width_pt, 100.0);
            assert_eq!(first_layout.columns[0].space_after_pt, 20.0);
            assert_eq!(first_layout.columns[1].width_pt, 200.0);
            assert!(hints.section_column_layouts[2].is_none());
            assert!(hints.section_column_layouts[3].is_none());
            assert!(hints.section_column_layouts[4].is_none());

            let final_layout = hints
                .final_section_column_layout
                .expect("final-section geometry");
            assert_eq!(final_layout.columns[0].width_pt, 150.0);
            assert_eq!(final_layout.columns[0].space_after_pt, 10.0);
            assert_eq!(final_layout.columns[1].width_pt, 250.0);
        });
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_unequal_columns_change_preview_pdf_deterministically() {
        let text = "left column\u{000e}right column\r";
        let section_cps = [0, text.encode_utf16().count() as u32];
        let custom_document = |first_width, second_width| {
            let mut section = section_page_grpprl(4_400, 3_000, 400, 400, 400, 400, false);
            push_section_column_count(&mut section, 1);
            push_section_column_width(&mut section, 0, first_width);
            push_section_column_custom_spacing(&mut section, 0, 400);
            push_section_column_width(&mut section, 1, second_width);
            push_section_evenly_spaced(&mut section, 0);
            Document::open(&legacy_doc_with_section_page_grpprls(
                text,
                &section_cps,
                &[section.as_slice()],
            ))
            .unwrap()
        };
        let first_wide = custom_document(2_000, 1_200);
        let second_wide = custom_document(1_200, 2_000);
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        assert_eq!(first_wide.model(), second_wide.model());
        let first_pdf = first_wide.try_to_pdf_with_fonts(&fonts).unwrap();
        let second_pdf = second_wide.try_to_pdf_with_fonts(&fonts).unwrap();
        assert!(first_pdf.starts_with(b"%PDF-"));
        assert_ne!(first_pdf, second_pdf);
        assert_eq!(first_pdf, first_wide.try_to_pdf_with_fonts(&fonts).unwrap());
        assert_eq!(
            first_wide.layout_pages_with_fonts(&fonts).unwrap(),
            first_wide.layout_pages_with_fonts(&fonts).unwrap()
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_aligns_column_separators_and_isolates_malformed_sepx() {
        let section_cps = [0, 5, 10, 15];
        let mut first = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut first, 1);
        push_section_column_separator(&mut first, 1);
        let malformed = [0x03];
        let mut final_section =
            section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut final_section, 1);
        push_section_column_separator(&mut final_section, 1);
        let document = Document::open(&legacy_doc_with_section_page_grpprls(
            "AAAAABBBBBCCCCC",
            &section_cps,
            &[
                first.as_slice(),
                malformed.as_slice(),
                final_section.as_slice(),
            ],
        ))
        .unwrap();

        document.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 5);
            assert_eq!(
                hints.section_column_separators,
                &[false, true, false, false, false]
            );
            assert!(hints.final_section_column_separator);
        });
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_column_separator_changes_pdf_without_layout_change() {
        let text = "legacy separator";
        let section_cps = [0, text.encode_utf16().count() as u32];
        let document = |separator| {
            let mut section = section_page_grpprl(4_400, 3_000, 400, 400, 400, 400, false);
            push_section_column_count(&mut section, 1);
            if separator {
                push_section_column_separator(&mut section, 1);
            }
            Document::open(&legacy_doc_with_section_page_grpprls(
                text,
                &section_cps,
                &[section.as_slice()],
            ))
            .unwrap()
        };
        let baseline = document(false);
        let separated = document(true);
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        assert_eq!(baseline.model(), separated.model());
        assert_eq!(
            baseline.layout_pages_with_fonts(&fonts).unwrap(),
            separated.layout_pages_with_fonts(&fonts).unwrap()
        );
        separated.with_render_model_and_hints(|_, hints| {
            assert!(hints.final_section_column_separator);
        });
        let baseline_pdf = baseline.try_to_pdf_with_fonts(&fonts).unwrap();
        let separated_pdf = separated.try_to_pdf_with_fonts(&fonts).unwrap();
        assert_ne!(baseline_pdf, separated_pdf);
        assert_eq!(
            separated_pdf,
            separated.try_to_pdf_with_fonts(&fonts).unwrap()
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_aligns_rtl_columns_and_isolates_malformed_sepx() {
        let section_cps = [0, 5, 10, 15];
        let mut first = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut first, 1);
        push_section_column_rtl(&mut first, 1);
        let malformed = [0x03];
        let mut final_section =
            section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut final_section, 1);
        push_section_column_rtl(&mut final_section, 1);
        let document = Document::open(&legacy_doc_with_section_page_grpprls(
            "AAAAABBBBBCCCCC",
            &section_cps,
            &[
                first.as_slice(),
                malformed.as_slice(),
                final_section.as_slice(),
            ],
        ))
        .unwrap();

        document.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 5);
            assert_eq!(
                hints.section_column_rtl,
                &[false, true, false, false, false]
            );
            assert!(hints.final_section_column_rtl);
        });
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_rtl_section_populates_columns_from_right_without_layout_change() {
        let text = "legacy right-to-left columns";
        let section_cps = [0, text.encode_utf16().count() as u32];
        let document = |rtl| {
            let mut section = section_page_grpprl(4_400, 3_000, 400, 400, 400, 400, false);
            push_section_column_count(&mut section, 1);
            if rtl {
                push_section_column_rtl(&mut section, 1);
            }
            Document::open(&legacy_doc_with_section_page_grpprls(
                text,
                &section_cps,
                &[section.as_slice()],
            ))
            .unwrap()
        };
        let ltr = document(false);
        let rtl = document(true);
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        assert_eq!(ltr.model(), rtl.model());
        assert_eq!(
            ltr.layout_pages_with_fonts(&fonts).unwrap(),
            rtl.layout_pages_with_fonts(&fonts).unwrap()
        );
        rtl.with_render_model_and_hints(|_, hints| assert!(hints.final_section_column_rtl));
        let ltr_pdf = ltr.try_to_pdf_with_fonts(&fonts).unwrap();
        let rtl_pdf = rtl.try_to_pdf_with_fonts(&fonts).unwrap();
        assert_ne!(ltr_pdf, rtl_pdf);
        assert_eq!(rtl_pdf, rtl.try_to_pdf_with_fonts(&fonts).unwrap());
    }

    #[test]
    fn legacy_doc_sepx_preserves_single_section_columns() {
        let section_cps = [0, 4];
        let mut only = section_page_grpprl(11_520, 16_560, 1_200, 1_600, 800, 1_000, false);
        push_section_column_count(&mut only, 2);
        let sepx_grpprls = [only.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("ONLY", &section_cps, &sepx_grpprls);

        let model = Document::open(&bytes).unwrap().model();

        assert!(model
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::SectionBreak(_))));
        assert_eq!(model.setup.columns, Some(3));
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_section_columns_change_preview_flow() {
        let text = (0..8)
            .map(|index| format!("line {index}\r"))
            .collect::<String>();
        let section_cps = [0, text.encode_utf16().count() as u32];
        let single = section_page_grpprl(4_400, 2_000, 400, 400, 400, 400, false);
        let mut double = single.clone();
        push_section_column_count(&mut double, 1);
        let single_grpprls = [single.as_slice()];
        let double_grpprls = [double.as_slice()];
        let single_bytes =
            legacy_doc_with_section_page_grpprls(&text, &section_cps, &single_grpprls);
        let double_bytes =
            legacy_doc_with_section_page_grpprls(&text, &section_cps, &double_grpprls);
        let single_model = Document::open(&single_bytes).unwrap().model();
        let double_model = Document::open(&double_bytes).unwrap().model();
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        let single_layout = layout_pages_with_fonts(&single_model, &fonts).unwrap();
        let double_layout = layout_pages_with_fonts(&double_model, &fonts).unwrap();

        assert_eq!(single_model.blocks.len(), 8);
        assert_eq!(double_model.blocks.len(), 8);
        assert_eq!(single_model.setup.columns, None);
        assert_eq!(double_model.setup.columns, Some(2));
        assert!(
            double_layout.pages < single_layout.pages,
            "recovered equal-width columns must alter page flow: single={single_layout:?}, double={double_layout:?}"
        );
        assert_eq!(single_layout.block_pages.last(), Some(&Some(2)));
        assert_eq!(double_layout.block_pages.last(), Some(&Some(1)));
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_section_column_spacing_reaches_preview_hints_and_flow() {
        let section_cps = [0, 5, 10];
        let mut first = section_page_grpprl(4_400, 2_000, 400, 400, 400, 400, false);
        push_section_column_count(&mut first, 1);
        push_section_column_spacing(&mut first, 800);
        let mut final_section = first.clone();
        push_section_column_spacing(&mut final_section, 200);
        let sepx_grpprls = [first.as_slice(), final_section.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("FIRSTFINAL", &section_cps, &sepx_grpprls);
        let document = Document::open(&bytes).unwrap();

        document.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 3);
            assert_eq!(hints.section_column_gap_pt, &[None, Some(40.0), None]);
            assert_eq!(hints.final_section_column_gap_pt, Some(10.0));
        });

        let text = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi \
            omicron pi rho sigma tau upsilon phi chi psi omega";
        let section_cps = [0, text.encode_utf16().count() as u32];
        let mut section = section_page_grpprl(4_400, 2_000, 400, 400, 400, 400, false);
        push_section_column_count(&mut section, 1);
        push_section_column_spacing(&mut section, 2_000);
        let section_grpprls = [section.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls(text, &section_cps, &section_grpprls);
        let document = Document::open(&bytes).unwrap();
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        let model_only = layout_pages_with_fonts(&document.model(), &fonts).unwrap();
        let opened = document.layout_pages_with_fonts(&fonts).unwrap();
        assert!(
            opened.pages > model_only.pages,
            "explicit legacy spacing must narrow equal columns: model={model_only:?}, opened={opened:?}"
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_section_columns_switch_at_section_boundary() {
        let first_text = (0..8)
            .map(|index| format!("first {index}\r"))
            .collect::<String>();
        let final_text = (0..8)
            .map(|index| format!("final {index}\r"))
            .collect::<String>();
        let text = format!("{first_text}{final_text}");
        let section_cps = [
            0,
            first_text.encode_utf16().count() as u32,
            text.encode_utf16().count() as u32,
        ];
        let mut first = section_page_grpprl(4_400, 2_000, 400, 400, 400, 400, false);
        push_section_column_count(&mut first, 1);
        let final_section = section_page_grpprl(4_400, 2_000, 400, 400, 400, 400, false);
        let sepx_grpprls = [first.as_slice(), final_section.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls(&text, &section_cps, &sepx_grpprls);
        let model = Document::open(&bytes).unwrap().model();
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        let layout = layout_pages_with_fonts(&model, &fonts).unwrap();

        assert_eq!(model.blocks.len(), 17);
        let Block::SectionBreak(first_setup) = &model.blocks[8] else {
            panic!("expected section boundary");
        };
        assert_eq!(first_setup.columns, Some(2));
        assert_eq!(model.setup.columns, None);
        assert_eq!(layout.pages, 3);
        assert_eq!(layout.block_pages[7], Some(1));
        assert_eq!(layout.block_pages[8], Some(2));
        assert_eq!(layout.block_pages[9], Some(2));
        assert_eq!(layout.block_pages[16], Some(3));
    }

    #[test]
    fn legacy_doc_malformed_column_sepx_does_not_harm_neighbor() {
        let section_cps = [0, 5, 10];
        let mut malformed = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut malformed, 1);
        malformed.push(0xFF);
        let mut valid = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_column_count(&mut valid, 3);
        let sepx_grpprls = [malformed.as_slice(), valid.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("FIRSTFINAL", &section_cps, &sepx_grpprls);

        let model = Document::open(&bytes).unwrap().model();
        let first_columns = model
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::SectionBreak(setup) => Some(setup.columns),
                _ => None,
            })
            .expect("section boundary");

        assert_eq!(first_columns, None);
        assert_eq!(model.setup.columns, Some(4));
    }

    #[test]
    fn legacy_doc_sepx_preserves_even_and_odd_section_breaks() {
        let section_cps = [0, 5, 10, 15];
        let mut first = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        let mut second = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        let mut final_section =
            section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_break_kind(&mut first, 0x03);
        push_section_break_kind(&mut second, 0x04);
        push_section_break_kind(&mut final_section, 0x02);
        let sepx_grpprls = [
            first.as_slice(),
            second.as_slice(),
            final_section.as_slice(),
        ];
        let bytes =
            legacy_doc_with_section_page_grpprls("AAAAABBBBBCCCCC", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let kinds = doc
            .model()
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::SectionBreak(setup) => setup.section_break,
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![SectionBreakKind::EvenPage, SectionBreakKind::OddPage]
        );

        #[cfg(feature = "render")]
        {
            let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
            let model = doc.model();
            let layout = layout_pages_with_fonts(&model, &fonts).expect("layout legacy DOC");
            let section_break_pages = model
                .blocks
                .iter()
                .zip(&layout.block_pages)
                .filter_map(|(block, page)| match block {
                    Block::SectionBreak(setup) => setup.section_break.map(|kind| (kind, *page)),
                    _ => None,
                })
                .collect::<Vec<_>>();

            assert_eq!(layout.pages, 3);
            assert_eq!(
                section_break_pages,
                vec![
                    (SectionBreakKind::EvenPage, Some(2)),
                    (SectionBreakKind::OddPage, Some(3))
                ]
            );
        }

        #[cfg(feature = "docx")]
        {
            let docx = doc.to_docx();
            let document_xml = docx_part(&docx, "word/document.xml");
            let even = document_xml
                .find(r#"<w:type w:val="evenPage"/>"#)
                .expect("even-page section mark");
            let odd = document_xml
                .find(r#"<w:type w:val="oddPage"/>"#)
                .expect("odd-page section mark");
            assert!(even < odd, "section marks must preserve source order");

            let reopened = Document::open(&docx).unwrap();
            let reopened_kinds = reopened
                .model()
                .blocks
                .iter()
                .filter_map(|block| match block {
                    Block::SectionBreak(setup) => setup.section_break,
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(reopened_kinds, kinds);
        }
    }

    #[test]
    fn legacy_doc_malformed_sepx_defaults_its_break_without_harming_neighbor() {
        let section_cps = [0, 5, 10, 15];
        let mut malformed = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_break_kind(&mut malformed, 0x03);
        malformed.push(0xFF);
        let mut valid = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        push_section_break_kind(&mut valid, 0x04);
        let final_section = section_page_grpprl(12_240, 15_840, 1_440, 1_440, 1_440, 1_440, false);
        let sepx_grpprls = [
            malformed.as_slice(),
            valid.as_slice(),
            final_section.as_slice(),
        ];
        let bytes =
            legacy_doc_with_section_page_grpprls("AAAAABBBBBCCCCC", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let kinds = doc
            .model()
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::SectionBreak(setup) => setup.section_break,
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            kinds,
            vec![SectionBreakKind::NextPage, SectionBreakKind::OddPage]
        );
    }

    #[test]
    fn legacy_doc_sepx_preserves_single_section_page_geometry() {
        let section_cps = [0, 4];
        let only = section_page_grpprl(11_520, 16_560, 1_200, 1_600, 800, 1_000, false);
        let sepx_grpprls = [only.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("ONLY", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();

        assert!(model
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::SectionBreak(_))));
        assert_eq!(model.setup.page.width_pt, 576.0);
        assert_eq!(model.setup.page.height_pt, 828.0);
        assert_eq!(model.setup.page.margin_left_pt, Some(60.0));
        assert_eq!(model.setup.page.margin_right_pt, Some(80.0));
        assert_eq!(model.setup.page.margin_top_pt, Some(40.0));
        assert_eq!(model.setup.page.margin_bottom_pt, Some(50.0));
        assert!(!model.setup.page.landscape);
    }

    #[test]
    fn legacy_doc_malformed_sepx_falls_back_per_section() {
        let section_cps = [0, 5, 10];
        let mut malformed = section_page_grpprl(12_240, 15_840, 1_440, 1_800, 720, 900, false);
        malformed.push(0xFF);
        let valid = section_page_grpprl(15_840, 12_240, 2_160, 1_080, 1_440, 720, true);
        let sepx_grpprls = [malformed.as_slice(), valid.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("FIRSTFINAL", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();
        let first_page = model
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::SectionBreak(setup) => Some(setup.page),
                _ => None,
            })
            .expect("outer PlcfSed remains valid");

        assert_eq!(first_page.width_pt, 612.0);
        assert_eq!(first_page.height_pt, 792.0);
        assert_eq!(first_page.margin_left_pt, None);
        assert_eq!(first_page.margin_right_pt, None);
        assert_eq!(first_page.margin_top_pt, None);
        assert_eq!(first_page.margin_bottom_pt, None);
        assert!(!first_page.landscape);
        assert_eq!(model.setup.page.width_pt, 792.0);
        assert_eq!(model.setup.page.height_pt, 612.0);
        assert!(model.setup.page.landscape);
    }

    #[test]
    fn legacy_doc_sepx_partial_geometry_uses_word_defaults() {
        let section_cps = [0, 4];
        let mut partial = Vec::new();
        partial.extend_from_slice(&0xB021u16.to_le_bytes());
        partial.extend_from_slice(&1_440u16.to_le_bytes());
        let sepx_grpprls = [partial.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("ONLY", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let page = doc.model().setup.page;

        assert_eq!(page.width_pt, 612.0);
        assert_eq!(page.height_pt, 792.0);
        assert_eq!(page.margin_left_pt, Some(72.0));
        assert!(!page.landscape);
    }

    #[test]
    fn legacy_doc_plcfsed_clamps_final_cp_beyond_main_story() {
        let section_cps = [0, 5, 12];
        let first = section_page_grpprl(12_240, 15_840, 1_440, 1_800, 720, 900, false);
        let second = section_page_grpprl(15_840, 12_240, 2_160, 1_080, 1_440, 720, true);
        let sepx_grpprls = [first.as_slice(), second.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("FIRSTFINAL", &section_cps, &sepx_grpprls);

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();

        assert_eq!(
            model
                .blocks
                .iter()
                .filter(|block| matches!(block, Block::SectionBreak(_)))
                .count(),
            1
        );
        assert_eq!(model.setup.page.width_pt, 792.0);
        assert_eq!(model.setup.page.height_pt, 612.0);
        assert!(model.setup.page.landscape);
    }

    #[test]
    fn legacy_doc_sections_preserve_unsplit_header_fallback() {
        let section_cps = [0, 5, 10];
        let first = section_page_grpprl(12_240, 15_840, 1_440, 1_800, 720, 900, false);
        let second = section_page_grpprl(15_840, 12_240, 2_160, 1_080, 1_440, 720, true);
        let sepx_grpprls = [first.as_slice(), second.as_slice()];
        let bytes = synth_doc_with_ccp_and_tables(
            "FIRSTFINALHEAD",
            "",
            0x00C1,
            0,
            0,
            [10, 0, 4, 0, 0, 0],
            SyntheticDocTables {
                plcf_sed_cps: Some(&section_cps),
                plcf_sed_sepx_grpprls: Some(&sepx_grpprls),
                ..SyntheticDocTables::default()
            },
        );

        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();

        let first_section = model
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::SectionBreak(setup) => Some(setup),
                _ => None,
            })
            .expect("legacy section boundary");
        assert_eq!(single_paragraph_text(&first_section.header), "HEAD");
        assert_eq!(single_paragraph_text(&model.setup.header), "HEAD");
        assert_eq!(model.setup.page.width_pt, 792.0);
        assert!(model.setup.page.landscape);

        #[cfg(feature = "docx")]
        {
            let reopened = Document::open(&doc.to_docx()).unwrap();
            let reopened_model = reopened.model();
            let reopened_first_section = reopened_model
                .blocks
                .iter()
                .find_map(|block| match block {
                    Block::SectionBreak(setup) => Some(setup),
                    _ => None,
                })
                .expect("DOCX section boundary");
            assert_eq!(
                single_paragraph_text(&reopened_first_section.header),
                "HEAD"
            );
            assert_eq!(single_paragraph_text(&reopened_model.setup.header), "HEAD");
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_sepx_page_geometry_roundtrips_through_docx() {
        let section_cps = [0, 5, 10];
        let first = section_page_grpprl(12_240, 15_840, 1_440, 1_800, 720, 900, false);
        let second = section_page_grpprl(15_840, 12_240, 2_160, 1_080, 1_440, 720, true);
        let sepx_grpprls = [first.as_slice(), second.as_slice()];
        let bytes = legacy_doc_with_section_page_grpprls("FIRSTFINAL", &section_cps, &sepx_grpprls);

        let legacy = Document::open(&bytes).unwrap();
        let docx = legacy.to_docx();
        let document_xml = docx_part(&docx, "word/document.xml");
        assert!(document_xml.contains(r#"<w:pgSz w:w="12240" w:h="15840"/>"#));
        assert!(document_xml.contains(r#"<w:pgSz w:w="15840" w:h="12240" w:orient="landscape"/>"#));

        let reopened = Document::open(&docx).unwrap();
        let reopened_model = reopened.model();
        let first_page = reopened_model
            .blocks
            .iter()
            .find_map(|block| match block {
                Block::SectionBreak(setup) => Some(setup.page),
                _ => None,
            })
            .expect("DOCX section boundary");
        assert_eq!(first_page.width_pt, 612.0);
        assert_eq!(first_page.height_pt, 792.0);
        assert_eq!(first_page.margin_left_pt, Some(72.0));
        assert_eq!(first_page.margin_right_pt, Some(90.0));
        assert_eq!(first_page.margin_top_pt, Some(36.0));
        assert_eq!(first_page.margin_bottom_pt, Some(45.0));
        assert!(!first_page.landscape);

        assert_eq!(reopened_model.setup.page.width_pt, 792.0);
        assert_eq!(reopened_model.setup.page.height_pt, 612.0);
        assert_eq!(reopened_model.setup.page.margin_left_pt, Some(108.0));
        assert_eq!(reopened_model.setup.page.margin_right_pt, Some(54.0));
        assert_eq!(reopened_model.setup.page.margin_top_pt, Some(72.0));
        assert_eq!(reopened_model.setup.page.margin_bottom_pt, Some(36.0));
        assert!(reopened_model.setup.page.landscape);
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_plcfsed_sections_roundtrip_to_docx_sectpr_refs() {
        let bytes = two_section_legacy_header_footer_doc(None);
        let doc = Document::open(&bytes).unwrap();
        let docx = doc.to_docx();
        let document_xml = docx_part(&docx, "word/document.xml");

        assert_eq!(document_xml.matches("<w:sectPr").count(), 2);
        assert_eq!(
            document_xml
                .matches(r#"<w:headerReference w:type="default""#)
                .count(),
            2
        );
        assert!(
            document_xml.contains(r#"<w:type w:val="nextPage"/>"#),
            "legacy section break should serialize as nextPage: {document_xml}"
        );
        for marker in [
            "E0", "O0", "e0", "o0", "F0", "f0", "E1", "O1", "e1", "o1", "F1", "f1",
        ] {
            assert!(
                !document_xml.contains(&format!(">{marker}</w:t>")),
                "promoted running story {marker:?} leaked into the body: {document_xml}"
            );
        }

        let mut zip = zip::ZipArchive::new(Cursor::new(docx)).unwrap();
        let mut header_xml = Vec::new();
        for index in 0..zip.len() {
            let mut file = zip.by_index(index).unwrap();
            let name = file.name().to_string();
            if name.starts_with("word/header") && name.ends_with(".xml") {
                let mut body = String::new();
                file.read_to_string(&mut body).unwrap();
                header_xml.push(body);
            }
        }
        assert!(
            header_xml.iter().any(|xml| xml.contains(">O0<")),
            "first section odd header part missing: {header_xml:?}"
        );
        assert!(
            header_xml.iter().any(|xml| xml.contains(">O1<")),
            "final section odd header part missing: {header_xml:?}"
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_running_paragraph_column_breaks_roundtrip_to_docx() {
        let document = Document::open(&two_section_legacy_running_column_break_doc()).unwrap();
        let source_model = document.model();
        let converted = document.to_docx();
        assert_eq!(converted, document.to_docx());
        assert_eq!(document.model(), source_model);

        let document_xml = docx_part(&converted, "word/document.xml");
        let parts = docx_running_parts(&converted);
        assert_eq!(parts.len(), 12, "{parts:?}");
        for marker in [
            "E0", "O0", "e0", "o0", "F0", "f0", "E1", "O1", "e1", "o1", "F1", "f1",
        ] {
            assert!(!document_xml.contains(&format!(">{marker}A</w:t>")));
            let selected = parts
                .iter()
                .filter(|(_, xml)| xml.contains(&format!(">{marker}A</w:t>")))
                .collect::<Vec<_>>();
            assert_eq!(selected.len(), 1, "{marker}: {parts:?}");
            assert_eq!(
                selected[0].1.matches(r#"<w:br w:type="column"/>"#).count(),
                1,
                "{marker}: {}",
                selected[0].1
            );
            assert!(!selected[0].1.contains("<w:br/>"), "{}", selected[0].1);
        }

        let reopened = Document::open(&converted).unwrap();
        assert_eq!(reopened.to_docx(), converted);

        let model_only = write_docx(&source_model);
        let model_only_parts = docx_running_parts(&model_only);
        assert_eq!(model_only_parts.len(), 12, "{model_only_parts:?}");
        assert!(model_only_parts.iter().all(|(_, xml)| {
            !xml.contains(r#"<w:br w:type="column"/>"#) && xml.matches("<w:br/>").count() == 1
        }));
    }

    #[test]
    fn legacy_doc_truncated_plcfsed_keeps_single_section_setup() {
        let bytes = two_section_legacy_header_footer_doc(Some(8));
        let doc = Document::open(&bytes).unwrap();
        let model = doc.model();

        assert_eq!(
            model
                .blocks
                .iter()
                .filter(|block| matches!(block, Block::SectionBreak(_)))
                .count(),
            0
        );
        assert_eq!(single_paragraph_text(&model.setup.header), "O0");
        assert_eq!(single_paragraph_text(&model.setup.even_header), "E0");
        assert_eq!(
            doc.header_footers()
                .iter()
                .filter(|record| record.section == Some(1))
                .count(),
            6
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_fresh_conversion_keeps_unselected_running_fallback_stories() {
        let bytes = two_section_legacy_header_footer_doc(Some(8));
        let doc = Document::open(&bytes).unwrap();
        let converted = doc.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");

        for marker in ["E0", "O0", "e0", "o0", "F0", "f0"] {
            assert!(
                !document_xml.contains(&format!(">{marker}</w:t>")),
                "promoted first-owner story {marker:?} leaked into the body: {document_xml}"
            );
        }
        for marker in ["E1", "O1", "e1", "o1", "F1", "f1"] {
            assert!(
                document_xml.contains(&format!(">{marker}</w:t>")),
                "unselected fallback story {marker:?} was dropped: {document_xml}"
            );
        }

        let running_parts = docx_running_parts(&converted);
        assert!(
            running_parts
                .iter()
                .any(|(_, xml)| xml.contains(">O0</w:t>")),
            "selected first-owner header missing: {running_parts:?}"
        );
        assert!(
            running_parts
                .iter()
                .all(|(_, xml)| !xml.contains(">O1</w:t>")),
            "unselected story was incorrectly promoted: {running_parts:?}"
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn docx_header_footer_parts_are_exposed_with_exact_kinds() {
        let model = DocBuilder::new()
            .header("DOCX HEAD")
            .footer("DOCX FOOT")
            .paragraph("Body")
            .build();
        let doc = Document::open(&write_docx(&model)).unwrap();

        let header_footers = doc.header_footers();
        assert_eq!(header_footers.len(), 2);
        assert_eq!(header_footers[0].id, "word/header1.xml#default");
        assert_eq!(header_footers[0].kind, HeaderFooterKind::Header);
        assert_eq!(header_footers[0].text, "DOCX HEAD");
        assert_eq!(header_footers[1].id, "word/footer1.xml#default");
        assert_eq!(header_footers[1].kind, HeaderFooterKind::Footer);
        assert_eq!(header_footers[1].text, "DOCX FOOT");
    }

    #[test]
    fn refuses_encrypted_document() {
        // fEncrypted = bit 8 (0x0100); fObfuscated = bit 15 (0x8000).
        let bytes = synth_doc_ex("x", "y", 0x00C1, 0, 0x0100 | 0x8000);
        assert!(matches!(
            Document::open(&bytes),
            Err(Error::Encrypted { obfuscated: true })
        ));
    }

    #[test]
    fn refuses_pre_word97_version() {
        // nFib 0x0065 = Word 6.0 (< 0x00C1).
        let bytes = synth_doc_ex("x", "y", 0x0065, 0, 0);
        assert!(matches!(
            Document::open(&bytes),
            Err(Error::UnsupportedVersion(0x0065))
        ));
    }

    #[test]
    fn lid_selects_korean_codepage() {
        // Korean lid 0x0412 -> cp949 -> EUC_KR; default lid -> cp1252.
        let kr = synth_doc_ex("본문", "", 0x00C1, 0x0412, 0);
        let doc = Document::open(&kr).unwrap();
        // The codepage surfaces on the model metadata (fib is now backend-private).
        assert_eq!(doc.model().meta.codepage, 949);
        assert!(std::ptr::eq(
            text::encoding_for_codepage(949),
            encoding_rs::EUC_KR
        ));
        assert!(std::ptr::eq(
            text::encoding_for_codepage(0),
            encoding_rs::WINDOWS_1252
        ));
    }

    #[test]
    fn rejects_non_ole2() {
        assert!(matches!(extract_text(b"not a doc"), Err(Error::NotOle2)));
    }

    #[test]
    fn opened_legacy_doc_uses_lfolvl_start_override() {
        let text = "alpha\rbeta\r";
        let text_end = text.encode_utf16().count() as u32;
        let runs = [SyntheticPapxRun {
            cp_lim: text_end,
            grpprl: vec![
                0x0A, 0x26, 0x00, // sprmPIlvl = 0
                0x0B, 0x46, 0x01, 0x00, // sprmPIlfo = 1
            ],
        }];

        let mut list_header = Vec::new();
        list_header.extend_from_slice(&1i16.to_le_bytes());
        let mut lstf = [0u8; 28];
        lstf[0..4].copy_from_slice(&42i32.to_le_bytes());
        lstf[26] = 0x01;
        list_header.extend_from_slice(&lstf);

        let mut list_level = vec![0u8; 28];
        list_level[0..4].copy_from_slice(&1i32.to_le_bytes());
        list_level[6] = 1;
        list_level[15] = 1;
        list_level.extend_from_slice(&2u16.to_le_bytes());
        list_level.extend_from_slice(&0u16.to_le_bytes());
        list_level.extend_from_slice(&('.' as u16).to_le_bytes());

        let mut list_overrides = Vec::new();
        list_overrides.extend_from_slice(&1u32.to_le_bytes());
        let mut lfo = [0u8; 16];
        lfo[0..4].copy_from_slice(&42i32.to_le_bytes());
        lfo[12] = 1;
        list_overrides.extend_from_slice(&lfo);
        list_overrides.extend_from_slice(&0u32.to_le_bytes());
        list_overrides.extend_from_slice(&5i32.to_le_bytes());
        list_overrides.extend_from_slice(&(1u32 << 4).to_le_bytes());

        let bytes = synth_doc_with_ccp_and_tables(
            text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                list_definition: Some((&list_header, &list_level)),
                list_overrides: Some(&list_overrides),
                ..SyntheticDocTables::default()
            },
        );
        let document = Document::open(&bytes).expect("synthetic legacy list document opens");

        assert_eq!(document.text(), "5. alpha\n6. beta");
        let model = document.model();
        let labels: Vec<_> = model
            .blocks
            .iter()
            .filter_map(|block| match block {
                Block::Paragraph(paragraph) => paragraph
                    .props
                    .list
                    .as_ref()
                    .map(|list| list.label.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(labels, ["5. ", "6. "]);
    }

    #[test]
    fn missing_word_document_stream_errors() {
        let mut comp = cfb::CompoundFile::create(Cursor::new(Vec::new())).unwrap();
        comp.create_stream("/1Table")
            .unwrap()
            .write_all(b"x")
            .unwrap();
        comp.flush().unwrap();
        let bytes = comp.into_inner().into_inner();
        assert!(matches!(
            Document::open(&bytes),
            Err(Error::MissingStream("WordDocument"))
        ));
    }

    /// Build a minimal `.docx` (ZIP of OOXML parts) in memory and read it
    /// end-to-end through the *same* public API as `.doc`, proving format
    /// detection and that both backends feed the shared model/exporters.
    #[cfg(feature = "docx")]
    #[test]
    fn reads_a_minimal_docx_through_the_shared_model() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let png = [0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3, 4];
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        let text_parts = [
            (
                "word/_rels/document.xml.rels",
                r#"<Relationships><Relationship Id="rId1" Type="http://x/image" Target="media/image1.png"/></Relationships>"#,
            ),
            (
                "word/styles.xml",
                r#"<w:styles><w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style></w:styles>"#,
            ),
            (
                "word/numbering.xml",
                r#"<w:numbering><w:abstractNum w:abstractNumId="0"><w:lvl w:ilvl="0"><w:numFmt w:val="decimal"/></w:lvl></w:abstractNum><w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num></w:numbering>"#,
            ),
            (
                "word/document.xml",
                r#"<w:document><w:body>
                    <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>제목</w:t></w:r></w:p>
                    <w:p><w:r><w:rPr><w:b/></w:rPr><w:t>굵게</w:t></w:r><w:r><w:t> 보통</w:t></w:r></w:p>
                    <w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>항목</w:t></w:r></w:p>
                    <w:p><w:r><w:drawing><a:blip r:embed="rId1"/></w:drawing></w:r></w:p>
                    <w:tbl>
                        <w:tr><w:tc><w:p><w:r><w:t>A</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr>
                        <w:tr><w:tc><w:p><w:r><w:t>1</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc></w:tr>
                    </w:tbl>
                </w:body></w:document>"#,
            ),
        ];
        for (name, body) in text_parts {
            zw.start_file(name, opt).unwrap();
            zw.write_all(body.as_bytes()).unwrap();
        }
        zw.start_file("word/media/image1.png", opt).unwrap();
        zw.write_all(&png).unwrap();
        let bytes = zw.finish().unwrap().into_inner();

        let doc = Document::open(&bytes).unwrap();

        // Flat text: heading, emphasis run merge, list item, tab-joined table row.
        let text = doc.text();
        assert!(text.contains("제목"), "{text:?}");
        assert!(text.contains("굵게 보통"), "{text:?}");
        assert!(text.contains("항목"), "{text:?}");
        assert!(text.contains("A\tB"), "{text:?}");

        // Markdown via the shared exporter.
        let md = doc.to_markdown();
        assert!(md.contains("# 제목"), "{md}");
        assert!(md.contains("**굵게**"), "{md}");
        assert!(md.contains("1. 항목"), "{md}"); // numbering → ordered list
        assert!(md.contains("| A | B |"), "{md}");

        // HTML via the shared exporter.
        let html = doc.to_html();
        assert!(html.contains("<h1>제목</h1>"), "{html}");
        assert!(html.contains("<strong>굵게</strong>"), "{html}");

        // Image extraction through the shared accessor.
        let imgs = doc.images();
        assert_eq!(imgs.len(), 1);
        assert_eq!(imgs[0].mime.as_deref(), Some("image/png"));
        assert_eq!(imgs[0].bytes.as_deref(), Some(&png[..]));

        assert!(!doc.is_complex());
        assert!(doc.model().meta.stats.tables >= 1);
    }

    #[cfg(feature = "docx")]
    #[test]
    fn docx_magic_routes_to_docx_backend() {
        // A truncated/garbage ZIP is a clean Docx error, never an OLE2 error.
        assert!(matches!(
            Document::open(b"PK\x03\x04garbage"),
            Err(Error::Docx(_))
        ));
    }

    /// Unzip a `.docx` into a name→bytes map for byte-level part comparison.
    #[cfg(feature = "docx")]
    fn unzip_parts(bytes: &[u8]) -> std::collections::BTreeMap<String, Vec<u8>> {
        use std::io::Read;
        let mut z = zip::ZipArchive::new(Cursor::new(bytes.to_vec())).unwrap();
        let mut m = std::collections::BTreeMap::new();
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let n = f.name().to_string();
            let mut b = Vec::new();
            f.read_to_end(&mut b).unwrap();
            m.insert(n, b);
        }
        m
    }

    /// PR2: a `.docx` (with **unique** part names) opened then saved is byte-stable for
    /// every part (the package-preserving no-op round-trip — nothing the model doesn't
    /// carry is touched or dropped). Duplicate-part-name normalization is a separate,
    /// documented behavior (collapsed to the single entry the ZIP reader exposes) covered
    /// by `opc::tests::duplicate_part_names_collapse_deterministically`.
    #[cfg(feature = "docx")]
    #[test]
    fn roundtrip_preserves_unmodeled_parts() {
        // A heading + body ⇒ several parts: document.xml, styles.xml, rels, CT.
        let model = DocModel {
            blocks: vec![
                Block::Paragraph(Paragraph {
                    props: ParaProps {
                        heading_level: Some(1),
                        ..Default::default()
                    },
                    runs: vec![Run {
                        text: "제목".into(),
                        ..Default::default()
                    }],
                }),
                Block::Paragraph(Paragraph {
                    runs: vec![Run {
                        text: "본문".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
            ],
            ..Default::default()
        };
        // Inject parts rwml does NOT model — a custom XML item and an entirely
        // unknown binary part — to prove the round-trip preserves arbitrary content,
        // not just the parts the writer happens to emit.
        let orig = {
            use std::io::{Read, Write};
            use zip::write::SimpleFileOptions;
            let gen = write_docx(&model);
            let mut zin = zip::ZipArchive::new(Cursor::new(gen)).unwrap();
            let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
            let opt = SimpleFileOptions::default();
            for i in 0..zin.len() {
                let mut f = zin.by_index(i).unwrap();
                let name = f.name().to_string();
                let mut b = Vec::new();
                f.read_to_end(&mut b).unwrap();
                // Type the unmodeled binary part so it's a valid Word-openable OPC part
                // (not just an untyped extra entry): add a `bin` Default to [Content_Types].
                if name == "[Content_Types].xml" {
                    let s = String::from_utf8(b).unwrap().replace(
                        "</Types>",
                        r#"<Default Extension="bin" ContentType="application/octet-stream"/></Types>"#,
                    );
                    b = s.into_bytes();
                }
                zw.start_file(name, opt).unwrap();
                zw.write_all(&b).unwrap();
            }
            zw.start_file("customXml/item1.xml", opt).unwrap();
            zw.write_all(br#"<?xml version="1.0"?><root note="keep me"/>"#)
                .unwrap();
            zw.start_file("word/unknownPart.bin", opt).unwrap();
            zw.write_all(&[0u8, 1, 2, 3, 255, 254]).unwrap();
            zw.finish().unwrap().into_inner()
        };
        let saved = Document::open(&orig).unwrap().save().unwrap();

        let a = unzip_parts(&orig);
        let b = unzip_parts(&saved);
        assert_eq!(
            a.keys().collect::<Vec<_>>(),
            b.keys().collect::<Vec<_>>(),
            "part set changed on no-op save"
        );
        for (name, bytes) in &a {
            assert_eq!(bytes, &b[name], "part {name} not byte-stable on no-op save");
        }
        assert!(a.contains_key("word/styles.xml"), "fixture lacked styles");
        // The unmodeled parts survived byte-for-byte.
        assert_eq!(
            b.get("customXml/item1.xml").map(|v| v.as_slice()),
            Some(&br#"<?xml version="1.0"?><root note="keep me"/>"#[..]),
            "custom XML part not preserved"
        );
        assert_eq!(
            b.get("word/unknownPart.bin").map(|v| v.as_slice()),
            Some(&[0u8, 1, 2, 3, 255, 254][..]),
            "unknown binary part not preserved"
        );
        // And the saved package still types both unmodeled parts (content-type
        // correctness, not just byte passthrough), with [Content_Types].xml byte-stable.
        let pkg = crate::opc::Package::from_zip(&saved).unwrap();
        assert!(pkg.part_has_content_type("word/unknownPart.bin"));
        assert!(pkg.part_has_content_type("customXml/item1.xml"));
        assert_eq!(
            a.get("[Content_Types].xml"),
            b.get("[Content_Types].xml"),
            "[Content_Types].xml not byte-stable on no-op save"
        );
    }

    /// PR2: `Document::new()` is a valid blank package that saves and re-opens.
    #[cfg(feature = "docx")]
    #[test]
    fn new_from_template_saves_and_reopens() {
        let doc = Document::new();
        assert!(
            doc.text().trim().is_empty(),
            "blank template should have no body text, got {:?}",
            doc.text()
        );
        let bytes = doc.save().unwrap();
        let reopened = Document::open(&bytes).unwrap();
        assert!(reopened.text().trim().is_empty());
        assert!(unzip_parts(&bytes).contains_key("word/document.xml"));
    }

    /// Build a `.docx` whose body carries exactly what the lossy model drops — a
    /// content control, a field, an mc:AlternateContent shape, and a comment
    /// reference — plus a comments.xml satellite, to prove B preserves them.
    #[cfg(feature = "docx")]
    fn docx_rich_body() -> Vec<u8> {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        let parts: [(&str, &str); 5] = [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/comments.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/_rels/document.xml.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments" Target="comments.xml"/></Relationships>"#,
            ),
            (
                "word/document.xml",
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p><w:sdt><w:sdtContent><w:p><w:r><w:t>SDT-CONTENT</w:t></w:r></w:p></w:sdtContent></w:sdt><w:p><w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple></w:p><mc:AlternateContent><mc:Choice Requires="wps"><w:drawing/></mc:Choice></mc:AlternateContent><w:p><w:commentRangeStart w:id="0"/><w:r><w:t>commented</w:t></w:r><w:commentRangeEnd w:id="0"/><w:r><w:commentReference w:id="0"/></w:r></w:p></w:body></w:document>"#,
            ),
            (
                "word/comments.xml",
                r#"<w:comments xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:comment w:id="0" w:author="A"><w:p><w:r><w:t>note</w:t></w:r></w:p></w:comment></w:comments>"#,
            ),
        ];
        for (name, body) in parts {
            zw.start_file(name, opt).unwrap();
            zw.write_all(body.as_bytes()).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    /// A genuinely valid 2×3 RGB PNG (correct chunk CRCs + a real zlib `IDAT`) for
    /// image-insertion tests — passes [`is_png`]'s full CRC-checked validation.
    #[cfg(feature = "docx")]
    fn tiny_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x36, 0x88, 0x49, 0xD6, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78,
            0xDA, 0x63, 0x60, 0xC0, 0x02, 0x00, 0x00, 0x15, 0x00, 0x01, 0x39, 0xC1, 0xE0, 0x23,
            0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]
    }

    /// Wrap a `document.xml` body in a minimal valid package (CT + root rels). The
    /// caller supplies the full `<w:document>…</w:document>` string.
    #[cfg(feature = "docx")]
    fn minimal_docx(document_xml: &str) -> Vec<u8> {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        for (n, b) in [
            ("[Content_Types].xml", r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_string()),
            ("_rels/.rels", r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_string()),
            ("word/document.xml", document_xml.to_string()),
        ] {
            zw.start_file(n, opt).unwrap();
            zw.write_all(b.as_bytes()).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    #[cfg(feature = "docx")]
    fn minimal_docx_with_styles(document_xml: &str, styles_xml: &str) -> Vec<u8> {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        for (name, body) in [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/_rels/document.xml.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#,
            ),
            ("word/document.xml", document_xml),
            ("word/styles.xml", styles_xml),
        ] {
            zw.start_file(name, opt).unwrap();
            zw.write_all(body.as_bytes()).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    fn prm0(isprm: u8, value: u8) -> u16 {
        (u16::from(value) << 8) | (u16::from(isprm) << 1)
    }

    fn prm1(index: u16) -> u16 {
        assert!(index <= 0x7FFF);
        (index << 1) | 1
    }

    fn legacy_pcd_prm0_doc(
        text_utf16: &str,
        ansi_tail: &str,
        piece_prms: [u16; 2],
        chpx_runs: Option<&[SyntheticChpxRun]>,
    ) -> Vec<u8> {
        let ccp_text = text_utf16
            .encode_utf16()
            .count()
            .saturating_add(ansi_tail.len()) as u32;
        synth_doc_with_ccp_and_tables(
            text_utf16,
            ansi_tail,
            0x00C1,
            0,
            0,
            [ccp_text, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                piece_prms,
                chpx_runs,
                ..SyntheticDocTables::default()
            },
        )
    }

    fn legacy_pcd_prm1_doc(
        text_utf16: &str,
        ansi_tail: &str,
        piece_prms: [u16; 2],
        prcs: &[&[u8]],
        chpx_runs: Option<&[SyntheticChpxRun]>,
    ) -> Vec<u8> {
        let ccp_text = text_utf16
            .encode_utf16()
            .count()
            .saturating_add(ansi_tail.len()) as u32;
        synth_doc_with_ccp_and_tables(
            text_utf16,
            ansi_tail,
            0x00C1,
            0,
            0,
            [ccp_text, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                prcs: Some(prcs),
                piece_prms,
                chpx_runs,
                ..SyntheticDocTables::default()
            },
        )
    }

    fn paragraph_run_toggles(model: &DocModel) -> Vec<(&str, [bool; 6])> {
        let Block::Paragraph(paragraph) = &model.blocks[0] else {
            panic!("synthetic legacy block must be a paragraph");
        };
        paragraph
            .runs
            .iter()
            .map(|run| {
                (
                    run.text.as_str(),
                    [
                        run.props.bold,
                        run.props.italic,
                        run.props.strike,
                        run.props.small_caps,
                        run.props.caps,
                        run.props.hidden,
                    ],
                )
            })
            .collect()
    }

    #[test]
    fn opened_legacy_doc_applies_literal_pcd_prm0_character_toggles() {
        for (isprm, expected) in [
            (0x55, [true, false, false, false, false, false]),
            (0x56, [false, true, false, false, false, false]),
            (0x57, [false, false, true, false, false, false]),
            (0x5A, [false, false, false, true, false, false]),
            (0x5B, [false, false, false, false, true, false]),
            (0x5C, [false, false, false, false, false, true]),
        ] {
            let bytes = legacy_pcd_prm0_doc("X\r", "", [prm0(isprm, 1), 0], None);
            let document = Document::open(&bytes).unwrap();
            let model = document.model();

            assert_eq!(
                paragraph_run_toggles(&model),
                vec![("X", expected)],
                "literal Prm0 isprm 0x{isprm:02X} was not applied"
            );
        }
    }

    #[test]
    fn opened_legacy_doc_applies_bounded_pcd_prm1_character_formatting() {
        let formatting = [
            0x35, 0x08, 1, // bold
            0x36, 0x08, 1, // italic
            0x37, 0x08, 1, // strike
            0x3C, 0x08, 1, // hidden
            0x3A, 0x08, 1, // small caps
            0x3B, 0x08, 1, // caps
            0x5A, 0x08, 1, // RTL
            0x3E, 0x2A, 1, // underline
            0x0C, 0x2A, 7, // yellow highlight
            0x48, 0x2A, 1, // superscript
        ];
        let prcs = [&formatting[..]];
        let bytes = legacy_pcd_prm1_doc("Complex\r", "", [prm1(0), 0], &prcs, None);
        let model = Document::open(&bytes).unwrap().model();

        assert_eq!(paragraph_run_toggles(&model), vec![("Complex", [true; 6])]);
        let Block::Paragraph(paragraph) = &model.blocks[0] else {
            panic!("synthetic legacy block must be a paragraph");
        };
        let props = &paragraph.runs[0].props;
        assert!(props.underline);
        assert!(props.rtl);
        assert_eq!(props.highlight.as_deref(), Some("yellow"));
        assert_eq!(props.vert_align, VertAlign::Super);
    }

    #[test]
    fn legacy_doc_pcd_prm1_stays_aligned_across_piece_encodings_and_surrogates() {
        let bold = [0x35, 0x08, 1];
        let italic = [0x36, 0x08, 1];
        let prcs = [&bold[..], &italic[..]];
        let bytes = legacy_pcd_prm1_doc("A\u{1F600}", "Tail\r", [prm1(0), prm1(1)], &prcs, None);
        let document = Document::open(&bytes).unwrap();

        assert_eq!(document.text(), "A\u{1F600}Tail");
        assert_eq!(
            paragraph_run_toggles(&document.model()),
            vec![
                ("A\u{1F600}", [true, false, false, false, false, false]),
                ("Tail", [false, true, false, false, false, false]),
            ]
        );
    }

    #[test]
    fn legacy_doc_pcd_prm1_overrides_chpx_and_preserves_unmodified_properties() {
        let chpx = [SyntheticChpxRun {
            cp_lim: 4,
            grpprl: vec![
                0x35, 0x08, 1, // bold
                0x0C, 0x2A, 7, // yellow highlight
            ],
        }];
        let overlay = [
            0x35, 0x08, 0, // bold off
            0x36, 0x08, 1, // italic on
        ];
        let prcs = [&overlay[..]];
        let bytes = legacy_pcd_prm1_doc("Mix\r", "", [prm1(0), 0], &prcs, Some(&chpx));
        let model = Document::open(&bytes).unwrap().model();

        assert_eq!(
            paragraph_run_toggles(&model),
            vec![("Mix", [false, true, false, false, false, false])]
        );
        let Block::Paragraph(paragraph) = &model.blocks[0] else {
            panic!("synthetic legacy block must be a paragraph");
        };
        assert_eq!(paragraph.runs[0].props.highlight.as_deref(), Some("yellow"));
    }

    #[test]
    fn missing_or_style_dependent_pcd_prm1_keeps_chpx_formatting() {
        let bold_chpx = [SyntheticChpxRun {
            cp_lim: 4,
            grpprl: vec![0x35, 0x08, 1],
        }];
        let style_relative = [0x35, 0x08, 0x80];
        let prcs = [&style_relative[..]];
        for raw_prm in [prm1(0), prm1(1)] {
            let bytes = legacy_pcd_prm1_doc("Keep\r", "", [raw_prm, 0], &prcs, Some(&bold_chpx));
            assert_eq!(
                paragraph_run_toggles(&Document::open(&bytes).unwrap().model()),
                vec![("Keep", [true, false, false, false, false, false])]
            );
        }
    }

    #[test]
    fn equal_effective_pcd_prm1_properties_coalesce_across_piece_boundaries() {
        let bold_a = [0x35, 0x08, 1];
        let bold_b = [0x35, 0x08, 1];
        let prcs = [&bold_a[..], &bold_b[..]];
        let bytes = legacy_pcd_prm1_doc("A\u{1F600}", "Tail\r", [prm1(0), prm1(1)], &prcs, None);

        assert_eq!(
            paragraph_run_toggles(&Document::open(&bytes).unwrap().model()),
            vec![("A\u{1F600}Tail", [true, false, false, false, false, false])]
        );
    }

    #[test]
    fn legacy_doc_pcd_prm1_stays_aligned_across_story_regions() {
        let bold = [0x35, 0x08, 1];
        let italic = [0x36, 0x08, 1];
        let prcs = [&bold[..], &italic[..]];
        let bytes = synth_doc_with_ccp_and_tables(
            "Main\r",
            "Note\r",
            0x00C1,
            0,
            0,
            [5, 5, 0, 0, 0, 0],
            SyntheticDocTables {
                prcs: Some(&prcs),
                piece_prms: [prm1(0), prm1(1)],
                ..SyntheticDocTables::default()
            },
        );
        let model = Document::open(&bytes).unwrap().model();

        for (kind, text, expected) in [
            (
                SourceRegionKind::Main,
                "Main",
                [true, false, false, false, false, false],
            ),
            (
                SourceRegionKind::Footnote,
                "Note",
                [false, true, false, false, false, false],
            ),
        ] {
            let region = model
                .regions
                .iter()
                .find(|region| region.kind == kind)
                .expect("synthetic source region");
            let Block::Paragraph(paragraph) = &model.blocks[region.block_start] else {
                panic!("synthetic source region must contain a paragraph");
            };
            assert_eq!(paragraph.runs.len(), 1);
            assert_eq!(paragraph.runs[0].text, text);
            assert_eq!(
                [
                    paragraph.runs[0].props.bold,
                    paragraph.runs[0].props.italic,
                    paragraph.runs[0].props.strike,
                    paragraph.runs[0].props.small_caps,
                    paragraph.runs[0].props.caps,
                    paragraph.runs[0].props.hidden,
                ],
                expected
            );
        }
    }

    #[test]
    fn legacy_doc_pcd_prm1_reaches_every_modeled_story_region() {
        let bold = [0x35, 0x08, 1];
        let prcs = [&bold[..]];
        let bytes = synth_doc_with_ccp_and_tables(
            "BODYFTNHEADANNENDBOX",
            "",
            0x00C1,
            0,
            0,
            [4, 3, 4, 3, 3, 3],
            SyntheticDocTables {
                prcs: Some(&prcs),
                piece_prms: [prm1(0), 0],
                ..SyntheticDocTables::default()
            },
        );
        let model = Document::open(&bytes).unwrap().model();

        for (kind, text) in [
            (SourceRegionKind::Main, "BODY"),
            (SourceRegionKind::Footnote, "FTN"),
            (SourceRegionKind::HeaderFooter, "HEAD"),
            (SourceRegionKind::Annotation, "ANN"),
            (SourceRegionKind::Endnote, "END"),
            (SourceRegionKind::TextBox, "BOX"),
        ] {
            let region = model
                .regions
                .iter()
                .find(|region| region.kind == kind)
                .expect("synthetic source region");
            let Block::Paragraph(paragraph) = &model.blocks[region.block_start] else {
                panic!("synthetic source region must contain a paragraph");
            };
            assert_eq!(paragraph.runs.len(), 1);
            assert_eq!(paragraph.runs[0].text, text);
            assert!(paragraph.runs[0].props.bold);
        }
    }

    #[test]
    fn explicit_clear_pcd_prm1_properties_coalesce_with_implicit_defaults() {
        let clears = [
            0x35, 0x08, 0, // bold
            0x36, 0x08, 0, // italic
            0x37, 0x08, 0, // strike
            0x3C, 0x08, 0, // hidden
            0x3A, 0x08, 0, // small caps
            0x3B, 0x08, 0, // caps
            0x5A, 0x08, 0, // RTL
            0x3E, 0x2A, 0, // underline
            0x0C, 0x2A, 0, // highlight
            0x48, 0x2A, 0, // baseline
        ];
        let prcs = [&clears[..]];
        let bytes = legacy_pcd_prm1_doc("A\u{1F600}", "Tail\r", [prm1(0), 0], &prcs, None);
        let model = Document::open(&bytes).unwrap().model();

        assert_eq!(
            paragraph_run_toggles(&model),
            vec![("A\u{1F600}Tail", [false; 6])]
        );
        let Block::Paragraph(paragraph) = &model.blocks[0] else {
            panic!("synthetic legacy block must be a paragraph");
        };
        let props = &paragraph.runs[0].props;
        assert!(!props.underline);
        assert!(!props.rtl);
        assert_eq!(props.highlight, None);
        assert_eq!(props.vert_align, VertAlign::Baseline);
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_pcd_prm1_character_formatting_roundtrips_to_docx() {
        let formatting = [
            0x35, 0x08, 1, // bold
            0x36, 0x08, 1, // italic
            0x37, 0x08, 1, // strike
            0x3C, 0x08, 1, // hidden
            0x3A, 0x08, 1, // small caps
            0x3B, 0x08, 1, // caps
            0x5A, 0x08, 1, // RTL
            0x3E, 0x2A, 1, // underline
            0x0C, 0x2A, 7, // yellow highlight
            0x48, 0x2A, 2, // subscript
        ];
        let prcs = [&formatting[..]];
        let legacy =
            Document::open(&legacy_pcd_prm1_doc("X\r", "", [prm1(0), 0], &prcs, None)).unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();

        assert_eq!(
            paragraph_run_toggles(&reopened.model()),
            vec![("X", [true; 6])]
        );
        let Block::Paragraph(paragraph) = &reopened.model().blocks[0] else {
            panic!("reopened DOCX must contain a paragraph");
        };
        let props = &paragraph.runs[0].props;
        assert!(props.underline);
        assert!(props.rtl);
        assert_eq!(props.highlight.as_deref(), Some("yellow"));
        assert_eq!(props.vert_align, VertAlign::Sub);
    }

    #[test]
    fn legacy_doc_pcd_prm0_stays_aligned_across_piece_encodings_and_surrogates() {
        let bytes =
            legacy_pcd_prm0_doc("A\u{1F600}", "Tail\r", [prm0(0x55, 1), prm0(0x56, 1)], None);
        let document = Document::open(&bytes).unwrap();
        assert_eq!(document.text(), "A\u{1F600}Tail");

        assert_eq!(
            paragraph_run_toggles(&document.model()),
            vec![
                ("A\u{1F600}", [true, false, false, false, false, false]),
                ("Tail", [false, true, false, false, false, false]),
            ]
        );
    }

    #[test]
    fn legacy_doc_pcd_prm0_stays_aligned_across_story_regions() {
        let bytes = synth_doc_with_ccp_and_tables(
            "Main\r",
            "Note\r",
            0x00C1,
            0,
            0,
            [5, 5, 0, 0, 0, 0],
            SyntheticDocTables {
                piece_prms: [prm0(0x55, 1), prm0(0x56, 1)],
                ..SyntheticDocTables::default()
            },
        );
        let model = Document::open(&bytes).unwrap().model();

        for (kind, text, expected) in [
            (
                SourceRegionKind::Main,
                "Main",
                [true, false, false, false, false, false],
            ),
            (
                SourceRegionKind::Footnote,
                "Note",
                [false, true, false, false, false, false],
            ),
        ] {
            let region = model
                .regions
                .iter()
                .find(|region| region.kind == kind)
                .expect("synthetic source region");
            let Block::Paragraph(paragraph) = &model.blocks[region.block_start] else {
                panic!("synthetic source region must contain a paragraph");
            };
            assert_eq!(paragraph.runs.len(), 1);
            assert_eq!(paragraph.runs[0].text, text);
            assert_eq!(
                [
                    paragraph.runs[0].props.bold,
                    paragraph.runs[0].props.italic,
                    paragraph.runs[0].props.strike,
                    paragraph.runs[0].props.small_caps,
                    paragraph.runs[0].props.caps,
                    paragraph.runs[0].props.hidden,
                ],
                expected
            );
        }
    }

    #[test]
    fn legacy_doc_pcd_prm0_overrides_chpx_but_preserves_other_properties() {
        let bold = [SyntheticChpxRun {
            cp_lim: 4,
            grpprl: vec![0x35, 0x08, 1],
        }];

        let explicit_off = legacy_pcd_prm0_doc("Off\r", "", [prm0(0x55, 0), 0], Some(&bold));
        assert_eq!(
            paragraph_run_toggles(&Document::open(&explicit_off).unwrap().model()),
            vec![("Off", [false; 6])]
        );

        let additive = legacy_pcd_prm0_doc("Add\r", "", [prm0(0x56, 1), 0], Some(&bold));
        assert_eq!(
            paragraph_run_toggles(&Document::open(&additive).unwrap().model()),
            vec![("Add", [true, true, false, false, false, false])]
        );
    }

    #[test]
    fn unsupported_pcd_prm_values_leave_chpx_formatting_unchanged() {
        let bold = [SyntheticChpxRun {
            cp_lim: 5,
            grpprl: vec![0x35, 0x08, 1],
        }];
        for raw_prm in [
            0,
            1,
            prm0(0x54, 1),
            prm0(0x55, 0x80),
            prm0(0x55, 0x81),
            prm0(0x55, 2),
        ] {
            let bytes = legacy_pcd_prm0_doc("Keep\r", "", [raw_prm, 0], Some(&bold));
            assert_eq!(
                paragraph_run_toggles(&Document::open(&bytes).unwrap().model()),
                vec![("Keep", [true, false, false, false, false, false])],
                "unsupported raw PRM 0x{raw_prm:04X} changed CHPX formatting"
            );
        }
    }

    #[test]
    fn equal_effective_pcd_prm0_properties_coalesce_across_piece_boundaries() {
        let bytes =
            legacy_pcd_prm0_doc("A\u{1F600}", "Tail\r", [prm0(0x55, 1), prm0(0x55, 1)], None);
        assert_eq!(
            paragraph_run_toggles(&Document::open(&bytes).unwrap().model()),
            vec![("A\u{1F600}Tail", [true, false, false, false, false, false])]
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_pcd_prm0_character_toggles_roundtrip_to_docx() {
        for (isprm, expected) in [
            (0x55, [true, false, false, false, false, false]),
            (0x56, [false, true, false, false, false, false]),
            (0x57, [false, false, true, false, false, false]),
            (0x5A, [false, false, false, true, false, false]),
            (0x5B, [false, false, false, false, true, false]),
            (0x5C, [false, false, false, false, false, true]),
        ] {
            let legacy =
                Document::open(&legacy_pcd_prm0_doc("X\r", "", [prm0(isprm, 1), 0], None)).unwrap();
            let reopened = Document::open(&legacy.to_docx()).unwrap();

            assert_eq!(
                paragraph_run_toggles(&reopened.model()),
                vec![("X", expected)],
                "literal Prm0 isprm 0x{isprm:02X} did not survive DOCX reopen"
            );
        }
    }

    fn legacy_chpx_highlight_doc() -> Vec<u8> {
        let text = "YellowPlainDark";
        let runs = [
            SyntheticChpxRun {
                cp_lim: 6,
                grpprl: vec![0x0C, 0x2A, 7],
            },
            SyntheticChpxRun {
                cp_lim: 11,
                grpprl: vec![0x0C, 0x2A, 0],
            },
            SyntheticChpxRun {
                cp_lim: 15,
                grpprl: vec![0x0C, 0x2A, 14],
            },
        ];
        synth_doc_with_ccp_and_tables(
            text,
            "",
            0x00C1,
            0,
            0,
            [text.encode_utf16().count() as u32, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                chpx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn paragraph_run_highlights(model: &DocModel) -> Vec<(&str, Option<&str>)> {
        let Block::Paragraph(paragraph) = &model.blocks[0] else {
            panic!("synthetic legacy block must be a paragraph");
        };
        paragraph
            .runs
            .iter()
            .map(|run| (run.text.as_str(), run.props.highlight.as_deref()))
            .collect()
    }

    #[test]
    fn opened_legacy_doc_preserves_chpx_text_highlighting() {
        let document = Document::open(&legacy_chpx_highlight_doc()).unwrap();
        let model = document.model();

        assert_eq!(
            paragraph_run_highlights(&model),
            vec![
                ("Yellow", Some("yellow")),
                ("Plain", None),
                ("Dark", Some("darkYellow")),
            ]
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_chpx_text_highlighting_roundtrips_to_docx() {
        let legacy = Document::open(&legacy_chpx_highlight_doc()).unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();
        let model = reopened.model();

        assert_eq!(
            paragraph_run_highlights(&model),
            vec![
                ("Yellow", Some("yellow")),
                ("Plain", None),
                ("Dark", Some("darkYellow")),
            ]
        );
    }

    fn legacy_chpx_vertical_align_doc() -> Vec<u8> {
        let text = "SuperBaseSub";
        let runs = [
            SyntheticChpxRun {
                cp_lim: 5,
                grpprl: vec![0x48, 0x2A, 1],
            },
            SyntheticChpxRun {
                cp_lim: 9,
                grpprl: vec![0x48, 0x2A, 0],
            },
            SyntheticChpxRun {
                cp_lim: 12,
                grpprl: vec![0x48, 0x2A, 2],
            },
        ];
        synth_doc_with_ccp_and_tables(
            text,
            "",
            0x00C1,
            0,
            0,
            [text.encode_utf16().count() as u32, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                chpx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn paragraph_run_vertical_alignments(model: &DocModel) -> Vec<(&str, VertAlign)> {
        let Block::Paragraph(paragraph) = &model.blocks[0] else {
            panic!("synthetic legacy block must be a paragraph");
        };
        paragraph
            .runs
            .iter()
            .map(|run| (run.text.as_str(), run.props.vert_align))
            .collect()
    }

    #[test]
    fn opened_legacy_doc_preserves_chpx_vertical_alignment() {
        let document = Document::open(&legacy_chpx_vertical_align_doc()).unwrap();
        let model = document.model();

        assert_eq!(
            paragraph_run_vertical_alignments(&model),
            vec![
                ("Super", VertAlign::Super),
                ("Base", VertAlign::Baseline),
                ("Sub", VertAlign::Sub),
            ]
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_chpx_vertical_alignment_roundtrips_to_docx() {
        let legacy = Document::open(&legacy_chpx_vertical_align_doc()).unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();
        let model = reopened.model();

        assert_eq!(
            paragraph_run_vertical_alignments(&model),
            vec![
                ("Super", VertAlign::Super),
                ("Base", VertAlign::Baseline),
                ("Sub", VertAlign::Sub),
            ]
        );
    }

    fn legacy_chpx_capitalization_doc() -> Vec<u8> {
        let text = "SmallPlainCapsBoth";
        let runs = [
            SyntheticChpxRun {
                cp_lim: 5,
                grpprl: vec![0x3A, 0x08, 1],
            },
            SyntheticChpxRun {
                cp_lim: 10,
                grpprl: vec![0x3A, 0x08, 0, 0x3B, 0x08, 0],
            },
            SyntheticChpxRun {
                cp_lim: 14,
                grpprl: vec![0x3B, 0x08, 1],
            },
            SyntheticChpxRun {
                cp_lim: 18,
                grpprl: vec![0x3A, 0x08, 1, 0x3B, 0x08, 1],
            },
        ];
        synth_doc_with_ccp_and_tables(
            text,
            "",
            0x00C1,
            0,
            0,
            [text.encode_utf16().count() as u32, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                chpx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn paragraph_run_capitalization(model: &DocModel) -> Vec<(&str, bool, bool)> {
        let Block::Paragraph(paragraph) = &model.blocks[0] else {
            panic!("synthetic legacy block must be a paragraph");
        };
        paragraph
            .runs
            .iter()
            .map(|run| (run.text.as_str(), run.props.small_caps, run.props.caps))
            .collect()
    }

    #[test]
    fn opened_legacy_doc_preserves_chpx_capitalization() {
        let document = Document::open(&legacy_chpx_capitalization_doc()).unwrap();
        let model = document.model();

        assert_eq!(
            paragraph_run_capitalization(&model),
            vec![
                ("Small", true, false),
                ("Plain", false, false),
                ("Caps", false, true),
                ("Both", true, true),
            ]
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_chpx_capitalization_roundtrips_to_docx() {
        let legacy = Document::open(&legacy_chpx_capitalization_doc()).unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();
        let model = reopened.model();

        assert_eq!(
            paragraph_run_capitalization(&model),
            vec![
                ("Small", true, false),
                ("Plain", false, false),
                ("Caps", false, true),
                ("Both", true, true),
            ]
        );
    }

    fn legacy_chpx_run_rtl_doc() -> Vec<u8> {
        let text = "RtlPlainRtl";
        let runs = [
            SyntheticChpxRun {
                cp_lim: 3,
                grpprl: vec![0x5A, 0x08, 1],
            },
            SyntheticChpxRun {
                cp_lim: 8,
                grpprl: vec![0x5A, 0x08, 0],
            },
            SyntheticChpxRun {
                cp_lim: 11,
                grpprl: vec![0x5A, 0x08, 1],
            },
        ];
        synth_doc_with_ccp_and_tables(
            text,
            "",
            0x00C1,
            0,
            0,
            [text.encode_utf16().count() as u32, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                chpx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn paragraph_run_directions(model: &DocModel) -> Vec<(&str, bool)> {
        let Block::Paragraph(paragraph) = &model.blocks[0] else {
            panic!("synthetic legacy block must be a paragraph");
        };
        paragraph
            .runs
            .iter()
            .map(|run| (run.text.as_str(), run.props.rtl))
            .collect()
    }

    #[test]
    fn opened_legacy_doc_preserves_chpx_run_rtl() {
        let document = Document::open(&legacy_chpx_run_rtl_doc()).unwrap();

        assert_eq!(
            paragraph_run_directions(&document.model()),
            vec![("Rtl", true), ("Plain", false), ("Rtl", true)]
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_chpx_run_rtl_roundtrips_to_docx() {
        let legacy = Document::open(&legacy_chpx_run_rtl_doc()).unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();

        assert_eq!(
            paragraph_run_directions(&reopened.model()),
            vec![("Rtl", true), ("Plain", false), ("Rtl", true)]
        );
    }

    fn legacy_paragraph_bidi_doc() -> Vec<u8> {
        let paragraphs = [
            ("RtlNoDirectJc", vec![0x41, 0x24, 0x01]),
            ("RtlPhysicalLeft", vec![0x41, 0x24, 0x01, 0x03, 0x24, 0x00]),
            ("RtlLogicalEnd", vec![0x41, 0x24, 0x01, 0x61, 0x24, 0x02]),
            ("LtrLogicalStart", vec![0x41, 0x24, 0x00, 0x61, 0x24, 0x00]),
            ("RtlLogicalStart", vec![0x41, 0x24, 0x01, 0x61, 0x24, 0x00]),
            ("RtlCenter", vec![0x41, 0x24, 0x01, 0x61, 0x24, 0x01]),
            ("RtlJustify", vec![0x41, 0x24, 0x01, 0x61, 0x24, 0x07]),
            ("RtlIndented", vec![0x41, 0x24, 0x01, 0x61, 0x24, 0x06]),
            ("LtrLogicalEnd", vec![0x41, 0x24, 0x00, 0x61, 0x24, 0x02]),
        ];
        let mut text = String::new();
        let mut runs = Vec::new();
        for (label, grpprl) in paragraphs {
            text.push_str(label);
            text.push('\r');
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl,
            });
        }
        let text_end = text.encode_utf16().count() as u32;
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn paragraph_layouts(model: &DocModel) -> Vec<(String, bool, Align)> {
        model
            .blocks
            .iter()
            .map(|block| {
                let Block::Paragraph(paragraph) = block else {
                    panic!("synthetic legacy block must be a paragraph");
                };
                (
                    paragraph.text(),
                    paragraph.props.bidi,
                    paragraph.props.align,
                )
            })
            .collect()
    }

    fn expected_legacy_paragraph_layouts() -> Vec<(String, bool, Align)> {
        vec![
            ("RtlNoDirectJc".to_string(), true, Align::Right),
            ("RtlPhysicalLeft".to_string(), true, Align::Left),
            ("RtlLogicalEnd".to_string(), true, Align::Left),
            ("LtrLogicalStart".to_string(), false, Align::Left),
            ("RtlLogicalStart".to_string(), true, Align::Right),
            ("RtlCenter".to_string(), true, Align::Center),
            ("RtlJustify".to_string(), true, Align::Justify),
            ("RtlIndented".to_string(), true, Align::Right),
            ("LtrLogicalEnd".to_string(), false, Align::Right),
        ]
    }

    fn legacy_paragraph_indent_doc() -> Vec<u8> {
        let paragraphs = [
            (
                "LtrFirst",
                false,
                [(0x845Eu16, 720i16), (0x845D, 1440), (0x465F, 120)],
                240i16,
                false,
                false,
            ),
            (
                "RtlFirst",
                true,
                [(0x845E, 720), (0x845D, 1440), (0x465F, 120)],
                240,
                false,
                false,
            ),
            (
                "RtlHanging",
                true,
                [(0x845E, 720), (0x845D, 360), (0x465F, 0)],
                -360,
                false,
                false,
            ),
            (
                "LastValid",
                false,
                [(0x845E, 400), (0x845E, 32000), (0x465F, 0)],
                0,
                true,
                false,
            ),
            (
                "RtlLate",
                true,
                [(0x845E, 720), (0x845D, 1440), (0x465F, 120)],
                240,
                false,
                true,
            ),
        ];
        let mut text = String::new();
        let mut runs = Vec::new();
        for (label, bidi, indents, first_line, truncate_suffix, bidi_after) in paragraphs {
            text.push_str(label);
            text.push('\r');
            let mut grpprl = Vec::new();
            if !bidi_after {
                grpprl.extend_from_slice(&[0x41, 0x24, u8::from(bidi)]);
            }
            for (sprm, value) in indents {
                grpprl.extend_from_slice(&sprm.to_le_bytes());
                grpprl.extend_from_slice(&value.to_le_bytes());
            }
            grpprl.extend_from_slice(&0x8460u16.to_le_bytes());
            grpprl.extend_from_slice(&first_line.to_le_bytes());
            if truncate_suffix {
                grpprl.extend_from_slice(&0x845Du16.to_le_bytes());
            }
            if bidi_after {
                grpprl.extend_from_slice(&[0x41, 0x24, u8::from(bidi)]);
            }
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl,
            });
        }
        for (label, grpprl) in [
            (
                "StyleRtl",
                vec![
                    0x00, 0x46, 0x01, 0x00, // paragraph style 1 supplies RTL
                    0x5E, 0x84, 0xE8, 0x03, // direct logical left = 1000
                    0x5D, 0x84, 0x58, 0x02, // direct logical right = 600
                    0x5F, 0x46, 0x78, 0x00, // direct nest = 120
                    0x60, 0x84, 0xF0, 0x00, // direct first line = 240
                ],
            ),
            (
                "StyleNestOnly",
                vec![
                    0x00, 0x46, 0x01, 0x00, // style supplies direction and indent base
                    0x5F, 0x46, 0x78, 0x00, // nest without a direct left base
                ],
            ),
            (
                "StyleOnly",
                vec![
                    0x00, 0x46, 0x01, 0x00, // style supplies all paragraph properties
                ],
            ),
            (
                "StyleDirectLtr",
                vec![
                    0x00, 0x46, 0x01, 0x00, // style supplies RTL indents
                    0x41, 0x24, 0x00, // final direct direction is LTR
                ],
            ),
        ] {
            text.push_str(label);
            text.push('\r');
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl,
            });
        }
        let text_end = text.encode_utf16().count() as u32;
        let stylesheet = synthetic_paragraph_stylesheet_grpprl(&[
            0x41, 0x24, 0x01, // style RTL
            0x5E, 0x84, 0xD0, 0x02, // style logical left = 720
            0x5D, 0x84, 0xA0, 0x05, // style logical right = 1440
            0x60, 0x84, 0x98, 0xFE, // style hanging indent = -360
        ]);
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                stylesheet: Some(&stylesheet),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    type ParagraphIndentSignature = (
        String,
        bool,
        Option<f32>,
        Option<f32>,
        Option<f32>,
        Option<f32>,
    );

    fn paragraph_indent_signature(model: &DocModel) -> Vec<ParagraphIndentSignature> {
        model
            .blocks
            .iter()
            .map(|block| {
                let Block::Paragraph(paragraph) = block else {
                    panic!("synthetic legacy block must be a paragraph");
                };
                (
                    paragraph.text(),
                    paragraph.props.bidi,
                    paragraph.props.indent.left_pt,
                    paragraph.props.indent.right_pt,
                    paragraph.props.indent.first_line_pt,
                    paragraph.props.indent.hanging_pt,
                )
            })
            .collect()
    }

    fn expected_legacy_paragraph_indent_signature() -> Vec<ParagraphIndentSignature> {
        vec![
            (
                "LtrFirst".to_string(),
                false,
                Some(42.0),
                Some(72.0),
                Some(12.0),
                None,
            ),
            (
                "RtlFirst".to_string(),
                true,
                Some(72.0),
                Some(42.0),
                Some(12.0),
                None,
            ),
            (
                "RtlHanging".to_string(),
                true,
                Some(18.0),
                Some(36.0),
                None,
                Some(18.0),
            ),
            ("LastValid".to_string(), false, Some(20.0), None, None, None),
            (
                "RtlLate".to_string(),
                true,
                Some(72.0),
                Some(42.0),
                Some(12.0),
                None,
            ),
            (
                "StyleRtl".to_string(),
                true,
                Some(30.0),
                Some(56.0),
                Some(12.0),
                None,
            ),
            (
                "StyleNestOnly".to_string(),
                true,
                Some(72.0),
                Some(42.0),
                None,
                Some(18.0),
            ),
            (
                "StyleOnly".to_string(),
                true,
                Some(72.0),
                Some(36.0),
                None,
                Some(18.0),
            ),
            (
                "StyleDirectLtr".to_string(),
                false,
                Some(36.0),
                Some(72.0),
                None,
                Some(18.0),
            ),
        ]
    }

    #[test]
    fn opened_legacy_doc_preserves_modern_logical_paragraph_indents() {
        let document = Document::open(&legacy_paragraph_indent_doc()).unwrap();

        assert_eq!(
            paragraph_indent_signature(&document.model()),
            expected_legacy_paragraph_indent_signature()
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_modern_logical_paragraph_indents_roundtrip_to_docx() {
        let legacy = Document::open(&legacy_paragraph_indent_doc()).unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();

        assert_eq!(
            paragraph_indent_signature(&reopened.model()),
            expected_legacy_paragraph_indent_signature()
        );
    }

    fn push_paragraph_spacing_twips(grpprl: &mut Vec<u8>, sprm: u16, value: u16) {
        grpprl.extend_from_slice(&sprm.to_le_bytes());
        grpprl.extend_from_slice(&value.to_le_bytes());
    }

    fn push_paragraph_line_spacing(grpprl: &mut Vec<u8>, dya_line: u16, multiple: u16) {
        grpprl.extend_from_slice(&0x6412u16.to_le_bytes());
        grpprl.extend_from_slice(&dya_line.to_le_bytes());
        grpprl.extend_from_slice(&multiple.to_le_bytes());
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn push_legacy_tab_change(
        grpprl: &mut Vec<u8>,
        sprm: u16,
        deletions: &[(i16, Option<u16>)],
        additions: &[(i16, u8)],
    ) {
        let mut operand = Vec::new();
        operand.push(deletions.len() as u8);
        for &(position, _) in deletions {
            operand.extend_from_slice(&position.to_le_bytes());
        }
        if sprm == 0xC615 {
            for &(_, close) in deletions {
                operand.extend_from_slice(&close.unwrap_or(26).to_le_bytes());
            }
        }
        operand.push(additions.len() as u8);
        for &(position, _) in additions {
            operand.extend_from_slice(&position.to_le_bytes());
        }
        for &(_, descriptor) in additions {
            operand.push(descriptor);
        }
        grpprl.extend_from_slice(&sprm.to_le_bytes());
        grpprl.push(operand.len() as u8);
        grpprl.extend_from_slice(&operand);
    }

    #[cfg(feature = "docx")]
    fn legacy_body_and_table_cell_tabs_doc() -> Vec<u8> {
        legacy_body_and_table_cell_tabs_doc_with_tabs(true)
    }

    #[cfg(feature = "docx")]
    fn legacy_body_and_table_cell_tabs_doc_with_tabs(include_tabs: bool) -> Vec<u8> {
        let mut style_tabs = Vec::new();
        if include_tabs {
            push_legacy_tab_change(&mut style_tabs, 0xC60D, &[], &[(720, 0x09), (1440, 0x12)]);
        }
        let stylesheet = synthetic_paragraph_stylesheet_grpprl(&style_tabs);

        let mut body_grpprl = vec![0x00, 0x46, 0x01, 0x00];
        if include_tabs {
            push_legacy_tab_change(
                &mut body_grpprl,
                0xC615,
                &[(720, Some(26))],
                &[(1080, 0x1B)],
            );
        }

        let mut cell_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
            0x00, 0x46, 0x01, 0x00, // sprmPIstd = tabbed style
        ];
        if include_tabs {
            push_legacy_tab_change(&mut cell_grpprl, 0xC60D, &[(1440, None)], &[(1800, 0x24)]);
        }

        let text = "Body\tTail\rCell\tTail\u{7}\u{7}";
        let body_end = "Body\tTail\r".encode_utf16().count() as u32;
        let cell_end = "Body\tTail\rCell\tTail\u{7}".encode_utf16().count() as u32;
        let row_end = text.encode_utf16().count() as u32;
        let mut row_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
            0x17, 0x24, 0x01, // sprmPFTtp
            0x08, 0xD6, 0x1A, 0x00, // sprmTDefTable, cb=26
            0x01, // one cell
            0x00, 0x00, 0xD0, 0x07, // cell boundaries 0..2000 twips
        ];
        row_grpprl.extend_from_slice(&[0u8; 20]);
        let runs = [
            SyntheticPapxRun {
                cp_lim: body_end,
                grpprl: body_grpprl,
            },
            SyntheticPapxRun {
                cp_lim: cell_end,
                grpprl: cell_grpprl,
            },
            SyntheticPapxRun {
                cp_lim: row_end,
                grpprl: row_grpprl,
            },
        ];

        synth_doc_with_ccp_and_tables(
            text,
            "",
            0x00C1,
            0,
            0,
            [row_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                stylesheet: Some(&stylesheet),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn legacy_paragraph_spacing_doc() -> Vec<u8> {
        const SPRM_P_DYA_BEFORE: u16 = 0xA413;
        const SPRM_P_DYA_AFTER: u16 = 0xA414;

        let mut style_grpprl = Vec::new();
        push_paragraph_spacing_twips(&mut style_grpprl, SPRM_P_DYA_BEFORE, 360);
        push_paragraph_spacing_twips(&mut style_grpprl, SPRM_P_DYA_AFTER, 180);
        push_paragraph_line_spacing(&mut style_grpprl, 360, 1);
        let stylesheet = synthetic_paragraph_stylesheet_grpprl(&style_grpprl);

        let mut paragraphs = Vec::new();
        paragraphs.push(("Default", Vec::new()));

        let mut direct = Vec::new();
        push_paragraph_spacing_twips(&mut direct, SPRM_P_DYA_BEFORE, 240);
        push_paragraph_spacing_twips(&mut direct, SPRM_P_DYA_AFTER, 120);
        push_paragraph_line_spacing(&mut direct, 360, 1);
        paragraphs.push(("Direct", direct));

        let mut last_valid = Vec::new();
        push_paragraph_spacing_twips(&mut last_valid, SPRM_P_DYA_BEFORE, 200);
        push_paragraph_spacing_twips(&mut last_valid, SPRM_P_DYA_BEFORE, 32_000);
        push_paragraph_spacing_twips(&mut last_valid, SPRM_P_DYA_AFTER, 100);
        push_paragraph_spacing_twips(&mut last_valid, SPRM_P_DYA_AFTER, 32_000);
        push_paragraph_line_spacing(&mut last_valid, 480, 1);
        push_paragraph_line_spacing(&mut last_valid, 32_000, 1);
        paragraphs.push(("LastValid", last_valid));

        let mut at_least = Vec::new();
        push_paragraph_line_spacing(&mut at_least, 240, 0);
        paragraphs.push(("AtLeast", at_least));

        paragraphs.push(("StyleOnly", vec![0x00, 0x46, 0x01, 0x00]));

        let mut style_direct = vec![0x00, 0x46, 0x01, 0x00];
        push_paragraph_spacing_twips(&mut style_direct, SPRM_P_DYA_AFTER, 0);
        push_paragraph_line_spacing(&mut style_direct, 480, 1);
        paragraphs.push(("StyleDirect", style_direct));

        let mut style_at_least = vec![0x00, 0x46, 0x01, 0x00];
        push_paragraph_line_spacing(&mut style_at_least, 240, 0);
        paragraphs.push(("StyleAtLeast", style_at_least));

        let mut style_exact = vec![0x00, 0x46, 0x01, 0x00];
        push_paragraph_line_spacing(&mut style_exact, 0xFF10, 0);
        paragraphs.push(("StyleExact", style_exact));

        let mut style_zero_line = vec![0x00, 0x46, 0x01, 0x00];
        push_paragraph_line_spacing(&mut style_zero_line, 0, 1);
        paragraphs.push(("StyleZeroLine", style_zero_line));

        let mut style_invalid_line = vec![0x00, 0x46, 0x01, 0x00];
        push_paragraph_line_spacing(&mut style_invalid_line, 480, 2);
        paragraphs.push(("StyleInvalidLine", style_invalid_line));

        let mut style_reset = Vec::new();
        push_paragraph_spacing_twips(&mut style_reset, SPRM_P_DYA_BEFORE, 240);
        push_paragraph_line_spacing(&mut style_reset, 480, 1);
        style_reset.extend_from_slice(&[0x00, 0x46, 0x01, 0x00]);
        push_paragraph_spacing_twips(&mut style_reset, SPRM_P_DYA_AFTER, 60);
        paragraphs.push(("StyleReset", style_reset));

        let mut valid_prefix = Vec::new();
        push_paragraph_spacing_twips(&mut valid_prefix, SPRM_P_DYA_BEFORE, 300);
        valid_prefix.extend_from_slice(&SPRM_P_DYA_AFTER.to_le_bytes());
        paragraphs.push(("ValidPrefix", valid_prefix));

        let mut text = String::new();
        let mut runs = Vec::new();
        for (label, grpprl) in paragraphs {
            text.push_str(label);
            text.push('\r');
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl,
            });
        }
        let text_end = text.encode_utf16().count() as u32;
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                stylesheet: Some(&stylesheet),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(feature = "docx")]
    fn legacy_paragraph_pagination_conversion_doc() -> Vec<u8> {
        const KEEP_LINES: u16 = 0x2405;
        const KEEP_NEXT: u16 = 0x2406;
        const PAGE_BREAK_BEFORE: u16 = 0x2407;
        const WIDOW_CONTROL: u16 = 0x2431;

        let mut style_grpprl = Vec::new();
        for (sprm, value) in [
            (KEEP_LINES, 1),
            (KEEP_NEXT, 1),
            (PAGE_BREAK_BEFORE, 1),
            (WIDOW_CONTROL, 0),
        ] {
            style_grpprl.extend_from_slice(&sprm.to_le_bytes());
            style_grpprl.push(value);
        }
        let stylesheet = synthetic_paragraph_stylesheet_grpprl(&style_grpprl);

        let direct = |properties: &[(u16, u8)]| {
            let mut grpprl = Vec::with_capacity(properties.len() * 3);
            for &(sprm, value) in properties {
                grpprl.extend_from_slice(&sprm.to_le_bytes());
                grpprl.push(value);
            }
            grpprl
        };
        let mut style_cleared = vec![0x00, 0x46, 0x01, 0x00];
        style_cleared.extend(direct(&[
            (KEEP_LINES, 0),
            (KEEP_NEXT, 0),
            (PAGE_BREAK_BEFORE, 0),
            (WIDOW_CONTROL, 1),
        ]));
        let paragraphs = [
            ("Default", Vec::new()),
            ("KeepNext", direct(&[(KEEP_NEXT, 1)])),
            ("KeepLines", direct(&[(KEEP_LINES, 1)])),
            ("WidowOff", direct(&[(WIDOW_CONTROL, 0)])),
            ("StyleAll", vec![0x00, 0x46, 0x01, 0x00]),
            ("StyleCleared", style_cleared),
        ];
        let mut text = String::new();
        let mut runs = Vec::new();
        for (label, grpprl) in paragraphs {
            text.push_str(label);
            text.push('\r');
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl,
            });
        }
        let text_end = text.encode_utf16().count() as u32;
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                stylesheet: Some(&stylesheet),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    type ParagraphSpacingSignature = (String, Option<f32>, Option<f32>, Option<f32>);

    fn paragraph_spacing_signature(model: &DocModel) -> Vec<ParagraphSpacingSignature> {
        model
            .blocks
            .iter()
            .map(|block| {
                let Block::Paragraph(paragraph) = block else {
                    panic!("synthetic legacy block must be a paragraph");
                };
                (
                    paragraph.text(),
                    paragraph.props.spacing.before_pt,
                    paragraph.props.spacing.after_pt,
                    paragraph.props.spacing.line_pct,
                )
            })
            .collect()
    }

    fn expected_legacy_paragraph_spacing_signature() -> Vec<ParagraphSpacingSignature> {
        vec![
            ("Default".to_string(), Some(0.0), Some(0.0), Some(1.0)),
            ("Direct".to_string(), Some(12.0), Some(6.0), Some(1.5)),
            ("LastValid".to_string(), Some(10.0), Some(5.0), Some(2.0)),
            ("AtLeast".to_string(), Some(0.0), Some(0.0), None),
            ("StyleOnly".to_string(), Some(18.0), Some(9.0), Some(1.5)),
            ("StyleDirect".to_string(), Some(18.0), Some(0.0), Some(2.0)),
            ("StyleAtLeast".to_string(), Some(18.0), Some(9.0), None),
            ("StyleExact".to_string(), Some(18.0), Some(9.0), None),
            ("StyleZeroLine".to_string(), Some(18.0), Some(9.0), None),
            (
                "StyleInvalidLine".to_string(),
                Some(18.0),
                Some(9.0),
                Some(1.5),
            ),
            ("StyleReset".to_string(), Some(18.0), Some(3.0), Some(1.5)),
            ("ValidPrefix".to_string(), Some(15.0), Some(0.0), Some(1.0)),
        ]
    }

    #[test]
    fn opened_legacy_doc_preserves_bounded_paragraph_spacing() {
        let document = Document::open(&legacy_paragraph_spacing_doc()).unwrap();

        assert_eq!(
            paragraph_spacing_signature(&document.model()),
            expected_legacy_paragraph_spacing_signature()
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_retains_absolute_line_spacing_render_hints() {
        let document = Document::open(&legacy_paragraph_spacing_doc()).unwrap();
        let Backend::Doc(state) = &document.backend else {
            panic!("synthetic legacy document must use the DOC backend");
        };
        let hints = legacy_build_output_from_doc_state(state).line_spacing_hints;

        assert_eq!(hints.len(), document.model().blocks.len());
        assert_eq!(hints[3], Some(crate::model::LineSpacingHint::AtLeast(12.0)));
        assert_eq!(hints[6], Some(crate::model::LineSpacingHint::AtLeast(12.0)));
        assert_eq!(hints[7], Some(crate::model::LineSpacingHint::Exact(12.0)));
        assert_eq!(hints[8], None, "zero multiplier only clears inheritance");
        assert_eq!(hints[9], None, "invalid direct LSPD keeps style multiplier");
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_bounded_paragraph_spacing_roundtrips_to_docx() {
        let legacy = Document::open(&legacy_paragraph_spacing_doc()).unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();

        assert_eq!(
            paragraph_spacing_signature(&reopened.model()),
            expected_legacy_paragraph_spacing_signature()
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_absolute_line_spacing_roundtrips_to_docx() {
        let legacy = Document::open(&legacy_paragraph_spacing_doc()).unwrap();
        let converted = legacy.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");

        let default = docx_paragraph_with_text(&document_xml, "Default");
        assert!(default.contains(r#"w:line="240" w:lineRule="auto""#));
        let at_least = docx_paragraph_with_text(&document_xml, "AtLeast");
        assert!(at_least.contains(
            r#"<w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="atLeast"/>"#
        ));
        let style_at_least = docx_paragraph_with_text(&document_xml, "StyleAtLeast");
        assert!(style_at_least.contains(
            r#"<w:spacing w:before="360" w:after="180" w:line="240" w:lineRule="atLeast"/>"#
        ));
        let style_exact = docx_paragraph_with_text(&document_xml, "StyleExact");
        assert!(style_exact.contains(r#"w:line="240" w:lineRule="exact""#));
        let zero = docx_paragraph_with_text(&document_xml, "StyleZeroLine");
        assert!(!zero.contains("w:line="), "{zero}");
        let invalid = docx_paragraph_with_text(&document_xml, "StyleInvalidLine");
        assert!(invalid.contains(r#"w:line="360" w:lineRule="auto""#));
        assert_eq!(converted, legacy.to_docx());

        let model_only_xml = docx_part(&write_docx(&legacy.model()), "word/document.xml");
        assert!(!model_only_xml.contains(r#"w:lineRule="atLeast""#));
        assert!(!model_only_xml.contains(r#"w:lineRule="exact""#));

        let reopened = Document::open(&converted).unwrap();
        assert_eq!(
            paragraph_spacing_signature(&reopened.model()),
            expected_legacy_paragraph_spacing_signature()
        );
        #[cfg(feature = "render")]
        {
            let Backend::Docx(state) = &reopened.backend else {
                panic!("converted document must use the DOCX backend");
            };
            assert_eq!(
                state.line_spacing_hints[3],
                Some(crate::model::LineSpacingHint::AtLeast(12.0))
            );
            assert_eq!(
                state.line_spacing_hints[6],
                Some(crate::model::LineSpacingHint::AtLeast(12.0))
            );
            assert_eq!(
                state.line_spacing_hints[7],
                Some(crate::model::LineSpacingHint::Exact(12.0))
            );
            assert_eq!(state.line_spacing_hints[8], None);
            assert_eq!(state.line_spacing_hints[9], None);
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_body_and_table_cell_tabs_roundtrip_to_docx() {
        let legacy = Document::open(&legacy_body_and_table_cell_tabs_doc()).unwrap();
        let control =
            Document::open(&legacy_body_and_table_cell_tabs_doc_with_tabs(false)).unwrap();
        assert_eq!(
            legacy.model(),
            control.model(),
            "legacy custom tabs must remain outside the public model"
        );
        let converted = legacy.to_docx();
        assert_eq!(converted, legacy.to_docx());
        assert_ne!(converted, control.to_docx());
        let document_xml = docx_part(&converted, "word/document.xml");

        let body = docx_paragraph_with_text(&document_xml, "Body");
        assert!(
            body.contains(concat!(
                r#"<w:tabs><w:tab w:val="decimal" w:pos="1080" w:leader="underscore"/>"#,
                r#"<w:tab w:val="right" w:pos="1440" w:leader="hyphen"/></w:tabs>"#,
            )),
            "{body}"
        );
        let cell = docx_paragraph_with_text(&document_xml, "Cell");
        assert!(
            cell.contains(concat!(
                r#"<w:tabs><w:tab w:val="center" w:pos="720" w:leader="dot"/>"#,
                r#"<w:tab w:val="bar" w:pos="1800"/></w:tabs>"#,
            )),
            "{cell}"
        );

        let model_only_xml = docx_part(&write_docx(&legacy.model()), "word/document.xml");
        assert!(!model_only_xml.contains("<w:tabs>"));

        let reopened = Document::open(&converted).unwrap();
        let reopened_xml = docx_part(&reopened.to_docx(), "word/document.xml");
        assert_eq!(reopened_xml.matches("<w:tabs>").count(), 2);
        assert!(reopened_xml.contains(r#"w:val="decimal" w:pos="1080""#));
        assert!(reopened_xml.contains(r#"w:val="bar" w:pos="1800""#));
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_legacy_doc_body_and_table_cell_tabs_change_preview_deterministically() {
        let legacy = Document::open(&legacy_body_and_table_cell_tabs_doc()).unwrap();
        let control =
            Document::open(&legacy_body_and_table_cell_tabs_doc_with_tabs(false)).unwrap();
        assert_eq!(legacy.model(), control.model());

        let Backend::Doc(state) = &legacy.backend else {
            panic!("synthetic legacy document must use the DOC backend");
        };
        let assembled = legacy_build_output_from_doc_state(state);
        assert_eq!(
            assembled.tab_stops[0],
            vec![
                crate::model::TabStop {
                    position_pt: 54.0,
                    alignment: crate::model::TabAlignment::Decimal,
                    leader: crate::model::TabLeader::Underscore,
                },
                crate::model::TabStop {
                    position_pt: 72.0,
                    alignment: crate::model::TabAlignment::Right,
                    leader: crate::model::TabLeader::Hyphen,
                },
            ]
        );
        assert_eq!(
            assembled.table_cell_tab_stops[1][0][0][0],
            vec![
                crate::model::TabStop {
                    position_pt: 36.0,
                    alignment: crate::model::TabAlignment::Center,
                    leader: crate::model::TabLeader::Dot,
                },
                crate::model::TabStop {
                    position_pt: 90.0,
                    alignment: crate::model::TabAlignment::Bar,
                    leader: crate::model::TabLeader::None,
                },
            ]
        );

        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let rendered = legacy.to_pdf_with_fonts(&fonts);
        let control_rendered = control.to_pdf_with_fonts(&fonts);
        assert!(rendered.starts_with(b"%PDF-"));
        assert_ne!(rendered, control_rendered);
        assert_eq!(rendered, legacy.to_pdf_with_fonts(&fonts));
        assert_eq!(
            control_rendered,
            render_pdf_with_fonts(&legacy.model(), &fonts)
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_top_level_pagination_controls_roundtrip_to_docx() {
        let legacy = Document::open(&legacy_paragraph_pagination_conversion_doc()).unwrap();
        let converted = legacy.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");

        let default = docx_paragraph_with_text(&document_xml, "Default");
        assert!(!default.contains("<w:keepNext"), "{default}");
        assert!(!default.contains("<w:keepLines"), "{default}");
        assert!(!default.contains("<w:widowControl"), "{default}");
        let keep_next = docx_paragraph_with_text(&document_xml, "KeepNext");
        assert!(keep_next.contains("<w:keepNext/>"), "{keep_next}");
        let keep_lines = docx_paragraph_with_text(&document_xml, "KeepLines");
        assert!(keep_lines.contains("<w:keepLines/>"), "{keep_lines}");
        let widow_off = docx_paragraph_with_text(&document_xml, "WidowOff");
        assert!(
            widow_off.contains(r#"<w:widowControl w:val="0"/>"#),
            "{widow_off}"
        );
        let style_all = docx_paragraph_with_text(&document_xml, "StyleAll");
        assert!(
            style_all.contains(concat!(
                r#"<w:pPr><w:pStyle w:val="Heading1"/><w:keepNext/><w:keepLines/>"#,
                r#"<w:pageBreakBefore/><w:widowControl w:val="0"/>"#,
            )),
            "{style_all}"
        );
        let style_cleared = docx_paragraph_with_text(&document_xml, "StyleCleared");
        assert!(!style_cleared.contains("<w:keepNext"), "{style_cleared}");
        assert!(!style_cleared.contains("<w:keepLines"), "{style_cleared}");
        assert!(
            !style_cleared.contains("<w:pageBreakBefore"),
            "{style_cleared}"
        );
        assert!(
            !style_cleared.contains("<w:widowControl"),
            "{style_cleared}"
        );
        assert_eq!(converted, legacy.to_docx());

        let model_only_xml = docx_part(&write_docx(&legacy.model()), "word/document.xml");
        assert!(!model_only_xml.contains("<w:keepNext"));
        assert!(!model_only_xml.contains("<w:keepLines"));
        assert!(!model_only_xml.contains("<w:widowControl"));

        #[cfg(feature = "render")]
        {
            let reopened = Document::open(&converted).unwrap();
            let Backend::Docx(state) = &reopened.backend else {
                panic!("converted document must use the DOCX backend");
            };
            assert_eq!(
                state.pagination_hints,
                vec![
                    crate::model::PaginationHint {
                        widow_control: true,
                        ..crate::model::PaginationHint::default()
                    },
                    crate::model::PaginationHint {
                        keep_next: true,
                        widow_control: true,
                        ..crate::model::PaginationHint::default()
                    },
                    crate::model::PaginationHint {
                        keep_lines: true,
                        widow_control: true,
                        ..crate::model::PaginationHint::default()
                    },
                    crate::model::PaginationHint::default(),
                    crate::model::PaginationHint {
                        keep_next: true,
                        keep_lines: true,
                        widow_control: false,
                    },
                    crate::model::PaginationHint {
                        widow_control: true,
                        ..crate::model::PaginationHint::default()
                    },
                ]
            );
        }
    }

    #[cfg(feature = "render")]
    fn legacy_paragraph_spacing_layout_doc(
        paragraph_count: usize,
        line_spacing: Option<(u16, u16)>,
    ) -> Vec<u8> {
        let mut text = String::new();
        for index in 0..paragraph_count {
            text.push_str(&format!("line {index}\r"));
        }
        let text_end = text.encode_utf16().count() as u32;
        let mut grpprl = Vec::new();
        if let Some((line_twips, multiple)) = line_spacing {
            push_paragraph_line_spacing(&mut grpprl, line_twips, multiple);
        }
        let runs = [SyntheticPapxRun {
            cp_lim: text_end,
            grpprl,
        }];
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_paragraph_spacing_changes_preview_layout() {
        const PARAGRAPH_COUNT: usize = 40;
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let single = Document::open(&legacy_paragraph_spacing_layout_doc(PARAGRAPH_COUNT, None))
            .unwrap()
            .model();
        let double = Document::open(&legacy_paragraph_spacing_layout_doc(
            PARAGRAPH_COUNT,
            Some((480, 1)),
        ))
        .unwrap()
        .model();
        let mut unset_fallback = single.clone();
        for block in &mut unset_fallback.blocks {
            let Block::Paragraph(paragraph) = block else {
                panic!("synthetic legacy block must be a paragraph");
            };
            paragraph.props.spacing = Default::default();
        }

        let single_layout = layout_pages_with_fonts(&single, &fonts).unwrap();
        let double_layout = layout_pages_with_fonts(&double, &fonts).unwrap();
        let unset_layout = layout_pages_with_fonts(&unset_fallback, &fonts).unwrap();

        assert_eq!(single.blocks.len(), PARAGRAPH_COUNT);
        assert_eq!(
            (
                single_layout.pages,
                single_layout.block_pages[PARAGRAPH_COUNT - 1],
                double_layout.pages,
                double_layout.block_pages[PARAGRAPH_COUNT - 1],
                unset_layout.pages,
                unset_layout.block_pages[PARAGRAPH_COUNT - 1],
            ),
            (1, Some(1), 2, Some(2), 2, Some(2))
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_absolute_line_spacing_changes_preview_layout() {
        const PARAGRAPH_COUNT: usize = 40;
        const EXACT_FIVE_POINTS: u16 = 0xFF9C;
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let exact = Document::open(&legacy_paragraph_spacing_layout_doc(
            PARAGRAPH_COUNT,
            Some((EXACT_FIVE_POINTS, 0)),
        ))
        .unwrap();
        let minimum = Document::open(&legacy_paragraph_spacing_layout_doc(
            PARAGRAPH_COUNT,
            Some((800, 0)),
        ))
        .unwrap();

        assert_eq!(
            exact.model(),
            minimum.model(),
            "legacy absolute line spacing must remain outside the public model"
        );
        let exact_layout = exact.layout_pages_with_fonts(&fonts).unwrap();
        let minimum_layout = minimum.layout_pages_with_fonts(&fonts).unwrap();
        assert!(minimum_layout.pages > exact_layout.pages);

        let exact_pdf = exact.to_pdf_with_fonts(&fonts);
        let minimum_pdf = minimum.to_pdf_with_fonts(&fonts);
        assert!(exact_pdf.starts_with(b"%PDF-"));
        assert!(minimum_pdf.starts_with(b"%PDF-"));
        assert_ne!(exact_pdf, minimum_pdf);
        assert_eq!(exact_pdf, exact.to_pdf_with_fonts(&fonts));
        assert_eq!(minimum_pdf, minimum.to_pdf_with_fonts(&fonts));
    }

    fn push_legacy_paragraph_shd80(
        grpprl: &mut Vec<u8>,
        foreground: u8,
        background: u8,
        pattern: u8,
    ) {
        assert!(foreground < 32 && background < 32 && pattern < 64);
        grpprl.extend_from_slice(&0x442Du16.to_le_bytes());
        let value =
            u16::from(foreground) | (u16::from(background) << 5) | (u16::from(pattern) << 10);
        grpprl.extend_from_slice(&value.to_le_bytes());
    }

    fn push_legacy_paragraph_shd(
        grpprl: &mut Vec<u8>,
        foreground: Option<Color>,
        background: Option<Color>,
        pattern: u16,
    ) {
        grpprl.extend_from_slice(&0xC64Du16.to_le_bytes());
        grpprl.push(10);
        for color in [foreground, background] {
            if let Some(color) = color {
                grpprl.extend_from_slice(&[color.r, color.g, color.b, 0]);
            } else {
                grpprl.extend_from_slice(&[0, 0, 0, 0xFF]);
            }
        }
        grpprl.extend_from_slice(&pattern.to_le_bytes());
    }

    fn legacy_paragraph_shading_doc() -> Vec<u8> {
        let clear = Color::rgb(0x11, 0x22, 0x33);
        let solid = Color::rgb(0x44, 0x55, 0x66);
        let equal = Color::rgb(0x24, 0x68, 0xAC);

        let mut paragraphs = vec![("Default", Vec::new())];

        let mut shd80_clear = Vec::new();
        push_legacy_paragraph_shd80(&mut shd80_clear, 0, 7, 0);
        paragraphs.push(("Shd80Clear", shd80_clear));

        let mut shd80_solid = Vec::new();
        push_legacy_paragraph_shd80(&mut shd80_solid, 6, 0, 1);
        paragraphs.push(("Shd80Solid", shd80_solid));

        let mut modern_clear = Vec::new();
        push_legacy_paragraph_shd(&mut modern_clear, None, Some(clear), 0);
        paragraphs.push(("ModernClear", modern_clear));

        let mut modern_solid = Vec::new();
        push_legacy_paragraph_shd(&mut modern_solid, Some(solid), None, 1);
        paragraphs.push(("ModernSolid", modern_solid));

        let mut equal_pattern = Vec::new();
        push_legacy_paragraph_shd(&mut equal_pattern, Some(equal), Some(equal), 8);
        paragraphs.push(("EqualPattern", equal_pattern));

        let mut patterned = Vec::new();
        push_legacy_paragraph_shd80(&mut patterned, 0, 7, 0);
        push_legacy_paragraph_shd(
            &mut patterned,
            Some(Color::rgb(0x10, 0x20, 0x30)),
            Some(Color::rgb(0xA0, 0xB0, 0xC0)),
            8,
        );
        paragraphs.push(("Patterned", patterned));

        let mut automatic = Vec::new();
        push_legacy_paragraph_shd80(&mut automatic, 0, 7, 0);
        push_legacy_paragraph_shd(&mut automatic, None, None, 0);
        paragraphs.push(("Automatic", automatic));

        let mut nil = Vec::new();
        push_legacy_paragraph_shd80(&mut nil, 0, 7, 0);
        nil.extend_from_slice(&0xC64Du16.to_le_bytes());
        nil.extend_from_slice(&[10, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0]);
        paragraphs.push(("Nil", nil));

        let mut style_reset = Vec::new();
        push_legacy_paragraph_shd80(&mut style_reset, 0, 7, 0);
        style_reset.extend_from_slice(&[0x00, 0x46, 0x0A, 0x00]);
        paragraphs.push(("StyleReset", style_reset));

        let mut style_then_direct = vec![0x00, 0x46, 0x0A, 0x00];
        push_legacy_paragraph_shd(&mut style_then_direct, None, Some(clear), 0);
        paragraphs.push(("StyleThenDirect", style_then_direct));

        let mut invalid = Vec::new();
        push_legacy_paragraph_shd80(&mut invalid, 0, 7, 0);
        invalid.extend_from_slice(&0xC64Du16.to_le_bytes());
        invalid.extend_from_slice(&[10, 0, 0, 0, 1, 0x11, 0x22, 0x33, 0, 0, 0]);
        paragraphs.push(("Invalid", invalid));

        let mut truncated = Vec::new();
        push_legacy_paragraph_shd80(&mut truncated, 0, 7, 0);
        truncated.extend_from_slice(&[0x4D, 0xC6, 10, 0, 0]);
        paragraphs.push(("Truncated", truncated));

        let mut text = String::new();
        let mut runs = Vec::new();
        for (label, grpprl) in paragraphs {
            text.push_str(label);
            text.push('\r');
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl,
            });
        }
        let text_end = text.encode_utf16().count() as u32;
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn legacy_paragraph_style_shading_doc() -> Vec<u8> {
        let inherited = Color::rgb(0x18, 0x52, 0x86);
        let recovered = Color::rgb(0x24, 0x68, 0xAC);
        let direct = Color::rgb(0x44, 0x55, 0x66);

        let mut root_style = Vec::new();
        push_legacy_paragraph_shd(&mut root_style, None, Some(inherited), 0);
        let inherited_style = Vec::new();
        let mut suppressed_style = Vec::new();
        push_legacy_paragraph_shd(&mut suppressed_style, None, None, 0);
        let mut recovered_style = Vec::new();
        push_legacy_paragraph_shd(&mut recovered_style, None, Some(recovered), 0);
        let stylesheet = synthetic_paragraph_stylesheet_from_styles(&[
            (0, 0, 0x0FFF, "Normal", &root_style),
            (1, 0x0FFE, 0, "Inherited", &inherited_style),
            (2, 0x0FFE, 1, "Suppressed", &suppressed_style),
            (3, 0x0FFE, 2, "Recovered", &recovered_style),
        ]);

        let style = |istd: u16| {
            let mut grpprl = Vec::from(0x4600u16.to_le_bytes());
            grpprl.extend_from_slice(&istd.to_le_bytes());
            grpprl
        };
        let mut direct_valid = style(1);
        push_legacy_paragraph_shd(&mut direct_valid, Some(direct), None, 1);
        let mut direct_suppress = style(1);
        push_legacy_paragraph_shd(&mut direct_suppress, None, None, 0);
        let mut direct_before_style = Vec::new();
        push_legacy_paragraph_shd(&mut direct_before_style, Some(direct), None, 1);
        direct_before_style.extend_from_slice(&style(1));
        let mut direct_recovery = style(2);
        push_legacy_paragraph_shd(&mut direct_recovery, Some(direct), None, 1);
        let mut direct_invalid = style(1);
        direct_invalid.extend_from_slice(&0xC64Du16.to_le_bytes());
        direct_invalid.extend_from_slice(&[10, 0, 0, 0, 1, 0x11, 0x22, 0x33, 0, 0, 0]);
        let mut direct_truncated = style(1);
        direct_truncated.extend_from_slice(&[0x4D, 0xC6, 10, 0, 0]);

        let paragraphs = [
            ("RootStyle", Vec::new()),
            ("InheritedStyle", style(1)),
            ("SuppressedStyle", style(2)),
            ("RecoveredStyle", style(3)),
            ("DirectValid", direct_valid),
            ("DirectSuppress", direct_suppress),
            ("DirectBeforeStyle", direct_before_style),
            ("DirectRecovery", direct_recovery),
            ("DirectInvalid", direct_invalid),
            ("DirectTruncated", direct_truncated),
        ];
        let mut text = String::new();
        let mut runs = Vec::new();
        for (label, grpprl) in paragraphs {
            text.push_str(label);
            text.push('\r');
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl,
            });
        }
        let text_end = text.encode_utf16().count() as u32;

        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                stylesheet: Some(&stylesheet),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn expected_legacy_paragraph_style_shading_signature() -> Vec<(String, Option<Color>)> {
        let inherited = Some(Color::rgb(0x18, 0x52, 0x86));
        let recovered = Some(Color::rgb(0x24, 0x68, 0xAC));
        let direct = Some(Color::rgb(0x44, 0x55, 0x66));
        vec![
            ("RootStyle".to_string(), inherited),
            ("InheritedStyle".to_string(), inherited),
            ("SuppressedStyle".to_string(), None),
            ("RecoveredStyle".to_string(), recovered),
            ("DirectValid".to_string(), direct),
            ("DirectSuppress".to_string(), None),
            ("DirectBeforeStyle".to_string(), inherited),
            ("DirectRecovery".to_string(), direct),
            ("DirectInvalid".to_string(), None),
            ("DirectTruncated".to_string(), None),
        ]
    }

    fn paragraph_shading_signature(model: &DocModel) -> Vec<(String, Option<Color>)> {
        model
            .blocks
            .iter()
            .map(|block| {
                let Block::Paragraph(paragraph) = block else {
                    panic!("synthetic legacy block must be a paragraph");
                };
                (paragraph.text(), paragraph.props.shading)
            })
            .collect()
    }

    fn expected_legacy_paragraph_shading_signature() -> Vec<(String, Option<Color>)> {
        vec![
            ("Default".to_string(), None),
            ("Shd80Clear".to_string(), Some(Color::rgb(0xFF, 0xFF, 0))),
            ("Shd80Solid".to_string(), Some(Color::rgb(0xFF, 0, 0))),
            (
                "ModernClear".to_string(),
                Some(Color::rgb(0x11, 0x22, 0x33)),
            ),
            (
                "ModernSolid".to_string(),
                Some(Color::rgb(0x44, 0x55, 0x66)),
            ),
            (
                "EqualPattern".to_string(),
                Some(Color::rgb(0x24, 0x68, 0xAC)),
            ),
            ("Patterned".to_string(), None),
            ("Automatic".to_string(), None),
            ("Nil".to_string(), None),
            ("StyleReset".to_string(), None),
            (
                "StyleThenDirect".to_string(),
                Some(Color::rgb(0x11, 0x22, 0x33)),
            ),
            ("Invalid".to_string(), None),
            ("Truncated".to_string(), None),
        ]
    }

    #[test]
    fn opened_legacy_doc_preserves_bounded_direct_paragraph_shading() {
        let document = Document::open(&legacy_paragraph_shading_doc()).unwrap();

        assert_eq!(
            paragraph_shading_signature(&document.model()),
            expected_legacy_paragraph_shading_signature()
        );
    }

    #[test]
    fn opened_legacy_doc_resolves_paragraph_style_shading_before_direct_overrides() {
        let document = Document::open(&legacy_paragraph_style_shading_doc()).unwrap();

        assert_eq!(
            paragraph_shading_signature(&document.model()),
            expected_legacy_paragraph_style_shading_signature()
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_style_derived_paragraph_shading_roundtrips_to_docx() {
        let legacy = Document::open(&legacy_paragraph_style_shading_doc()).unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();

        assert_eq!(
            paragraph_shading_signature(&reopened.model()),
            expected_legacy_paragraph_style_shading_signature()
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn legacy_doc_style_derived_shading_changes_pdf_without_changing_layout() {
        let recovered = Document::open(&legacy_paragraph_style_shading_doc())
            .unwrap()
            .model();
        let mut baseline = recovered.clone();
        for block in &mut baseline.blocks {
            let Block::Paragraph(paragraph) = block else {
                panic!("synthetic legacy block must be a paragraph");
            };
            paragraph.props.shading = None;
        }

        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let recovered_pdf = render_pdf_with_fonts(&recovered, &fonts);
        let baseline_pdf = render_pdf_with_fonts(&baseline, &fonts);

        assert_ne!(recovered_pdf, baseline_pdf);
        assert_eq!(recovered_pdf, render_pdf_with_fonts(&recovered, &fonts));
        assert_eq!(
            layout_pages_with_fonts(&recovered, &fonts).unwrap(),
            layout_pages_with_fonts(&baseline, &fonts).unwrap()
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_bounded_direct_paragraph_shading_roundtrips_to_docx() {
        let legacy = Document::open(&legacy_paragraph_shading_doc()).unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();

        assert_eq!(
            paragraph_shading_signature(&reopened.model()),
            expected_legacy_paragraph_shading_signature()
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn legacy_doc_paragraph_shading_changes_pdf_without_changing_layout() {
        let recovered = Document::open(&legacy_paragraph_shading_doc())
            .unwrap()
            .model();
        let mut baseline = recovered.clone();
        for block in &mut baseline.blocks {
            let Block::Paragraph(paragraph) = block else {
                panic!("synthetic legacy block must be a paragraph");
            };
            paragraph.props.shading = None;
        }

        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let recovered_pdf = render_pdf_with_fonts(&recovered, &fonts);
        let baseline_pdf = render_pdf_with_fonts(&baseline, &fonts);

        assert_ne!(recovered_pdf, baseline_pdf);
        assert_eq!(recovered_pdf, render_pdf_with_fonts(&recovered, &fonts));
        assert_eq!(
            layout_pages_with_fonts(&recovered, &fonts).unwrap(),
            layout_pages_with_fonts(&baseline, &fonts).unwrap()
        );
    }

    #[test]
    fn opened_legacy_doc_preserves_direct_paragraph_bidi_and_justification() {
        let document = Document::open(&legacy_paragraph_bidi_doc()).unwrap();

        assert_eq!(
            paragraph_layouts(&document.model()),
            expected_legacy_paragraph_layouts()
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_direct_paragraph_bidi_roundtrips_to_docx() {
        let legacy = Document::open(&legacy_paragraph_bidi_doc()).unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();

        assert_eq!(
            paragraph_layouts(&reopened.model()),
            expected_legacy_paragraph_layouts()
        );
    }

    fn legacy_paragraph_style_bidi_doc(style_properties: &[(u16, u8)]) -> Vec<u8> {
        let paragraphs = [
            ("StyleOnly", vec![0x00, 0x46, 0x01, 0x00]),
            ("DirectLtr", vec![0x00, 0x46, 0x01, 0x00, 0x41, 0x24, 0x00]),
            (
                "DirectPhysicalLeft",
                vec![0x00, 0x46, 0x01, 0x00, 0x03, 0x24, 0x00],
            ),
        ];
        let mut text = String::new();
        let mut runs = Vec::new();
        for (label, grpprl) in paragraphs {
            text.push_str(label);
            text.push('\r');
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl,
            });
        }
        let text_end = text.encode_utf16().count() as u32;
        let stylesheet = synthetic_paragraph_stylesheet(style_properties);
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                stylesheet: Some(&stylesheet),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn expected_legacy_paragraph_style_layouts() -> Vec<(String, bool, Align)> {
        vec![
            ("StyleOnly".to_string(), true, Align::Right),
            ("DirectLtr".to_string(), false, Align::Left),
            ("DirectPhysicalLeft".to_string(), true, Align::Left),
        ]
    }

    #[test]
    fn opened_legacy_doc_resolves_paragraph_style_bidi_before_direct_overrides() {
        let document = Document::open(&legacy_paragraph_style_bidi_doc(&[
            (0x2441, 1),
            (0x2461, 0),
        ]))
        .unwrap();

        assert_eq!(
            paragraph_layouts(&document.model()),
            expected_legacy_paragraph_style_layouts()
        );
    }

    #[test]
    fn opened_legacy_doc_resolves_physical_justification_from_paragraph_style() {
        let document = Document::open(&legacy_paragraph_style_bidi_doc(&[
            (0x2441, 1),
            (0x2403, 0),
        ]))
        .unwrap();

        assert_eq!(
            paragraph_layouts(&document.model())[0],
            ("StyleOnly".to_string(), true, Align::Left)
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_paragraph_style_bidi_roundtrips_to_docx() {
        let legacy = Document::open(&legacy_paragraph_style_bidi_doc(&[
            (0x2441, 1),
            (0x2461, 0),
        ]))
        .unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();

        assert_eq!(
            paragraph_layouts(&reopened.model()),
            expected_legacy_paragraph_style_layouts()
        );
    }

    fn legacy_table_cell_paragraph_bidi_doc() -> Vec<u8> {
        let text = "First\u{7}Second\u{7}\u{7}";
        let first_cell_end = "First\u{7}".encode_utf16().count() as u32;
        let second_cell_end = "First\u{7}Second\u{7}".encode_utf16().count() as u32;
        let row_end = text.encode_utf16().count() as u32;
        let mut row_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
            0x17, 0x24, 0x01, // sprmPFTtp
            0x08, 0xD6, 0x30, 0x00, // sprmTDefTable, cb=48
            0x02, // two cells
            0x00, 0x00, 0xE8, 0x03, 0xD0, 0x07, // boundaries 0..1000..2000
        ];
        row_grpprl.extend_from_slice(&[0u8; 40]);
        let runs = [
            SyntheticPapxRun {
                cp_lim: first_cell_end,
                grpprl: vec![
                    0x16, 0x24, 0x01, // sprmPFInTable
                    0x41, 0x24, 0x01, // sprmPFBiDi
                    0x03, 0x24, 0x00, // sprmPJc80 physical left
                ],
            },
            SyntheticPapxRun {
                cp_lim: second_cell_end,
                grpprl: vec![0x16, 0x24, 0x01],
            },
            SyntheticPapxRun {
                cp_lim: row_end,
                grpprl: row_grpprl,
            },
        ];
        synth_doc_with_ccp_and_tables(
            text,
            "",
            0x00C1,
            0,
            0,
            [row_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn assert_rtl_paragraph_inside_ltr_table(model: &DocModel) {
        let Block::Table(table) = &model.blocks[0] else {
            panic!("synthetic legacy block must be a table");
        };
        assert!(!table.bidi_visual);
        assert_eq!(table.rows[0].cells.len(), 2);
        let Block::Paragraph(first) = &table.rows[0].cells[0].blocks[0] else {
            panic!("synthetic first table cell must contain a paragraph");
        };
        let Block::Paragraph(second) = &table.rows[0].cells[1].blocks[0] else {
            panic!("synthetic second table cell must contain a paragraph");
        };
        assert_eq!(first.text(), "First");
        assert!(first.props.bidi);
        assert_eq!(first.props.align, Align::Left);
        assert_eq!(second.text(), "Second");
        assert!(!second.props.bidi);
        assert_eq!(second.props.align, Align::Left);
    }

    #[test]
    fn opened_legacy_doc_keeps_paragraph_bidi_independent_from_table_direction() {
        let document = Document::open(&legacy_table_cell_paragraph_bidi_doc()).unwrap();

        assert_rtl_paragraph_inside_ltr_table(&document.model());
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_table_cell_paragraph_bidi_roundtrips_without_mirroring_table() {
        let legacy = Document::open(&legacy_table_cell_paragraph_bidi_doc()).unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();

        assert_rtl_paragraph_inside_ltr_table(&reopened.model());
    }

    #[cfg(feature = "render")]
    fn legacy_inherited_table_cell_spacing_doc(paragraph_count: usize) -> Vec<u8> {
        const SPRM_P_DYA_BEFORE: u16 = 0xA413;
        const SPRM_P_DYA_AFTER: u16 = 0xA414;

        let mut style_spacing = Vec::new();
        push_paragraph_spacing_twips(&mut style_spacing, SPRM_P_DYA_BEFORE, 220);
        push_paragraph_spacing_twips(&mut style_spacing, SPRM_P_DYA_AFTER, 140);
        let stylesheet = synthetic_paragraph_stylesheet_from_styles(&[
            (0, 0, 0x0FFF, "Normal", &[]),
            (1, 0x0FFE, 0, "CellSpacing", &style_spacing),
            (2, 0x0FFD, 1, "CellSpacingChild", &[]),
        ]);

        let mut text = String::new();
        for index in 0..paragraph_count {
            text.push_str(&format!("cell paragraph {index}"));
            text.push(if index + 1 == paragraph_count {
                '\u{7}'
            } else {
                '\r'
            });
        }
        let cell_end = text.encode_utf16().count() as u32;
        let mut runs = vec![SyntheticPapxRun {
            cp_lim: cell_end,
            grpprl: vec![
                0x16, 0x24, 0x01, // sprmPFInTable
                0x00, 0x46, 0x02, 0x00, // sprmPIstd = child style 2
            ],
        }];
        text.push('\u{7}');
        let row_end = text.encode_utf16().count() as u32;
        let mut row_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
            0x17, 0x24, 0x01, // sprmPFTtp
            0x08, 0xD6, 0x1A, 0x00, // sprmTDefTable, cb=26
            0x01, // one cell
            0x00, 0x00, 0xD0, 0x07, // cell boundaries 0..2000 twips
        ];
        row_grpprl.extend_from_slice(&[0u8; 20]);
        runs.push(SyntheticPapxRun {
            cp_lim: row_end,
            grpprl: row_grpprl,
        });

        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [row_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                stylesheet: Some(&stylesheet),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn legacy_absolute_table_cell_spacing_doc(
        paragraph_count: usize,
        line_spacing: (u16, u16),
    ) -> Vec<u8> {
        let mut text = String::new();
        for index in 0..paragraph_count {
            text.push_str(&format!("cell paragraph {index}"));
            text.push(if index + 1 == paragraph_count {
                '\u{7}'
            } else {
                '\r'
            });
        }
        let cell_end = text.encode_utf16().count() as u32;
        let mut cell_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
        ];
        push_paragraph_line_spacing(&mut cell_grpprl, line_spacing.0, line_spacing.1);
        let mut runs = vec![SyntheticPapxRun {
            cp_lim: cell_end,
            grpprl: cell_grpprl,
        }];
        text.push('\u{7}');
        let row_end = text.encode_utf16().count() as u32;
        let mut row_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
            0x17, 0x24, 0x01, // sprmPFTtp
            0x08, 0xD6, 0x1A, 0x00, // sprmTDefTable, cb=26
            0x01, // one cell
            0x00, 0x00, 0xD0, 0x07, // cell boundaries 0..2000 twips
        ];
        row_grpprl.extend_from_slice(&[0u8; 20]);
        runs.push(SyntheticPapxRun {
            cp_lim: row_end,
            grpprl: row_grpprl,
        });

        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [row_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_inherited_cell_spacing_changes_preview_deterministically() {
        const PARAGRAPH_COUNT: usize = 40;
        let recovered = Document::open(&legacy_inherited_table_cell_spacing_doc(PARAGRAPH_COUNT))
            .unwrap()
            .model();
        let Block::Table(table) = &recovered.blocks[0] else {
            panic!("synthetic legacy block must be a table");
        };
        let paragraphs = table.rows[0].cells[0]
            .blocks
            .iter()
            .map(|block| {
                let Block::Paragraph(paragraph) = block else {
                    panic!("synthetic legacy cell block must be a paragraph");
                };
                paragraph
            })
            .collect::<Vec<_>>();
        assert_eq!(paragraphs.len(), PARAGRAPH_COUNT);
        assert!(paragraphs.iter().all(|paragraph| {
            paragraph.props.spacing.before_pt == Some(11.0)
                && paragraph.props.spacing.after_pt == Some(7.0)
        }));

        let mut compact = recovered.clone();
        let Block::Table(compact_table) = &mut compact.blocks[0] else {
            panic!("synthetic legacy block must remain a table");
        };
        for block in &mut compact_table.rows[0].cells[0].blocks {
            let Block::Paragraph(paragraph) = block else {
                panic!("synthetic legacy cell block must remain a paragraph");
            };
            paragraph.props.spacing.before_pt = Some(0.0);
            paragraph.props.spacing.after_pt = Some(0.0);
        }

        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let compact_layout = layout_pages_with_fonts(&compact, &fonts).unwrap();
        let recovered_layout = layout_pages_with_fonts(&recovered, &fonts).unwrap();
        assert_eq!(
            recovered_layout,
            layout_pages_with_fonts(&recovered, &fonts).unwrap()
        );
        assert!(recovered_layout.pages > compact_layout.pages);

        let compact_pdf = render_pdf_with_fonts(&compact, &fonts);
        let recovered_pdf = render_pdf_with_fonts(&recovered, &fonts);
        assert_ne!(recovered_pdf, compact_pdf);
        assert_eq!(recovered_pdf, render_pdf_with_fonts(&recovered, &fonts));
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_absolute_table_cell_spacing_changes_preview_layout() {
        const PARAGRAPH_COUNT: usize = 40;
        const EXACT_FIVE_POINTS: u16 = 0xFF9C;
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let exact = Document::open(&legacy_absolute_table_cell_spacing_doc(
            PARAGRAPH_COUNT,
            (EXACT_FIVE_POINTS, 0),
        ))
        .unwrap();
        let minimum = Document::open(&legacy_absolute_table_cell_spacing_doc(
            PARAGRAPH_COUNT,
            (800, 0),
        ))
        .unwrap();

        assert_eq!(
            exact.model(),
            minimum.model(),
            "legacy table-cell absolute spacing must remain outside the public model"
        );
        let Backend::Doc(exact_state) = &exact.backend else {
            panic!("synthetic legacy document must use the DOC backend");
        };
        let Backend::Doc(minimum_state) = &minimum.backend else {
            panic!("synthetic legacy document must use the DOC backend");
        };
        let exact_hints = legacy_build_output_from_doc_state(exact_state).table_cell_line_spacing;
        let minimum_hints =
            legacy_build_output_from_doc_state(minimum_state).table_cell_line_spacing;
        assert_eq!(exact_hints.len(), 1);
        assert_eq!(exact_hints[0].len(), 1);
        assert_eq!(exact_hints[0][0].len(), 1);
        assert_eq!(exact_hints[0][0][0].len(), PARAGRAPH_COUNT);
        assert_eq!(minimum_hints.len(), 1);
        assert_eq!(minimum_hints[0].len(), 1);
        assert_eq!(minimum_hints[0][0].len(), 1);
        assert_eq!(minimum_hints[0][0][0].len(), PARAGRAPH_COUNT);
        assert!(exact_hints[0][0][0]
            .iter()
            .all(|hint| *hint == Some(crate::model::LineSpacingHint::Exact(5.0))));
        assert!(minimum_hints[0][0][0]
            .iter()
            .all(|hint| *hint == Some(crate::model::LineSpacingHint::AtLeast(40.0))));
        let exact_layout = exact.layout_pages_with_fonts(&fonts).unwrap();
        let minimum_layout = minimum.layout_pages_with_fonts(&fonts).unwrap();
        assert!(minimum_layout.pages > exact_layout.pages);

        let exact_pdf = exact.to_pdf_with_fonts(&fonts);
        let minimum_pdf = minimum.to_pdf_with_fonts(&fonts);
        assert!(exact_pdf.starts_with(b"%PDF-"));
        assert!(minimum_pdf.starts_with(b"%PDF-"));
        assert_ne!(exact_pdf, minimum_pdf);
        assert_eq!(exact_pdf, exact.to_pdf_with_fonts(&fonts));
        assert_eq!(minimum_pdf, minimum.to_pdf_with_fonts(&fonts));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_table_cell_absolute_line_spacing_roundtrips_to_docx() {
        let convert = |line_spacing| {
            let legacy =
                Document::open(&legacy_absolute_table_cell_spacing_doc(2, line_spacing)).unwrap();
            let converted = legacy.to_docx();
            let document_xml = docx_part(&converted, "word/document.xml");
            (legacy, converted, document_xml)
        };

        let (exact, exact_docx, exact_xml) = convert((0xFF9C, 0));
        assert_eq!(
            exact_xml
                .matches(r#"w:line="100" w:lineRule="exact""#)
                .count(),
            2,
            "{exact_xml}"
        );
        assert_eq!(exact_docx, exact.to_docx());

        let (minimum, minimum_docx, minimum_xml) = convert((800, 0));
        assert_eq!(
            minimum_xml
                .matches(r#"w:line="800" w:lineRule="atLeast""#)
                .count(),
            2,
            "{minimum_xml}"
        );
        assert_eq!(minimum_docx, minimum.to_docx());

        let model_only_xml = docx_part(&write_docx(&exact.model()), "word/document.xml");
        assert!(!model_only_xml.contains(r#"w:lineRule="exact""#));
        assert!(!model_only_xml.contains(r#"w:lineRule="atLeast""#));

        let docx_backed = Document::open(&exact_docx).unwrap();
        let docx_backed_xml = docx_part(&docx_backed.to_docx(), "word/document.xml");
        assert_eq!(docx_backed_xml.matches(r#"w:lineRule="exact""#).count(), 2);
        assert!(!docx_backed_xml.contains(r#"w:lineRule="atLeast""#));

        #[cfg(feature = "render")]
        {
            let reopened_hints = |bytes: &[u8]| {
                let reopened = Document::open(bytes).unwrap();
                let Backend::Docx(state) = reopened.backend else {
                    panic!("converted document must use the DOCX backend");
                };
                state
                    .table_cell_line_spacing
                    .into_iter()
                    .find(|table| !table.is_empty())
                    .and_then(|table| table.first().cloned())
                    .and_then(|row| row.first().cloned())
                    .expect("converted table-cell line-spacing hints")
            };
            assert_eq!(
                reopened_hints(&exact_docx),
                vec![
                    Some(crate::model::LineSpacingHint::Exact(5.0)),
                    Some(crate::model::LineSpacingHint::Exact(5.0)),
                ]
            );
            assert_eq!(
                reopened_hints(&minimum_docx),
                vec![
                    Some(crate::model::LineSpacingHint::AtLeast(40.0)),
                    Some(crate::model::LineSpacingHint::AtLeast(40.0)),
                ]
            );
        }
    }

    fn legacy_table_bidi_doc() -> Vec<u8> {
        let rows = [
            (
                "A1",
                "A2",
                [(0x560Bu16, 1u16), (0x5664u16, 0u16)],
                [0i16, 1000, 2000],
            ),
            (
                "B1",
                "B2",
                [(0x5664u16, 1u16), (0x560Bu16, 0u16)],
                [0i16, 1000, 2000],
            ),
            (
                "C1",
                "C2",
                [(0x560Bu16, 0u16), (0x5664u16, 0u16)],
                [0i16, 1000, 4000],
            ),
        ];
        let mut text = String::new();
        let mut runs = Vec::new();
        for (first, second, direction, boundaries) in rows {
            text.push_str(first);
            text.push('\u{7}');
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl: vec![0x16, 0x24, 0x01],
            });
            text.push_str(second);
            text.push('\u{7}');
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl: vec![0x16, 0x24, 0x01],
            });
            text.push('\u{7}');
            let mut row_grpprl = vec![
                0x16, 0x24, 0x01, // sprmPFInTable
                0x17, 0x24, 0x01, // sprmPFTtp
            ];
            for (sprm, value) in direction {
                row_grpprl.extend_from_slice(&sprm.to_le_bytes());
                row_grpprl.extend_from_slice(&value.to_le_bytes());
            }
            row_grpprl.extend_from_slice(&[
                0x08, 0xD6, 0x30, 0x00, // sprmTDefTable, cb=48
                0x02, // two cells
            ]);
            for boundary in boundaries {
                row_grpprl.extend_from_slice(&boundary.to_le_bytes());
            }
            row_grpprl.extend_from_slice(&[0u8; 40]);
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl: row_grpprl,
            });
        }
        let text_end = text.encode_utf16().count() as u32;
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn legacy_table_bidi_signature(model: &DocModel) -> Vec<(bool, Vec<Vec<String>>)> {
        model
            .blocks
            .iter()
            .map(|block| {
                let Block::Table(table) = block else {
                    panic!("synthetic legacy block must be a table");
                };
                (
                    table.bidi_visual,
                    table
                        .rows
                        .iter()
                        .map(|row| row.cells.iter().map(Cell::text).collect())
                        .collect(),
                )
            })
            .collect()
    }

    fn expected_legacy_table_bidi_signature() -> Vec<(bool, Vec<Vec<String>>)> {
        vec![
            (
                true,
                vec![
                    vec!["A1".to_string(), "A2".to_string()],
                    vec!["B1".to_string(), "B2".to_string()],
                ],
            ),
            (false, vec![vec!["C1".to_string(), "C2".to_string()]]),
        ]
    }

    #[test]
    fn opened_legacy_doc_preserves_direct_table_bidi_and_direction_boundaries() {
        let document = Document::open(&legacy_table_bidi_doc()).unwrap();
        let model = document.model();

        assert_eq!(
            legacy_table_bidi_signature(&model),
            expected_legacy_table_bidi_signature()
        );
        let [Block::Table(rtl), Block::Table(ltr)] = model.blocks.as_slice() else {
            panic!("direction change must split the synthetic tables");
        };
        assert_eq!(rtl.col_widths_pct, vec![0.5, 0.5]);
        assert_eq!(ltr.col_widths_pct, vec![0.25, 0.75]);
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_table_bidi_roundtrips_to_docx() {
        let legacy = Document::open(&legacy_table_bidi_doc()).unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();

        assert_eq!(
            legacy_table_bidi_signature(&reopened.model()),
            expected_legacy_table_bidi_signature()
        );
    }

    #[derive(Clone, Copy)]
    enum SyntheticLegacyTableBorderSource {
        EmbeddedTc80,
        DirectCompatibility,
        DirectModern,
    }

    fn legacy_table_borders_doc(
        bidi_visual: bool,
        source: SyntheticLegacyTableBorderSource,
        auto_color: bool,
    ) -> Vec<u8> {
        let mut borders80 = [
            [2, 0x01, 2, 0],
            [4, 0x03, 3, 0],
            [6, 0x06, 4, 0],
            [8, 0x07, 5, 0],
            [10, 0x01, 6, 0],
            [12, 0x03, 7, 0],
        ];
        if auto_color {
            for border in &mut borders80 {
                border[2] = 0;
            }
        }
        let mut text = String::new();
        let mut runs = Vec::new();
        for (row_index, (first, second)) in [("A1", "A2"), ("B1", "B2")].into_iter().enumerate() {
            text.push_str(first);
            text.push('\u{7}');
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl: vec![0x16, 0x24, 0x01],
            });
            text.push_str(second);
            text.push('\u{7}');
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl: vec![0x16, 0x24, 0x01],
            });
            text.push('\u{7}');
            let mut row_grpprl = vec![
                0x16, 0x24, 0x01, // sprmPFInTable
                0x17, 0x24, 0x01, // sprmPFTtp
            ];
            if bidi_visual {
                row_grpprl.extend_from_slice(&[
                    0x0B, 0x56, 0x01, 0x00, // sprmTFBiDi
                ]);
            }
            row_grpprl.extend_from_slice(&[
                0x08, 0xD6, 0x30, 0x00, // sprmTDefTable, cb=48
                0x02, // two cells
                0x00, 0x00, 0xE8, 0x03, 0xD0, 0x07, // boundaries 0..1000..2000
            ]);
            match source {
                SyntheticLegacyTableBorderSource::EmbeddedTc80 => {
                    let cell_borders = if row_index == 0 {
                        [
                            [borders80[0], borders80[1], borders80[4], borders80[5]],
                            [borders80[0], borders80[5], borders80[4], borders80[3]],
                        ]
                    } else {
                        [
                            [borders80[4], borders80[1], borders80[2], borders80[5]],
                            [borders80[4], borders80[5], borders80[2], borders80[3]],
                        ]
                    };
                    for borders in cell_borders {
                        row_grpprl.extend_from_slice(&[0u8; 4]);
                        for border in borders {
                            row_grpprl.extend_from_slice(&border);
                        }
                    }
                }
                SyntheticLegacyTableBorderSource::DirectCompatibility
                | SyntheticLegacyTableBorderSource::DirectModern => {
                    row_grpprl.extend_from_slice(&[0u8; 40]);
                }
            }
            match source {
                SyntheticLegacyTableBorderSource::EmbeddedTc80 => {}
                SyntheticLegacyTableBorderSource::DirectCompatibility => {
                    row_grpprl.extend_from_slice(&[
                        0x05, 0xD6, 0x18, // sprmTTableBorders80, cb=24
                    ]);
                    for border in borders80 {
                        row_grpprl.extend_from_slice(&border);
                    }
                }
                SyntheticLegacyTableBorderSource::DirectModern => {
                    row_grpprl.extend_from_slice(&[
                        0x13, 0xD6, 0x30, // sprmTTableBorders, cb=48
                    ]);
                    for (border, color) in borders80.into_iter().zip([
                        Color::rgb(0, 0, 0xFF),
                        Color::rgb(0, 0xFF, 0xFF),
                        Color::rgb(0, 0xFF, 0),
                        Color::rgb(0xFF, 0, 0xFF),
                        Color::rgb(0xFF, 0, 0),
                        Color::rgb(0xFF, 0xFF, 0),
                    ]) {
                        row_grpprl.extend_from_slice(&[
                            color.r, color.g, color.b, 0, border[0], border[1], 0, 0,
                        ]);
                    }
                }
            }
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl: row_grpprl,
            });
        }
        let text_end = text.encode_utf16().count() as u32;
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    fn legacy_table_borders80_doc(bidi_visual: bool) -> Vec<u8> {
        legacy_table_borders_doc(
            bidi_visual,
            SyntheticLegacyTableBorderSource::DirectCompatibility,
            false,
        )
    }

    type LegacyTableBorderSignature = Vec<(
        TableBorderSide,
        Option<Color>,
        Option<u16>,
        Option<TableBorderStyle>,
    )>;

    fn legacy_table_border_signature(model: &DocModel) -> LegacyTableBorderSignature {
        let Block::Table(table) = &model.blocks[0] else {
            panic!("synthetic legacy block must be a table");
        };
        [
            TableBorderSide::Top,
            TableBorderSide::Left,
            TableBorderSide::Bottom,
            TableBorderSide::Right,
            TableBorderSide::InsideHorizontal,
            TableBorderSide::InsideVertical,
        ]
        .into_iter()
        .map(|side| {
            (
                side,
                table.border_colors.get(side).or(table.border_color),
                table.border_sizes.get(side).or(table.border_size_eighths),
                table.border_styles.get(side).or(table.border_style),
            )
        })
        .collect()
    }

    fn expected_legacy_table_border_signature(bidi_visual: bool) -> LegacyTableBorderSignature {
        let logical_left = (
            Some(Color::rgb(0, 0xFF, 0xFF)),
            Some(4),
            Some(TableBorderStyle::Double),
        );
        let logical_right = (
            Some(Color::rgb(0xFF, 0, 0xFF)),
            Some(8),
            Some(TableBorderStyle::Dashed),
        );
        let (left, right) = if bidi_visual {
            (logical_right, logical_left)
        } else {
            (logical_left, logical_right)
        };
        vec![
            (
                TableBorderSide::Top,
                Some(Color::rgb(0, 0, 0xFF)),
                Some(2),
                Some(TableBorderStyle::Single),
            ),
            (TableBorderSide::Left, left.0, left.1, left.2),
            (
                TableBorderSide::Bottom,
                Some(Color::rgb(0, 0xFF, 0)),
                Some(6),
                Some(TableBorderStyle::Dotted),
            ),
            (TableBorderSide::Right, right.0, right.1, right.2),
            (
                TableBorderSide::InsideHorizontal,
                Some(Color::rgb(0xFF, 0, 0)),
                Some(10),
                Some(TableBorderStyle::Single),
            ),
            (
                TableBorderSide::InsideVertical,
                Some(Color::rgb(0xFF, 0xFF, 0)),
                Some(12),
                Some(TableBorderStyle::Double),
            ),
        ]
    }

    #[test]
    fn opened_legacy_doc_recovers_coherent_direct_table_borders80() {
        let document = Document::open(&legacy_table_borders80_doc(false)).unwrap();

        assert_eq!(
            legacy_table_border_signature(&document.model()),
            expected_legacy_table_border_signature(false)
        );
    }

    #[test]
    fn opened_legacy_doc_recovers_coherent_embedded_tc80_table_borders() {
        for bidi_visual in [false, true] {
            let document = Document::open(&legacy_table_borders_doc(
                bidi_visual,
                SyntheticLegacyTableBorderSource::EmbeddedTc80,
                false,
            ))
            .unwrap();

            assert_eq!(
                legacy_table_border_signature(&document.model()),
                expected_legacy_table_border_signature(bidi_visual)
            );
        }
    }

    #[test]
    fn opened_legacy_doc_maps_logical_table_borders80_to_physical_rtl_sides() {
        let document = Document::open(&legacy_table_borders80_doc(true)).unwrap();

        assert_eq!(
            legacy_table_border_signature(&document.model()),
            expected_legacy_table_border_signature(true)
        );
    }

    #[test]
    fn opened_legacy_doc_recovers_coherent_direct_modern_table_borders() {
        for bidi_visual in [false, true] {
            let document = Document::open(&legacy_table_borders_doc(
                bidi_visual,
                SyntheticLegacyTableBorderSource::DirectModern,
                false,
            ))
            .unwrap();

            assert_eq!(
                legacy_table_border_signature(&document.model()),
                expected_legacy_table_border_signature(bidi_visual)
            );
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_table_borders_roundtrip_to_docx() {
        for source in [
            SyntheticLegacyTableBorderSource::EmbeddedTc80,
            SyntheticLegacyTableBorderSource::DirectCompatibility,
            SyntheticLegacyTableBorderSource::DirectModern,
        ] {
            for bidi_visual in [false, true] {
                let legacy =
                    Document::open(&legacy_table_borders_doc(bidi_visual, source, false)).unwrap();
                let reopened = Document::open(&legacy.to_docx()).unwrap();

                assert_eq!(
                    legacy_table_border_signature(&reopened.model()),
                    expected_legacy_table_border_signature(bidi_visual)
                );
            }
        }

        let legacy = Document::open(&legacy_table_borders_doc(
            false,
            SyntheticLegacyTableBorderSource::EmbeddedTc80,
            true,
        ))
        .unwrap();
        let reopened = Document::open(&legacy.to_docx()).unwrap();
        assert_eq!(
            legacy_table_border_signature(&reopened.model()),
            legacy_table_border_signature(&legacy.model())
        );
        assert!(legacy_table_border_signature(&reopened.model())
            .iter()
            .all(|(_, color, _, _)| color.is_none()));
    }

    #[cfg(feature = "render")]
    #[test]
    fn legacy_doc_table_border_recovery_changes_pdf_without_changing_layout() {
        for source in [
            SyntheticLegacyTableBorderSource::EmbeddedTc80,
            SyntheticLegacyTableBorderSource::DirectCompatibility,
            SyntheticLegacyTableBorderSource::DirectModern,
        ] {
            for bidi_visual in [false, true] {
                let document =
                    Document::open(&legacy_table_borders_doc(bidi_visual, source, false)).unwrap();
                let recovered = document.model();
                let mut baseline = recovered.clone();
                let Block::Table(table) = &mut baseline.blocks[0] else {
                    panic!("synthetic legacy block must be a table");
                };
                table.border_color = None;
                table.border_colors = TableBorderColors::default();
                table.border_size_eighths = None;
                table.border_sizes = TableBorderSizes::default();
                table.border_style = None;
                table.border_styles = TableBorderStyles::default();

                let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
                let recovered_pdf = render_pdf_with_fonts(&recovered, &fonts);
                let baseline_pdf = render_pdf_with_fonts(&baseline, &fonts);

                assert_ne!(recovered_pdf, baseline_pdf);
                assert_eq!(recovered_pdf, render_pdf_with_fonts(&recovered, &fonts));
                assert_eq!(
                    layout_pages_with_fonts(&recovered, &fonts).unwrap(),
                    layout_pages_with_fonts(&baseline, &fonts).unwrap()
                );
            }
        }
    }

    #[test]
    fn opened_legacy_doc_preserves_tdef_table_column_proportions() {
        let text = "left\u{7}right\u{7}\u{7}";
        let first_cell_end = "left\u{7}".encode_utf16().count() as u32;
        let second_cell_end = "left\u{7}right\u{7}".encode_utf16().count() as u32;
        let row_end = text.encode_utf16().count() as u32;
        let mut row_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
            0x17, 0x24, 0x01, // sprmPFTtp
            0x08, 0xD6, 0x30, 0x00, // sprmTDefTable, cb=48
            0x02, // two cells
        ];
        for boundary in [-500i16, 500, 3500] {
            row_grpprl.extend_from_slice(&boundary.to_le_bytes());
        }
        row_grpprl.extend_from_slice(&[0u8; 40]); // two TC80 records
        let runs = [
            SyntheticPapxRun {
                cp_lim: first_cell_end,
                grpprl: vec![0x16, 0x24, 0x01],
            },
            SyntheticPapxRun {
                cp_lim: second_cell_end,
                grpprl: vec![0x16, 0x24, 0x01],
            },
            SyntheticPapxRun {
                cp_lim: row_end,
                grpprl: row_grpprl,
            },
        ];
        let bytes = synth_doc_with_ccp_and_tables(
            text,
            "",
            0x00C1,
            0,
            0,
            [row_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        );

        let document = Document::open(&bytes).unwrap();
        let model = document.model();
        let Block::Table(table) = &model.blocks[0] else {
            panic!("synthetic legacy block must be a table");
        };
        assert_eq!(table.rows[0].cells.len(), 2);
        assert_eq!(table.col_widths_pct.len(), 2);
        assert!((table.col_widths_pct[0] - 0.25).abs() < f32::EPSILON);
        assert!((table.col_widths_pct[1] - 0.75).abs() < f32::EPSILON);
    }

    #[test]
    fn malformed_partial_tc80_doc_preserves_table_structure_without_projecting_borders() {
        let text = "left\u{7}right\u{7}\u{7}";
        let first_cell_end = "left\u{7}".encode_utf16().count() as u32;
        let second_cell_end = "left\u{7}right\u{7}".encode_utf16().count() as u32;
        let row_end = text.encode_utf16().count() as u32;
        let mut row_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
            0x17, 0x24, 0x01, // sprmPFTtp
            0x08, 0xD6, 0x1F, 0x00, // sprmTDefTable, cb=31
            0x02, // two cells
            0x00, 0x00, 0xE8, 0x03, 0xD0, 0x07, // boundaries 0..1000..2000
        ];
        row_grpprl.extend_from_slice(&[0u8; 23]); // one TC80 plus a malformed tail
        let runs = [
            SyntheticPapxRun {
                cp_lim: first_cell_end,
                grpprl: vec![0x16, 0x24, 0x01],
            },
            SyntheticPapxRun {
                cp_lim: second_cell_end,
                grpprl: vec![0x16, 0x24, 0x01],
            },
            SyntheticPapxRun {
                cp_lim: row_end,
                grpprl: row_grpprl,
            },
        ];
        let bytes = synth_doc_with_ccp_and_tables(
            text,
            "",
            0x00C1,
            0,
            0,
            [row_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        );

        let document = Document::open(&bytes).unwrap();
        let model = document.model();
        let Block::Table(table) = &model.blocks[0] else {
            panic!("synthetic legacy block must be a table");
        };
        assert_eq!(
            table.rows[0]
                .cells
                .iter()
                .map(Cell::text)
                .collect::<Vec<_>>(),
            ["left", "right"]
        );
        assert_eq!(table.col_widths_pct, vec![0.5, 0.5]);
        assert!(legacy_table_border_signature(&model)
            .iter()
            .all(|(_, color, size, style)| color.is_none() && size.is_none() && style.is_none()));
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_layout_uses_private_keep_lines_hints() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:pPr><w:spacing w:line="800" w:lineRule="exact"/></w:pPr><w:r><w:t>seed</w:t></w:r></w:p>
                <w:p><w:pPr><w:keepLines/><w:widowControl w:val="off"/><w:spacing w:line="200" w:lineRule="exact"/></w:pPr>
                    <w:r><w:t>one</w:t><w:br/><w:t>two</w:t><w:br/><w:t>three</w:t></w:r>
                </w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="2000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let document = Document::open(&bytes).unwrap();
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        let raw_model_layout = layout_pages_with_fonts(&document.model(), &fonts).unwrap();
        let opened_document_layout = document.layout_pages_with_fonts(&fonts).unwrap();

        assert_eq!(raw_model_layout.block_pages, vec![Some(1), Some(1)]);
        assert_eq!(opened_document_layout.block_pages, vec![Some(1), Some(2)]);
        assert_eq!(opened_document_layout.pages, 2);
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_docx_to_docx_preserves_private_running_surface_distances() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
                <w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
                <w:p><w:pPr><w:sectPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:pgMar w:header="0" w:footer="400"/></mc:Choice>
                    <mc:Fallback><w:pgMar w:header="100" w:footer="200"/></mc:Fallback>
                </mc:AlternateContent></w:sectPr></w:pPr>
                    <w:r><w:t>ending section</w:t></w:r>
                </w:p>
                <w:p><w:pPr><w:sectPr><w:pgMar w:header="1000" w:footer="4294967295"/></w:sectPr></w:pPr>
                    <w:r><w:t>bounded section</w:t></w:r>
                </w:p>
                <w:p><w:r><w:t>final section</w:t></w:r></w:p>
                <w:sectPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:pgMar w:header="bad" w:footer="0"/></mc:Choice>
                    <mc:Fallback><w:pgMar w:header="300" w:footer="500"/></mc:Fallback>
                </mc:AlternateContent></w:sectPr>
            </w:body></w:document>"#,
        );
        let document = Document::open(&bytes).unwrap();

        let converted = document.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");
        let page_margins = docx_page_margin_tags(&document_xml);

        assert_eq!(page_margins.len(), 3, "{document_xml}");
        assert!(page_margins[0].contains(r#"w:header="0""#));
        assert!(page_margins[0].contains(r#"w:footer="400""#));
        assert!(page_margins[1].contains(r#"w:header="1000""#));
        assert!(page_margins[1].contains(r#"w:footer="708""#));
        assert!(page_margins[2].contains(r#"w:header="708""#));
        assert!(page_margins[2].contains(r#"w:footer="0""#));
        assert!(!document_xml.contains(r#"w:header="100""#));
        assert!(!document_xml.contains(r#"w:footer="200""#));
        assert!(!document_xml.contains(r#"w:header="300""#));
        assert!(!document_xml.contains(r#"w:footer="500""#));
        assert_eq!(converted, document.to_docx());

        let reopened = Document::open(&converted).unwrap();
        let Backend::Docx(state) = &reopened.backend else {
            panic!("converted document must use the DOCX backend");
        };
        assert_eq!(
            state.running_surface_distances,
            [
                crate::model::RunningSurfaceDistanceHints {
                    header_pt: Some(0.0),
                    footer_pt: Some(20.0),
                },
                crate::model::RunningSurfaceDistanceHints {
                    header_pt: Some(50.0),
                    footer_pt: Some(35.4),
                },
                crate::model::RunningSurfaceDistanceHints {
                    header_pt: Some(35.4),
                    footer_pt: Some(0.0),
                },
            ]
        );

        let model_only_xml = docx_part(&write_docx(&document.model()), "word/document.xml");
        assert_eq!(
            docx_page_margin_tags(&model_only_xml)
                .iter()
                .filter(|tag| tag.contains(r#"w:header="708" w:footer="708""#))
                .count(),
            3
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_doc_to_docx_preserves_private_running_surface_distances() {
        let section_cps = [0, 5, 10, 15];
        let mut first = Vec::new();
        first.extend_from_slice(&0xB017u16.to_le_bytes());
        first.extend_from_slice(&0u16.to_le_bytes());
        first.extend_from_slice(&0xB018u16.to_le_bytes());
        first.extend_from_slice(&400u16.to_le_bytes());
        let malformed = [0x17];
        let mut final_section = Vec::new();
        final_section.extend_from_slice(&0xB017u16.to_le_bytes());
        final_section.extend_from_slice(&900u16.to_le_bytes());
        final_section.extend_from_slice(&0xB018u16.to_le_bytes());
        final_section.extend_from_slice(&0u16.to_le_bytes());
        let sepx_grpprls = [
            first.as_slice(),
            malformed.as_slice(),
            final_section.as_slice(),
        ];
        let bytes =
            legacy_doc_with_section_page_grpprls("AAAAABBBBBCCCCC", &section_cps, &sepx_grpprls);
        let document = Document::open(&bytes).unwrap();

        let converted = document.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");
        let page_margins = docx_page_margin_tags(&document_xml);
        assert_eq!(page_margins.len(), 3, "{document_xml}");
        assert!(page_margins[0].contains(r#"w:header="0""#));
        assert!(page_margins[0].contains(r#"w:footer="400""#));
        assert!(page_margins[1].contains(r#"w:header="708""#));
        assert!(page_margins[1].contains(r#"w:footer="708""#));
        assert!(page_margins[2].contains(r#"w:header="900""#));
        assert!(page_margins[2].contains(r#"w:footer="0""#));
        assert_eq!(converted, document.to_docx());

        let reopened = Document::open(&converted).unwrap();
        let Backend::Docx(state) = &reopened.backend else {
            panic!("converted document must use the DOCX backend");
        };
        assert_eq!(
            state.running_surface_distances,
            [
                crate::model::RunningSurfaceDistanceHints {
                    header_pt: Some(0.0),
                    footer_pt: Some(20.0),
                },
                crate::model::RunningSurfaceDistanceHints {
                    header_pt: Some(35.4),
                    footer_pt: Some(35.4),
                },
                crate::model::RunningSurfaceDistanceHints {
                    header_pt: Some(45.0),
                    footer_pt: Some(0.0),
                },
            ]
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_docx_to_docx_preserves_private_section_column_semantics() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:pPr><w:sectPr><w:cols w:num="2" w:space="800" w:sep="1"/></w:sectPr></w:pPr>
                    <w:r><w:t>ending section</w:t></w:r>
                </w:p>
                <w:p><w:r><w:t>final section</w:t></w:r></w:p>
                <w:sectPr><w:cols w:equalWidth="0">
                    <w:col w:w="3000" w:space="200"/><w:col w:w="5000"/>
                </w:cols><w:bidi/></w:sectPr>
            </w:body></w:document>"#,
        );
        let document = Document::open(&bytes).unwrap();

        let converted = document.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");
        let ending_columns = document_xml
            .find(r#"<w:cols w:num="2" w:space="800" w:sep="1"/>"#)
            .unwrap_or_else(|| panic!("ending-section column hints were lost: {document_xml}"));
        let final_columns = document_xml
            .find(
                r#"<w:cols w:num="2" w:equalWidth="0"><w:col w:w="3000" w:space="200"/><w:col w:w="5000"/></w:cols>"#,
            )
            .unwrap_or_else(|| panic!("final-section column hints were lost: {document_xml}"));
        assert!(ending_columns < final_columns, "{document_xml}");
        assert!(document_xml.contains("<w:bidi/>"), "{document_xml}");
        assert_eq!(converted, document.to_docx());

        let reopened = Document::open(&converted).unwrap();
        assert_eq!(reopened.model().setup.columns, Some(2));
        #[cfg(feature = "render")]
        reopened.with_render_model_and_hints(|model, hints| {
            let boundary = model
                .blocks
                .iter()
                .position(|block| matches!(block, Block::SectionBreak(_)))
                .expect("converted section boundary");
            assert_eq!(hints.section_column_gap_pt[boundary], Some(40.0));
            assert!(hints.section_column_separators[boundary]);
            let layout = hints
                .final_section_column_layout
                .expect("converted final custom columns");
            assert_eq!(layout.columns[0].width_pt, 150.0);
            assert_eq!(layout.columns[0].space_after_pt, 10.0);
            assert_eq!(layout.columns[1].width_pt, 250.0);
            assert!(hints.final_section_column_rtl);
        });

        let model_only = write_docx(&document.model());
        let model_only_xml = docx_part(&model_only, "word/document.xml");
        assert_eq!(model_only_xml.matches(r#"<w:cols w:num="2"/>"#).count(), 2);
        assert!(!model_only_xml.contains("w:equalWidth"));
        assert!(!model_only_xml.contains("<w:bidi/>"));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_docx_to_docx_selects_column_branch_and_isolates_bad_geometry() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
                <w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
                <w:p><w:pPr><w:sectPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:cols w:num="2" w:space="600" w:sep="1"/></mc:Choice>
                    <mc:Fallback><w:cols w:num="3" w:space="1200"/></mc:Fallback>
                </mc:AlternateContent></w:sectPr></w:pPr><w:r><w:t>choice</w:t></w:r></w:p>
                <w:p><w:pPr><w:sectPr><w:cols w:equalWidth="0" w:sep="1">
                    <w:col w:w="0" w:space="400"/><w:col w:w="4000"/>
                </w:cols><w:bidi/></w:sectPr></w:pPr><w:r><w:t>malformed</w:t></w:r></w:p>
                <w:p><w:r><w:t>final</w:t></w:r></w:p>
                <w:sectPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:cols w:num="2" w:space="200"/></mc:Choice>
                    <mc:Fallback><w:cols w:num="4" w:space="1400"/></mc:Fallback>
                </mc:AlternateContent></w:sectPr>
            </w:body></w:document>"#,
        );
        let document = Document::open(&bytes).unwrap();

        let document_xml = docx_part(&document.to_docx(), "word/document.xml");
        let selected = document_xml
            .find(r#"<w:cols w:num="2" w:space="600" w:sep="1"/>"#)
            .expect("selected column branch");
        let malformed = document_xml[selected + 1..]
            .find(r#"<w:cols w:sep="1"/>"#)
            .map(|index| selected + 1 + index)
            .expect("malformed section fallback");
        let final_section = document_xml[malformed + 1..]
            .find(r#"<w:cols w:num="2" w:space="200"/>"#)
            .map(|index| malformed + 1 + index)
            .expect("final section columns");

        assert!(selected < malformed && malformed < final_section);
        assert!(!document_xml.contains(r#"w:num="3""#));
        assert!(!document_xml.contains(r#"w:num="4""#));
        assert!(!document_xml.contains(r#"w:space="1200""#));
        assert!(!document_xml.contains(r#"w:space="1400""#));
        assert!(!document_xml.contains("w:equalWidth"));
        assert_eq!(document_xml.matches("<w:bidi/>").count(), 1);

        let reopened = Document::open(&document.to_docx()).unwrap();
        let section_columns = reopened
            .model()
            .blocks
            .into_iter()
            .filter_map(|block| match block {
                Block::SectionBreak(section) => Some(section.columns),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(section_columns, [Some(2), None]);
        assert_eq!(reopened.model().setup.columns, Some(2));
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_carries_section_local_equal_column_spacing_hints() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:pPr><w:sectPr><w:cols w:num="2" w:space="800"/></w:sectPr></w:pPr>
                    <w:r><w:t>ending section</w:t></w:r>
                </w:p>
                <w:p><w:r><w:t>final section</w:t></w:r></w:p>
                <w:sectPr><w:cols w:num="2" w:space="200"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let document = Document::open(&bytes).unwrap();

        document.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 3);
            assert_eq!(hints.section_column_gap_pt, &[None, Some(40.0), None]);
            assert_eq!(hints.final_section_column_gap_pt, Some(10.0));
        });
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_carries_section_local_unequal_column_geometry_hints() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:pPr><w:sectPr><w:cols w:equalWidth="0">
                    <w:col w:w="2000" w:space="400"/><w:col w:w="4000"/>
                </w:cols></w:sectPr></w:pPr><w:r><w:t>ending section</w:t></w:r></w:p>
                <w:p><w:r><w:t>final section</w:t></w:r></w:p>
                <w:sectPr><w:cols w:equalWidth="false">
                    <w:col w:w="3000" w:space="200"/><w:col w:w="5000"/>
                </w:cols></w:sectPr>
            </w:body></w:document>"#,
        );
        let document = Document::open(&bytes).unwrap();

        document.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 3);
            assert_eq!(model.setup.columns, Some(2));
            let ending = hints.section_column_layouts[1]
                .as_ref()
                .expect("ending-section geometry");
            assert_eq!(ending.columns[0].width_pt, 100.0);
            assert_eq!(ending.columns[0].space_after_pt, 20.0);
            assert_eq!(ending.columns[1].width_pt, 200.0);
            assert!(hints.section_column_layouts[0].is_none());
            assert!(hints.section_column_layouts[2].is_none());

            let final_layout = hints
                .final_section_column_layout
                .expect("final-section geometry");
            assert_eq!(final_layout.columns[0].width_pt, 150.0);
            assert_eq!(final_layout.columns[0].space_after_pt, 10.0);
            assert_eq!(final_layout.columns[1].width_pt, 250.0);
        });
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_carries_section_local_column_separator_hints() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:pPr><w:sectPr><w:cols w:num="2" w:sep="on"/></w:sectPr></w:pPr>
                    <w:r><w:t>ending section</w:t></w:r>
                </w:p>
                <w:p><w:r><w:t>final section</w:t></w:r></w:p>
                <w:sectPr><w:cols w:num="2" w:sep="true"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let document = Document::open(&bytes).unwrap();

        document.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 3);
            assert_eq!(hints.section_column_separators, &[false, true, false]);
            assert!(hints.final_section_column_separator);
        });
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_carries_section_local_rtl_column_hints() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:pPr><w:sectPr><w:cols w:num="2"/><w:bidi w:val="off"/><w:bidi/></w:sectPr></w:pPr>
                    <w:r><w:t>ending section</w:t></w:r>
                </w:p>
                <w:p><w:r><w:t>final section</w:t></w:r></w:p>
                <w:sectPr><w:cols w:num="2"/><w:bidi w:val="true"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let document = Document::open(&bytes).unwrap();

        document.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 3);
            assert_eq!(hints.section_column_rtl, &[false, true, false]);
            assert!(hints.final_section_column_rtl);
        });
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_unequal_columns_change_preview_pdf_deterministically() {
        let text = (0..24)
            .map(|index| format!("word{index}"))
            .collect::<Vec<_>>()
            .join(" ");
        let xml = format!(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:r><w:t>{text}</w:t></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="3000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/>
                    <w:cols w:equalWidth="0"><w:col w:w="1200" w:space="400"/><w:col w:w="2000"/></w:cols>
                </w:sectPr>
            </w:body></w:document>"#
        );
        let document = Document::open(&minimal_docx(&xml)).unwrap();
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        let opened_pdf = document.try_to_pdf_with_fonts(&fonts).unwrap();
        let model_only_pdf = render_pdf_with_fonts(&document.model(), &fonts);

        assert!(opened_pdf.starts_with(b"%PDF-"));
        assert_eq!(opened_pdf, document.try_to_pdf_with_fonts(&fonts).unwrap());
        assert_ne!(
            opened_pdf, model_only_pdf,
            "the public model intentionally omits unequal-column geometry"
        );
        assert_eq!(
            document.layout_pages_with_fonts(&fonts).unwrap(),
            document.layout_pages_with_fonts(&fonts).unwrap()
        );
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_column_separator_changes_preview_pdf_without_layout_change() {
        let document_xml = |separator: &str| {
            format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                    <w:p><w:r><w:t>first column content</w:t></w:r></w:p>
                    <w:sectPr><w:pgSz w:w="4400" w:h="3000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/>
                        <w:cols w:num="2" w:space="400"{separator}/>
                    </w:sectPr>
                </w:body></w:document>"#
            )
        };
        let without_separator =
            Document::open(&minimal_docx(&document_xml(""))).expect("baseline DOCX");
        let with_separator =
            Document::open(&minimal_docx(&document_xml(r#" w:sep="1""#))).expect("separated DOCX");
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        assert_eq!(without_separator.model(), with_separator.model());
        assert_eq!(
            without_separator.layout_pages_with_fonts(&fonts).unwrap(),
            with_separator.layout_pages_with_fonts(&fonts).unwrap()
        );
        let baseline_pdf = without_separator.try_to_pdf_with_fonts(&fonts).unwrap();
        let separated_pdf = with_separator.try_to_pdf_with_fonts(&fonts).unwrap();
        assert_eq!(
            separated_pdf,
            with_separator.try_to_pdf_with_fonts(&fonts).unwrap()
        );
        assert_ne!(baseline_pdf, separated_pdf);
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_rtl_section_populates_columns_from_right_without_layout_change() {
        let document_xml = |section_bidi: &str| {
            format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                    <w:p><w:r><w:t>first logical column</w:t></w:r></w:p>
                    <w:sectPr><w:pgSz w:w="4400" w:h="3000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/>
                        <w:cols w:num="2" w:space="400"/>{section_bidi}
                    </w:sectPr>
                </w:body></w:document>"#
            )
        };
        let ltr = Document::open(&minimal_docx(&document_xml(""))).expect("LTR DOCX");
        let rtl = Document::open(&minimal_docx(&document_xml("<w:bidi/>"))).expect("RTL DOCX");
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        assert_eq!(ltr.model(), rtl.model());
        assert_eq!(
            ltr.layout_pages_with_fonts(&fonts).unwrap(),
            rtl.layout_pages_with_fonts(&fonts).unwrap()
        );
        let ltr_pdf = ltr.try_to_pdf_with_fonts(&fonts).unwrap();
        let rtl_pdf = rtl.try_to_pdf_with_fonts(&fonts).unwrap();
        assert_eq!(rtl_pdf, rtl.try_to_pdf_with_fonts(&fonts).unwrap());
        assert_ne!(ltr_pdf, rtl_pdf);
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_one_column_separator_is_paint_inert() {
        let document_xml = |separator: &str| {
            format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                    <w:p><w:r><w:t>single column</w:t></w:r></w:p>
                    <w:sectPr><w:pgSz w:w="4400" w:h="3000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/>
                        <w:cols w:num="1"{separator}/>
                    </w:sectPr>
                </w:body></w:document>"#
            )
        };
        let baseline = Document::open(&minimal_docx(&document_xml(""))).unwrap();
        let separated = Document::open(&minimal_docx(&document_xml(r#" w:sep="1""#))).unwrap();
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        assert_eq!(baseline.model(), separated.model());
        assert_eq!(
            baseline.layout_pages_with_fonts(&fonts).unwrap(),
            separated.layout_pages_with_fonts(&fonts).unwrap()
        );
        assert_eq!(
            baseline.try_to_pdf_with_fonts(&fonts).unwrap(),
            separated.try_to_pdf_with_fonts(&fonts).unwrap()
        );
    }

    #[test]
    fn legacy_doc_promotes_internal_page_breaks_without_duplicating_section_breaks() {
        let single_text = "before\u{000c}after\r";
        let single_end = single_text.encode_utf16().count() as u32;
        let single = Document::open(&legacy_doc_with_section_page_grpprls(
            single_text,
            &[0, single_end],
            &[&[]],
        ))
        .unwrap()
        .model();
        let [Block::Paragraph(before), Block::PageBreak, Block::Paragraph(after)] =
            single.blocks.as_slice()
        else {
            panic!(
                "manual page break must split the legacy paragraph: {:?}",
                single.blocks
            );
        };
        assert_eq!(before.text(), "before");
        assert_eq!(after.text(), "after");

        let first_section = "first\u{000c}";
        let final_section = "second\u{000c}third\r";
        let combined = format!("{first_section}{final_section}");
        let first_end = first_section.encode_utf16().count() as u32;
        let combined_end = combined.encode_utf16().count() as u32;
        let sectioned = Document::open(&legacy_doc_with_section_page_grpprls(
            &combined,
            &[0, first_end, combined_end],
            &[&[], &[]],
        ))
        .unwrap()
        .model();
        let [Block::Paragraph(first), Block::SectionBreak(_), Block::Paragraph(second), Block::PageBreak, Block::Paragraph(third)] =
            sectioned.blocks.as_slice()
        else {
            panic!(
                "section terminator and manual page break must stay distinct: {:?}",
                sectioned.blocks
            );
        };
        assert_eq!(first.text(), "first");
        assert_eq!(second.text(), "second");
        assert_eq!(third.text(), "third");

        let main = "main\u{000c}body\r";
        let footnote = "note\u{000c}body\r";
        let stories_text = format!("{main}{footnote}");
        let main_len = main.encode_utf16().count() as u32;
        let footnote_len = footnote.encode_utf16().count() as u32;
        let stories = Document::open(&synth_doc_with_ccp_and_tables(
            &stories_text,
            "",
            0x00C1,
            0,
            0,
            [main_len, footnote_len, 0, 0, 0, 0],
            SyntheticDocTables::default(),
        ))
        .unwrap()
        .model();
        let [Block::Paragraph(main_before), Block::PageBreak, Block::Paragraph(main_after), Block::Paragraph(note)] =
            stories.blocks.as_slice()
        else {
            panic!(
                "only the main story page break may be promoted: {:?}",
                stories.blocks
            );
        };
        assert_eq!(main_before.text(), "main");
        assert_eq!(main_after.text(), "body");
        assert_eq!(note.text(), "note\nbody");
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_manual_page_breaks_survive_fresh_docx_conversion() {
        let text = "before\u{000c}after\r";
        let text_end = text.encode_utf16().count() as u32;
        let legacy = Document::open(&legacy_doc_with_section_page_grpprls(
            text,
            &[0, text_end],
            &[&[]],
        ))
        .unwrap();

        let reopened = Document::open(&legacy.to_docx()).unwrap().model();
        let [Block::Paragraph(before), Block::PageBreak, Block::Paragraph(after)] =
            reopened.blocks.as_slice()
        else {
            panic!("fresh DOCX conversion must retain the promoted page break");
        };
        assert_eq!(before.text(), "before");
        assert_eq!(after.text(), "after");
    }

    #[cfg(feature = "render")]
    fn legacy_manual_column_break_layout_doc() -> Vec<u8> {
        let text = "left\u{000e}right\u{000e}page two\r\u{000e}\rafter orphan\r";
        let text_end = text.encode_utf16().count() as u32;
        let mut section = section_page_grpprl(4400, 3000, 400, 400, 400, 400, false);
        push_section_column_count(&mut section, 1);
        legacy_doc_with_section_page_grpprls(text, &[0, text_end], &[&section])
    }

    #[cfg(feature = "docx")]
    fn legacy_table_cell_column_break_conversion_doc() -> Vec<u8> {
        let text = "A\u{000e}B\rC\u{000e}D\u{0007}\u{0007}TAIL\r";
        let first_paragraph_end = "A\u{000e}B\r".encode_utf16().count() as u32;
        let cell_end = "A\u{000e}B\rC\u{000e}D\u{0007}".encode_utf16().count() as u32;
        let row_end = cell_end + 1;
        let text_end = text.encode_utf16().count() as u32;
        let mut row_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
            0x17, 0x24, 0x01, // sprmPFTtp
            0x08, 0xD6, 0x1A, 0x00, // sprmTDefTable, cb=26
            0x01, // one cell
            0x00, 0x00, 0xD0, 0x07, // boundaries 0..2000 twips
        ];
        row_grpprl.extend_from_slice(&[0u8; 20]);
        let runs = [
            SyntheticPapxRun {
                cp_lim: first_paragraph_end,
                grpprl: vec![0x16, 0x24, 0x01],
            },
            SyntheticPapxRun {
                cp_lim: cell_end,
                grpprl: vec![0x16, 0x24, 0x01],
            },
            SyntheticPapxRun {
                cp_lim: row_end,
                grpprl: row_grpprl,
            },
            SyntheticPapxRun {
                cp_lim: text_end,
                grpprl: Vec::new(),
            },
        ];
        synth_doc_with_ccp_and_tables(
            text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_manual_column_breaks_reach_preview_flow() {
        let document = Document::open(&legacy_manual_column_break_layout_doc()).unwrap();
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        assert_eq!(document.model().blocks.len(), 3);
        document.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 3);
            assert_eq!(hints.column_break_offsets, &[vec![4, 10], vec![0], vec![]]);
        });

        let model_only = layout_pages_with_fonts(&document.model(), &fonts).unwrap();
        let opened = document.layout_pages_with_fonts(&fonts).unwrap();
        assert_eq!(model_only.pages, 1);
        assert_eq!(model_only.block_pages, vec![Some(1), Some(1), Some(1)]);
        assert_eq!(opened.pages, 2);
        assert_eq!(opened.block_pages, vec![Some(1), Some(2), Some(2)]);
        assert_eq!(opened, document.layout_pages_with_fonts(&fonts).unwrap());
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_column_break_capture_is_visible_main_story_only() {
        let hidden_text = "A\u{000e}B\u{000e}C\r";
        let hidden_text_end = hidden_text.encode_utf16().count() as u32;
        let mut section = section_page_grpprl(4400, 3000, 400, 400, 400, 400, false);
        push_section_column_count(&mut section, 1);
        let section_cps = [0, hidden_text_end];
        let section_grpprls = [section.as_slice()];
        let chpx_runs = [
            SyntheticChpxRun {
                cp_lim: 3,
                grpprl: Vec::new(),
            },
            SyntheticChpxRun {
                cp_lim: 4,
                grpprl: vec![0x3C, 0x08, 0x01],
            },
            SyntheticChpxRun {
                cp_lim: hidden_text_end,
                grpprl: Vec::new(),
            },
        ];
        let hidden = Document::open(&synth_doc_with_ccp_and_tables(
            hidden_text,
            "",
            0x00C1,
            0,
            0,
            [hidden_text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                plcf_sed_cps: Some(&section_cps),
                plcf_sed_sepx_grpprls: Some(&section_grpprls),
                chpx_runs: Some(&chpx_runs),
                ..SyntheticDocTables::default()
            },
        ))
        .unwrap();
        hidden.with_render_model_and_hints(|model, hints| {
            assert_eq!(single_paragraph_text(&model.blocks), "A\nB\nC");
            assert_eq!(hints.column_break_offsets, &[vec![1]]);
        });

        let main = "M\u{000e}N\r";
        let footnote = "F\u{000e}G\r";
        let combined = format!("{main}{footnote}");
        let main_len = main.encode_utf16().count() as u32;
        let footnote_len = footnote.encode_utf16().count() as u32;
        let stories = Document::open(&synth_doc_with_ccp_and_tables(
            &combined,
            "",
            0x00C1,
            0,
            0,
            [main_len, footnote_len, 0, 0, 0, 0],
            SyntheticDocTables::default(),
        ))
        .unwrap();
        stories.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 2);
            assert_eq!(hints.column_break_offsets, &[vec![1], vec![]]);
        });
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_manual_column_breaks_reach_preview_flow() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:r><w:t>left</w:t><w:br w:type=" column "/><w:t>right</w:t><w:br w:type="column"/><w:t>page two</w:t></w:r></w:p>
                <w:p><w:r><w:br w:type="column"/></w:r></w:p>
                <w:p><w:r><w:t>after orphan</w:t></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="3000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/><w:cols w:num="2"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let document = Document::open(&bytes).unwrap();
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        assert_eq!(document.model().blocks.len(), 3);
        document.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 3);
            assert_eq!(hints.column_break_offsets, &[vec![4, 10], vec![0], vec![]]);
        });

        let model_only = layout_pages_with_fonts(&document.model(), &fonts).unwrap();
        let opened = document.layout_pages_with_fonts(&fonts).unwrap();
        assert_eq!(model_only.pages, 1);
        assert_eq!(model_only.block_pages, vec![Some(1), Some(1), Some(1)]);
        assert_eq!(opened.pages, 2);
        assert_eq!(opened.block_pages, vec![Some(1), Some(2), Some(2)]);
        assert_eq!(opened, document.layout_pages_with_fonts(&fonts).unwrap());
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_column_break_capture_is_visible_top_level_body_only() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:r><w:t>A</w:t><w:br w:type="column"/><w:t>B</w:t></w:r></w:p>
                <w:p><w:r><w:t>C</w:t><w:br/><w:t>D</w:t></w:r><w:r><w:rPr><w:vanish/></w:rPr><w:br w:type="column"/></w:r></w:p>
                <w:tbl><w:tr><w:tc><w:p><w:r><w:t>E</w:t><w:br w:type="column"/><w:t>F</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
                <w:p><w:r><w:t>G</w:t><w:br w:type="column"/><w:t>H</w:t><w:br w:type="page"/><w:t>I</w:t><w:br w:type="column"/><w:t>J</w:t></w:r></w:p>
                <w:sectPr><w:cols w:num="2"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let document = Document::open(&bytes).unwrap();

        document.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 6);
            assert_eq!(
                hints.column_break_offsets,
                &[vec![1], vec![], vec![], vec![1], vec![], vec![1]]
            );
        });
        let model = document.model();
        let Block::Paragraph(first) = &model.blocks[0] else {
            panic!("first body block must be a paragraph");
        };
        let Block::Paragraph(second) = &model.blocks[1] else {
            panic!("second body block must be a paragraph");
        };
        assert_eq!(first.text(), "A\nB");
        assert_eq!(second.text(), "C\nD\n");
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_absolute_line_spacing_reaches_preview_flow() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:pPr><w:spacing w:line="800" w:lineRule="exact"/></w:pPr><w:r><w:t>exact</w:t></w:r></w:p>
                <w:p><w:pPr><w:spacing w:line="600" w:lineRule="atLeast"/></w:pPr><w:r><w:t>minimum</w:t></w:r></w:p>
                <w:tbl><w:tr><w:tc><w:p><w:pPr><w:spacing w:line="1000" w:lineRule="exact"/></w:pPr><w:r><w:t>cell ceiling</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
                <w:p><w:pPr><w:spacing w:line="400" w:lineRule="exact"/></w:pPr><w:r><w:t>before</w:t><w:br w:type="page"/><w:t>after</w:t></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="3000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let document = Document::open(&bytes).unwrap();
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        document.with_render_model_and_hints(|model, hints| {
            assert_eq!(model.blocks.len(), 6);
            assert_eq!(
                hints.line_spacing,
                &[
                    Some(crate::model::LineSpacingHint::Exact(40.0)),
                    Some(crate::model::LineSpacingHint::AtLeast(30.0)),
                    None,
                    Some(crate::model::LineSpacingHint::Exact(20.0)),
                    None,
                    Some(crate::model::LineSpacingHint::Exact(20.0)),
                ]
            );
            for paragraph in model.blocks.iter().filter_map(|block| match block {
                Block::Paragraph(paragraph) => Some(paragraph),
                _ => None,
            }) {
                assert_eq!(paragraph.props.spacing.line_pct, None);
            }
        });

        let opened_pdf = document.try_to_pdf_with_fonts(&fonts).unwrap();
        assert!(opened_pdf.starts_with(b"%PDF-"));
        assert_eq!(opened_pdf, document.try_to_pdf_with_fonts(&fonts).unwrap());
        assert_ne!(
            opened_pdf,
            render_pdf_with_fonts(&document.model(), &fonts),
            "the public model intentionally has no absolute line-spacing representation"
        );
        assert_eq!(
            document.layout_pages_with_fonts(&fonts).unwrap(),
            document.layout_pages_with_fonts(&fonts).unwrap()
        );
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_tabs_keep_page_margin_coordinates_under_paragraph_indents() {
        let make_document = |indent: &str| {
            let xml = format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                    <w:p><w:pPr>{indent}<w:tabs><w:tab w:val="left" w:pos="2000"/></w:tabs></w:pPr>
                        <w:r><w:tab/><w:t>B</w:t></w:r>
                    </w:p>
                    <w:sectPr><w:pgSz w:w="4400" w:h="2200"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
                </w:body></w:document>"#
            );
            Document::open(&minimal_docx(&xml)).unwrap()
        };
        let baseline = make_document("");
        let first_line = make_document(r#"<w:ind w:left="400" w:firstLine="200"/>"#);
        let hanging = make_document(r#"<w:ind w:left="800" w:hanging="400"/>"#);
        let Block::Paragraph(first_line_paragraph) = &first_line.model().blocks[0] else {
            panic!("synthetic DOCX must contain a paragraph");
        };
        assert_eq!(first_line_paragraph.props.indent.left_pt, Some(20.0));
        assert_eq!(first_line_paragraph.props.indent.first_line_pt, Some(10.0));
        let Block::Paragraph(hanging_paragraph) = &hanging.model().blocks[0] else {
            panic!("synthetic DOCX must contain a paragraph");
        };
        assert_eq!(hanging_paragraph.props.indent.left_pt, Some(40.0));
        assert_eq!(hanging_paragraph.props.indent.hanging_pt, Some(20.0));

        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let baseline_layout = baseline.layout_pages_with_fonts(&fonts).unwrap();
        for document in [&baseline, &first_line, &hanging] {
            let opened_layout = document.layout_pages_with_fonts(&fonts).unwrap();
            assert_eq!(opened_layout, baseline_layout);
            assert_eq!(
                opened_layout,
                document.layout_pages_with_fonts(&fonts).unwrap()
            );

            let opened_pdf = document.try_to_pdf_with_fonts(&fonts).unwrap();
            assert_eq!(opened_pdf, document.try_to_pdf_with_fonts(&fonts).unwrap());
            assert_ne!(
                opened_pdf,
                render_pdf_with_fonts(&document.model(), &fonts),
                "model-only rendering has no opened-document custom-tab sidecar"
            );
        }
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_rtl_start_tabs_reach_deterministic_rendering() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:p><w:pPr><w:bidi/><w:jc w:val="start"/><w:ind w:start="400" w:end="200"/>
                <w:tabs><w:tab w:val="start" w:pos="2000"/></w:tabs></w:pPr>
                <w:r><w:rPr><w:rtl/></w:rPr><w:t>א</w:t><w:tab/><w:t>ב</w:t></w:r>
            </w:p>
            <w:sectPr><w:pgSz w:w="4400" w:h="2200"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
        </w:body></w:document>"#;
        let document = Document::open(&minimal_docx(xml)).unwrap();
        let Block::Paragraph(paragraph) = &document.model().blocks[0] else {
            panic!("synthetic DOCX must contain a paragraph");
        };
        assert!(paragraph.props.bidi);
        assert_eq!(paragraph.props.align, Align::Right);
        assert_eq!(paragraph.props.indent.right_pt, Some(20.0));
        assert_eq!(paragraph.props.indent.left_pt, Some(10.0));
        let fonts = vec![rwml_fonts::noto_sans_hebrew_subset().to_vec()];

        let opened_pdf = document.try_to_pdf_with_fonts(&fonts).unwrap();
        assert_eq!(opened_pdf, document.try_to_pdf_with_fonts(&fonts).unwrap());
        assert_ne!(
            opened_pdf,
            render_pdf_with_fonts(&document.model(), &fonts),
            "model-only rendering has no opened-document custom-tab sidecar"
        );
        assert_eq!(
            document.layout_pages_with_fonts(&fonts).unwrap(),
            document.layout_pages_with_fonts(&fonts).unwrap()
        );
    }

    fn synthetic_paragraph_stylesheet(properties: &[(u16, u8)]) -> Vec<u8> {
        let mut grpprl = Vec::with_capacity(properties.len() * 3);
        for &(sprm, value) in properties {
            grpprl.extend_from_slice(&sprm.to_le_bytes());
            grpprl.push(value);
        }
        synthetic_paragraph_stylesheet_grpprl(&grpprl)
    }

    fn synthetic_paragraph_stylesheet_grpprl(properties: &[u8]) -> Vec<u8> {
        synthetic_paragraph_stylesheet_from_styles(&[
            (0, 0, 0x0FFF, "Normal", &[]),
            (1, 0x0FFE, 0, "Pagination", properties),
        ])
    }

    fn synthetic_paragraph_stylesheet_from_styles(
        styles: &[(u16, u16, u16, &str, &[u8])],
    ) -> Vec<u8> {
        fn push_style(
            stylesheet: &mut Vec<u8>,
            istd: u16,
            sti: u16,
            base: u16,
            name: &str,
            properties: &[u8],
        ) {
            let mut std = vec![0u8; 10];
            std[0..2].copy_from_slice(&(sti & 0x0FFF).to_le_bytes());
            std[2..4].copy_from_slice(&(1 | ((base & 0x0FFF) << 4)).to_le_bytes());
            std[4..6].copy_from_slice(&2u16.to_le_bytes());

            let name_units: Vec<u16> = name.encode_utf16().collect();
            std.extend_from_slice(&(name_units.len() as u16).to_le_bytes());
            for unit in name_units {
                std.extend_from_slice(&unit.to_le_bytes());
            }
            std.extend_from_slice(&0u16.to_le_bytes());

            let mut papx = Vec::with_capacity(2 + properties.len());
            papx.extend_from_slice(&istd.to_le_bytes());
            papx.extend_from_slice(properties);
            std.extend_from_slice(&(papx.len() as u16).to_le_bytes());
            std.extend_from_slice(&papx);
            if papx.len() % 2 == 1 {
                std.push(0);
            }
            std.extend_from_slice(&0u16.to_le_bytes());

            let cb_std = std.len() as u16;
            std[6..8].copy_from_slice(&cb_std.to_le_bytes());
            stylesheet.extend_from_slice(&cb_std.to_le_bytes());
            stylesheet.extend_from_slice(&std);
        }

        let mut stylesheet = vec![0u8; 20];
        stylesheet[0..2].copy_from_slice(&18u16.to_le_bytes());
        stylesheet[2..4].copy_from_slice(&(styles.len() as u16).to_le_bytes());
        stylesheet[4..6].copy_from_slice(&10u16.to_le_bytes());
        stylesheet[6..8].copy_from_slice(&1u16.to_le_bytes());
        for &(istd, sti, base, name, properties) in styles {
            push_style(&mut stylesheet, istd, sti, base, name, properties);
        }
        stylesheet
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn push_legacy_pagination_fixture_spacing(grpprl: &mut Vec<u8>) {
        push_paragraph_spacing_twips(grpprl, 0xA414, 120);
        push_paragraph_line_spacing(grpprl, 324, 1);
    }

    #[cfg(feature = "render")]
    fn legacy_paragraph_style_pagination_doc(
        line_count: usize,
        style_properties: &[(u16, u8)],
        direct_properties: &[(u16, u8)],
    ) -> Vec<u8> {
        const SEED_COUNT: usize = 32;
        let mut text = String::new();
        for index in 0..SEED_COUNT {
            text.push_str(&format!("seed {index}\r"));
        }
        let seed_end = text.encode_utf16().count() as u32;
        for line_index in 0..line_count {
            if line_index > 0 {
                text.push('\u{b}');
            }
            text.push_str(&format!("target line {line_index}"));
        }
        text.push('\r');
        let target_end = text.encode_utf16().count() as u32;
        for index in 0..25 {
            text.push_str(&format!("after {index}\r"));
        }
        let text_end = text.encode_utf16().count() as u32;

        let mut target_grpprl = vec![0x00, 0x46, 0x01, 0x00];
        push_legacy_pagination_fixture_spacing(&mut target_grpprl);
        for &(sprm, value) in direct_properties {
            target_grpprl.extend_from_slice(&sprm.to_le_bytes());
            target_grpprl.push(value);
        }
        let mut seed_grpprl = vec![0x31, 0x24, 0x00];
        push_legacy_pagination_fixture_spacing(&mut seed_grpprl);
        let mut after_grpprl = vec![0x31, 0x24, 0x00];
        push_legacy_pagination_fixture_spacing(&mut after_grpprl);
        let runs = [
            SyntheticPapxRun {
                cp_lim: seed_end,
                grpprl: seed_grpprl,
            },
            SyntheticPapxRun {
                cp_lim: target_end,
                grpprl: target_grpprl,
            },
            SyntheticPapxRun {
                cp_lim: text_end,
                grpprl: after_grpprl,
            },
        ];
        let stylesheet = synthetic_paragraph_stylesheet(style_properties);
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                stylesheet: Some(&stylesheet),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(any(feature = "docx", feature = "render"))]
    fn legacy_row_pagination_doc(row_properties: &[(u16, u8)]) -> Vec<u8> {
        let mut text = String::new();
        for index in 0..32 {
            text.push_str(&format!("seed {index}\r"));
        }
        let table_start = text.encode_utf16().count() as u32;
        for index in 0..12 {
            text.push_str(&format!("row {index}\r"));
        }
        text.push('\u{7}');
        let cell_end = text.encode_utf16().count() as u32;
        text.push('\u{7}');
        let row_end = text.encode_utf16().count() as u32;
        for index in 0..25 {
            text.push_str(&format!("after {index}\r"));
        }
        let text_end = text.encode_utf16().count() as u32;

        let mut row_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
            0x17, 0x24, 0x01, // sprmPFTtp
            0x08, 0xD6, 0x1A, 0x00, // sprmTDefTable, cb=26
            0x01, // one cell
            0x00, 0x00, 0xD0, 0x07, // cell boundaries 0..2000 twips
        ];
        row_grpprl.extend_from_slice(&[0u8; 20]); // one TC80
        for &(sprm, value) in row_properties {
            row_grpprl.extend_from_slice(&sprm.to_le_bytes());
            row_grpprl.push(value);
        }

        let mut seed_grpprl = Vec::new();
        push_legacy_pagination_fixture_spacing(&mut seed_grpprl);
        let mut cell_grpprl = vec![0x16, 0x24, 0x01];
        push_paragraph_line_spacing(&mut cell_grpprl, 324, 1);
        let mut after_grpprl = Vec::new();
        push_legacy_pagination_fixture_spacing(&mut after_grpprl);
        let runs = [
            SyntheticPapxRun {
                cp_lim: table_start,
                grpprl: seed_grpprl,
            },
            SyntheticPapxRun {
                cp_lim: cell_end,
                grpprl: cell_grpprl,
            },
            SyntheticPapxRun {
                cp_lim: row_end,
                grpprl: row_grpprl,
            },
            SyntheticPapxRun {
                cp_lim: text_end,
                grpprl: after_grpprl,
            },
        ];
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(feature = "render")]
    fn legacy_paragraph_pagination_doc(
        seed_count: usize,
        paragraphs: &[(usize, &[(u16, u8)])],
        after_count: usize,
    ) -> Vec<u8> {
        let mut text = String::new();
        for index in 0..seed_count {
            text.push_str(&format!("seed {index}\r"));
        }
        let seed_end = text.encode_utf16().count() as u32;
        let mut seed_grpprl = vec![0x31, 0x24, 0x00];
        push_legacy_pagination_fixture_spacing(&mut seed_grpprl);
        let mut runs = vec![SyntheticPapxRun {
            cp_lim: seed_end,
            grpprl: seed_grpprl,
        }];

        for (paragraph_index, &(line_count, properties)) in paragraphs.iter().enumerate() {
            for line_index in 0..line_count {
                if line_index > 0 {
                    text.push('\u{b}');
                }
                text.push_str(&format!("target {paragraph_index} line {line_index}"));
            }
            text.push('\r');
            let mut grpprl = Vec::with_capacity(properties.len() * 3);
            push_legacy_pagination_fixture_spacing(&mut grpprl);
            for &(sprm, value) in properties {
                grpprl.extend_from_slice(&sprm.to_le_bytes());
                grpprl.push(value);
            }
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl,
            });
        }

        for index in 0..after_count {
            text.push_str(&format!("after {index}\r"));
        }
        let text_end = text.encode_utf16().count() as u32;
        let mut after_grpprl = vec![0x31, 0x24, 0x00];
        push_legacy_pagination_fixture_spacing(&mut after_grpprl);
        runs.push(SyntheticPapxRun {
            cp_lim: text_end,
            grpprl: after_grpprl,
        });
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(feature = "render")]
    fn legacy_table_cell_pagination_doc(
        seed_count: usize,
        line_count: usize,
        cell_properties: &[(u16, u8)],
        after_count: usize,
    ) -> Vec<u8> {
        let mut text = String::new();
        for index in 0..seed_count {
            text.push_str(&format!("seed {index}\r"));
        }
        let seed_end = text.encode_utf16().count() as u32;
        for line_index in 0..line_count {
            if line_index > 0 {
                text.push('\u{b}');
            }
            text.push_str(&format!("cell line {line_index}"));
        }
        text.push('\u{7}');
        let cell_end = text.encode_utf16().count() as u32;
        text.push('\u{7}');
        let row_end = text.encode_utf16().count() as u32;
        for index in 0..after_count {
            text.push_str(&format!("after {index}\r"));
        }
        let text_end = text.encode_utf16().count() as u32;

        let mut seed_grpprl = vec![0x31, 0x24, 0x00];
        push_legacy_pagination_fixture_spacing(&mut seed_grpprl);
        let mut cell_grpprl = vec![0x16, 0x24, 0x01];
        push_paragraph_line_spacing(&mut cell_grpprl, 324, 1);
        for &(sprm, value) in cell_properties {
            cell_grpprl.extend_from_slice(&sprm.to_le_bytes());
            cell_grpprl.push(value);
        }
        let mut row_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
            0x17, 0x24, 0x01, // sprmPFTtp
            0x08, 0xD6, 0x1A, 0x00, // sprmTDefTable, cb=26
            0x01, // one cell
            0x00, 0x00, 0xD0, 0x07, // cell boundaries 0..2000 twips
        ];
        row_grpprl.extend_from_slice(&[0u8; 20]);
        let mut after_grpprl = vec![0x31, 0x24, 0x00];
        push_legacy_pagination_fixture_spacing(&mut after_grpprl);
        let runs = [
            SyntheticPapxRun {
                cp_lim: seed_end,
                grpprl: seed_grpprl,
            },
            SyntheticPapxRun {
                cp_lim: cell_end,
                grpprl: cell_grpprl,
            },
            SyntheticPapxRun {
                cp_lim: row_end,
                grpprl: row_grpprl,
            },
            SyntheticPapxRun {
                cp_lim: text_end,
                grpprl: after_grpprl,
            },
        ];
        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(feature = "docx")]
    fn legacy_table_cell_pagination_conversion_doc() -> Vec<u8> {
        const KEEP_LINES: u16 = 0x2405;
        const KEEP_NEXT: u16 = 0x2406;
        const PAGE_BREAK_BEFORE: u16 = 0x2407;
        const WIDOW_CONTROL: u16 = 0x2431;

        let properties = |values: &[(u16, u8)]| {
            let mut grpprl = Vec::with_capacity(values.len() * 3);
            for &(sprm, value) in values {
                grpprl.extend_from_slice(&sprm.to_le_bytes());
                grpprl.push(value);
            }
            grpprl
        };
        let style_properties = properties(&[
            (KEEP_LINES, 1),
            (KEEP_NEXT, 1),
            (PAGE_BREAK_BEFORE, 1),
            (WIDOW_CONTROL, 0),
        ]);
        let stylesheet = synthetic_paragraph_stylesheet_grpprl(&style_properties);

        let mut combined = properties(&[
            (KEEP_LINES, 1),
            (KEEP_NEXT, 1),
            (PAGE_BREAK_BEFORE, 1),
            (WIDOW_CONTROL, 0),
        ]);
        push_paragraph_line_spacing(&mut combined, 0xFF9C, 0);
        let mut style_cleared = vec![0x00, 0x46, 0x01, 0x00];
        style_cleared.extend(properties(&[
            (KEEP_LINES, 0),
            (KEEP_NEXT, 0),
            (PAGE_BREAK_BEFORE, 0),
            (WIDOW_CONTROL, 1),
        ]));
        let paragraphs = [
            ("Default", Vec::new()),
            ("KeepNext", properties(&[(KEEP_NEXT, 1)])),
            ("KeepLines", properties(&[(KEEP_LINES, 1)])),
            ("WidowOff", properties(&[(WIDOW_CONTROL, 0)])),
            ("Combined", combined),
            ("StyleAll", vec![0x00, 0x46, 0x01, 0x00]),
            ("StyleCleared", style_cleared),
        ];

        let mut text = String::new();
        let mut runs = Vec::new();
        let paragraph_count = paragraphs.len();
        for (index, (label, properties)) in paragraphs.into_iter().enumerate() {
            text.push_str(label);
            text.push(if index + 1 == paragraph_count {
                '\u{7}'
            } else {
                '\r'
            });
            let mut grpprl = vec![0x16, 0x24, 0x01];
            grpprl.extend(properties);
            runs.push(SyntheticPapxRun {
                cp_lim: text.encode_utf16().count() as u32,
                grpprl,
            });
        }
        text.push('\u{7}');
        let text_end = text.encode_utf16().count() as u32;
        let mut row_grpprl = vec![
            0x16, 0x24, 0x01, // sprmPFInTable
            0x17, 0x24, 0x01, // sprmPFTtp
            0x08, 0xD6, 0x1A, 0x00, // sprmTDefTable, cb=26
            0x01, // one cell
            0x00, 0x00, 0xD0, 0x07, // cell boundaries 0..2000 twips
        ];
        row_grpprl.extend_from_slice(&[0u8; 20]);
        row_grpprl.extend_from_slice(&[0x66, 0x34, 0x01]); // sprmTFCantSplit
        runs.push(SyntheticPapxRun {
            cp_lim: text_end,
            grpprl: row_grpprl,
        });

        synth_doc_with_ccp_and_tables(
            &text,
            "",
            0x00C1,
            0,
            0,
            [text_end, 0, 0, 0, 0, 0],
            SyntheticDocTables {
                stylesheet: Some(&stylesheet),
                papx_runs: Some(&runs),
                ..SyntheticDocTables::default()
            },
        )
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_layout_uses_inherited_style_pagination_with_direct_off() {
        const TARGET_INDEX: usize = 32;
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let inherited_break = Document::open(&legacy_paragraph_style_pagination_doc(
            1,
            &[(0x2407, 1), (0x2431, 0)],
            &[],
        ))
        .unwrap();
        let direct_break_off = Document::open(&legacy_paragraph_style_pagination_doc(
            1,
            &[(0x2407, 1), (0x2431, 0)],
            &[(0x2407, 0)],
        ))
        .unwrap();
        let inherited_keep = Document::open(&legacy_paragraph_style_pagination_doc(
            12,
            &[(0x2405, 1), (0x2431, 0)],
            &[],
        ))
        .unwrap();
        let direct_keep_off = Document::open(&legacy_paragraph_style_pagination_doc(
            12,
            &[(0x2405, 1), (0x2431, 0)],
            &[(0x2405, 0), (0x2431, 0)],
        ))
        .unwrap();

        let break_on_model = inherited_break.model();
        let break_off_model = direct_break_off.model();
        let Block::Paragraph(break_on) = &break_on_model.blocks[TARGET_INDEX] else {
            panic!("target must be a paragraph");
        };
        let Block::Paragraph(break_off) = &break_off_model.blocks[TARGET_INDEX] else {
            panic!("target must be a paragraph");
        };
        assert!(break_on.props.page_break_before);
        assert!(!break_off.props.page_break_before);

        let layout = |document: &Document| document.layout_pages_with_fonts(&fonts).unwrap();
        assert_eq!(
            (
                layout(&inherited_break).block_pages[TARGET_INDEX],
                layout(&direct_break_off).block_pages[TARGET_INDEX],
                layout(&inherited_keep).block_pages[TARGET_INDEX],
                layout(&direct_keep_off).block_pages[TARGET_INDEX],
            ),
            (Some(2), Some(1), Some(2), Some(1))
        );
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_layout_uses_direct_paragraph_pagination_hints() {
        const SEED_COUNT: usize = 32;
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let split = Document::open(&legacy_paragraph_pagination_doc(
            SEED_COUNT,
            &[(12, &[(0x2405, 0), (0x2431, 0)])],
            25,
        ))
        .unwrap();
        let kept = Document::open(&legacy_paragraph_pagination_doc(
            SEED_COUNT,
            &[(12, &[(0x2405, 1), (0x2431, 0)])],
            25,
        ))
        .unwrap();
        let widow_off = Document::open(&legacy_paragraph_pagination_doc(
            SEED_COUNT,
            &[(3, &[(0x2431, 0)])],
            25,
        ))
        .unwrap();
        let widow_default = Document::open(&legacy_paragraph_pagination_doc(
            SEED_COUNT,
            &[(3, &[])],
            25,
        ))
        .unwrap();
        let follow_off = Document::open(&legacy_paragraph_pagination_doc(
            SEED_COUNT,
            &[
                (1, &[(0x2406, 0), (0x2431, 0)]),
                (12, &[(0x2405, 1), (0x2431, 0)]),
            ],
            25,
        ))
        .unwrap();
        let follow_on = Document::open(&legacy_paragraph_pagination_doc(
            SEED_COUNT,
            &[
                (1, &[(0x2406, 1), (0x2431, 0)]),
                (12, &[(0x2405, 1), (0x2431, 0)]),
            ],
            25,
        ))
        .unwrap();
        let page_break_off = Document::open(&legacy_paragraph_pagination_doc(
            SEED_COUNT,
            &[(1, &[(0x2407, 0), (0x2431, 0)])],
            25,
        ))
        .unwrap();
        let page_break_on = Document::open(&legacy_paragraph_pagination_doc(
            SEED_COUNT,
            &[(1, &[(0x2407, 1), (0x2431, 0)])],
            25,
        ))
        .unwrap();

        let layout = |document: &Document| document.layout_pages_with_fonts(&fonts).unwrap();
        let split_layout = layout(&split);
        let kept_layout = layout(&kept);
        let widow_off_layout = layout(&widow_off);
        let widow_default_layout = layout(&widow_default);
        let follow_off_layout = layout(&follow_off);
        let follow_on_layout = layout(&follow_on);
        let page_break_off_layout = layout(&page_break_off);
        let page_break_on_layout = layout(&page_break_on);

        assert_eq!(
            (
                split_layout.pages,
                split_layout.block_pages[SEED_COUNT],
                kept_layout.pages,
                kept_layout.block_pages[SEED_COUNT],
            ),
            (2, Some(1), 3, Some(2))
        );
        assert_eq!(
            (
                widow_off_layout.pages,
                widow_off_layout.block_pages[SEED_COUNT],
                widow_default_layout.pages,
                widow_default_layout.block_pages[SEED_COUNT],
            ),
            (2, Some(1), 2, Some(2))
        );
        assert_eq!(
            (
                follow_off_layout.pages,
                follow_off_layout.block_pages[SEED_COUNT],
                follow_on_layout.pages,
                follow_on_layout.block_pages[SEED_COUNT],
            ),
            (3, Some(1), 3, Some(2))
        );
        assert_eq!(
            (
                page_break_off_layout.block_pages[SEED_COUNT],
                page_break_on_layout.block_pages[SEED_COUNT],
            ),
            (Some(1), Some(2))
        );

        let raw_kept_layout = layout_pages_with_fonts(&kept.model(), &fonts).unwrap();
        let raw_widow_layout = layout_pages_with_fonts(&widow_default.model(), &fonts).unwrap();
        let raw_follow_layout = layout_pages_with_fonts(&follow_on.model(), &fonts).unwrap();
        assert_eq!(raw_kept_layout.block_pages[SEED_COUNT], Some(1));
        assert_eq!(raw_widow_layout.block_pages[SEED_COUNT], Some(1));
        assert_eq!(raw_follow_layout.block_pages[SEED_COUNT], Some(1));

        let page_break_off_model = page_break_off.model();
        let page_break_on_model = page_break_on.model();
        let Block::Paragraph(off_paragraph) = &page_break_off_model.blocks[SEED_COUNT] else {
            panic!("target must be a paragraph");
        };
        let Block::Paragraph(on_paragraph) = &page_break_on_model.blocks[SEED_COUNT] else {
            panic!("target must be a paragraph");
        };
        assert!(!off_paragraph.props.page_break_before);
        assert!(on_paragraph.props.page_break_before);
        let raw_page_break_layout = layout_pages_with_fonts(&page_break_on_model, &fonts).unwrap();
        assert_eq!(raw_page_break_layout.block_pages[SEED_COUNT], Some(2));
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_layout_uses_direct_table_cell_pagination_hints() {
        const SEED_COUNT: usize = 32;
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let split = Document::open(&legacy_table_cell_pagination_doc(
            SEED_COUNT,
            12,
            &[(0x2405, 0), (0x2431, 0)],
            25,
        ))
        .unwrap();
        let kept = Document::open(&legacy_table_cell_pagination_doc(
            SEED_COUNT,
            12,
            &[(0x2405, 1), (0x2431, 0)],
            25,
        ))
        .unwrap();
        let over_tall = Document::open(&legacy_table_cell_pagination_doc(
            0,
            100,
            &[(0x2405, 1), (0x2431, 0)],
            0,
        ))
        .unwrap();

        let first_cell_hint = |document: &Document| match &document.backend {
            Backend::Doc(state) => legacy_build_output_from_doc_state(state)
                .table_cell_pagination
                .into_iter()
                .find(|table| !table.is_empty())
                .and_then(|table| {
                    table
                        .first()
                        .and_then(|row| row.first())
                        .and_then(|cell| cell.first())
                        .copied()
                        .flatten()
                })
                .expect("synthetic legacy table-cell hint"),
            #[cfg(feature = "docx")]
            Backend::Docx(_) => unreachable!("synthetic fixture is OLE"),
        };
        assert_eq!(
            first_cell_hint(&split),
            crate::model::PaginationHint {
                widow_control: false,
                ..crate::model::PaginationHint::default()
            }
        );
        assert_eq!(
            first_cell_hint(&kept),
            crate::model::PaginationHint {
                keep_lines: true,
                widow_control: false,
                ..crate::model::PaginationHint::default()
            }
        );

        let model_pages = layout_pages_with_fonts(&split.model(), &fonts)
            .unwrap()
            .pages;
        let split_pages = split.layout_pages_with_fonts(&fonts).unwrap().pages;
        let kept_pages = kept.layout_pages_with_fonts(&fonts).unwrap().pages;
        assert_eq!((model_pages, split_pages, kept_pages), (3, 2, 3));

        let over_tall_layout = over_tall.layout_pages_with_fonts(&fonts).unwrap();
        assert!(
            over_tall_layout.pages > 1,
            "an over-tall kept cell paragraph must split and make progress"
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_table_cell_pagination_roundtrips_to_docx() {
        let legacy = Document::open(&legacy_table_cell_pagination_conversion_doc()).unwrap();
        let converted = legacy.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");

        let default = docx_paragraph_with_text(&document_xml, "Default");
        assert!(!default.contains("<w:keepNext"), "{default}");
        assert!(!default.contains("<w:keepLines"), "{default}");
        assert!(!default.contains("<w:widowControl"), "{default}");
        let keep_next = docx_paragraph_with_text(&document_xml, "KeepNext");
        assert!(keep_next.contains("<w:keepNext/>"), "{keep_next}");
        let keep_lines = docx_paragraph_with_text(&document_xml, "KeepLines");
        assert!(keep_lines.contains("<w:keepLines/>"), "{keep_lines}");
        let widow_off = docx_paragraph_with_text(&document_xml, "WidowOff");
        assert!(
            widow_off.contains(r#"<w:widowControl w:val="0"/>"#),
            "{widow_off}"
        );
        let combined = docx_paragraph_with_text(&document_xml, "Combined");
        assert!(
            combined.contains(concat!(
                "<w:pPr><w:keepNext/><w:keepLines/><w:pageBreakBefore/>",
                r#"<w:widowControl w:val="0"/>"#,
                r#"<w:spacing w:before="0" w:after="0" w:line="100" w:lineRule="exact"/>"#,
            )),
            "{combined}"
        );
        let style_all = docx_paragraph_with_text(&document_xml, "StyleAll");
        assert!(
            style_all.contains(concat!(
                r#"<w:pPr><w:pStyle w:val="Heading1"/><w:keepNext/><w:keepLines/>"#,
                r#"<w:pageBreakBefore/><w:widowControl w:val="0"/>"#,
            )),
            "{style_all}"
        );
        let style_cleared = docx_paragraph_with_text(&document_xml, "StyleCleared");
        assert!(!style_cleared.contains("<w:keepNext"), "{style_cleared}");
        assert!(!style_cleared.contains("<w:keepLines"), "{style_cleared}");
        assert!(
            !style_cleared.contains("<w:pageBreakBefore"),
            "{style_cleared}"
        );
        assert!(
            !style_cleared.contains("<w:widowControl"),
            "{style_cleared}"
        );
        assert!(
            document_xml.contains("<w:tr><w:trPr><w:cantSplit/></w:trPr>"),
            "{document_xml}"
        );
        assert_eq!(converted, legacy.to_docx());

        let model_only_xml = docx_part(&write_docx(&legacy.model()), "word/document.xml");
        assert!(!model_only_xml.contains("<w:keepNext"));
        assert!(!model_only_xml.contains("<w:keepLines"));
        assert!(!model_only_xml.contains("<w:widowControl"));
        assert!(!model_only_xml.contains("<w:cantSplit"));
        assert!(!model_only_xml.contains(r#"w:lineRule="exact""#));

        let docx_backed = Document::open(&converted).unwrap();
        let docx_backed_xml = docx_part(&docx_backed.to_docx(), "word/document.xml");
        assert_eq!(docx_backed_xml.matches("<w:keepNext").count(), 3);
        assert_eq!(docx_backed_xml.matches("<w:keepLines").count(), 3);
        assert_eq!(docx_backed_xml.matches("<w:widowControl").count(), 3);
        assert_eq!(docx_backed_xml.matches("<w:cantSplit").count(), 1);
        assert_eq!(docx_backed_xml.matches(r#"w:lineRule="exact""#).count(), 1);

        #[cfg(feature = "render")]
        {
            let Backend::Docx(state) = docx_backed.backend else {
                panic!("converted document must use the DOCX backend");
            };
            let hints = state
                .table_cell_pagination
                .into_iter()
                .find(|table| !table.is_empty())
                .and_then(|table| table.first().cloned())
                .and_then(|row| row.first().cloned())
                .expect("converted table-cell pagination hints");
            let widow_on = crate::model::PaginationHint {
                widow_control: true,
                ..crate::model::PaginationHint::default()
            };
            assert_eq!(
                hints,
                vec![
                    Some(widow_on),
                    Some(crate::model::PaginationHint {
                        keep_next: true,
                        ..widow_on
                    }),
                    Some(crate::model::PaginationHint {
                        keep_lines: true,
                        ..widow_on
                    }),
                    Some(crate::model::PaginationHint::default()),
                    Some(crate::model::PaginationHint {
                        keep_next: true,
                        keep_lines: true,
                        widow_control: false,
                    }),
                    Some(crate::model::PaginationHint {
                        keep_next: true,
                        keep_lines: true,
                        widow_control: false,
                    }),
                    Some(widow_on),
                ]
            );
        }
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_docx_body_layout_hints_roundtrip_through_fresh_conversion() {
        let source = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:pPr><w:keepNext/><w:keepLines/><w:widowControl w:val="0"/><w:spacing w:line="200" w:lineRule="exact"/></w:pPr><w:r><w:t>top</w:t></w:r></w:p>
                <w:tbl><w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc>
                    <w:p><w:pPr><w:keepNext/><w:keepLines/><w:widowControl w:val="0"/><w:spacing w:line="400" w:lineRule="atLeast"/></w:pPr><w:r><w:t>cell</w:t><w:br w:type="page"/><w:t>cell after</w:t></w:r></w:p>
                    <w:tbl><w:tr><w:tc><w:p><w:pPr><w:keepNext/><w:spacing w:line="600" w:lineRule="exact"/></w:pPr><w:r><w:t>nested</w:t><w:br w:type="page"/><w:t>nested after</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
                </w:tc></w:tr></w:tbl>
                <w:sectPr/>
            </w:body></w:document>"#,
        );
        let opened = Document::open(&source).unwrap();
        let source_model = opened.model();
        let model_only = write_docx(&source_model);
        let normalized_blocks = Document::open(&model_only).unwrap().model().blocks;
        let converted = opened.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");
        let top = docx_paragraph_with_text(&document_xml, "top");

        assert!(top.contains("<w:keepNext/>"), "{top}");
        assert!(top.contains("<w:keepLines/>"), "{top}");
        assert!(top.contains(r#"<w:widowControl w:val="0"/>"#), "{top}");
        assert!(top.contains(r#"w:line="200" w:lineRule="exact""#), "{top}");
        assert!(document_xml.contains("<w:cantSplit/>"), "{document_xml}");
        let cell = docx_paragraph_with_text(&document_xml, "cell");
        assert!(cell.contains("<w:keepNext/>"), "{cell}");
        assert!(cell.contains("<w:keepLines/>"), "{cell}");
        assert!(cell.contains(r#"<w:widowControl w:val="0"/>"#), "{cell}");
        assert!(
            cell.contains(r#"w:line="400" w:lineRule="atLeast""#),
            "{cell}"
        );
        let nested = docx_paragraph_with_text(&document_xml, "nested");
        assert!(nested.contains("<w:keepNext/>"), "{nested}");
        assert!(
            nested.contains(r#"w:line="600" w:lineRule="exact""#),
            "{nested}"
        );
        assert_eq!(
            document_xml.matches(r#"<w:br w:type="page"/>"#).count(),
            2,
            "{document_xml}"
        );
        assert!(!document_xml.contains("<w:tabs>"), "{document_xml}");
        assert_eq!(converted, opened.to_docx());
        assert_eq!(opened.model(), source_model);

        let model_only_xml = docx_part(&model_only, "word/document.xml");
        assert_eq!(
            model_only_xml.matches(r#"<w:br w:type="page"/>"#).count(),
            2,
            "{model_only_xml}"
        );
        assert!(!model_only_xml.contains("<w:keepNext"));
        assert!(!model_only_xml.contains("<w:keepLines"));
        assert!(!model_only_xml.contains("<w:widowControl"));
        assert!(!model_only_xml.contains("<w:cantSplit"));
        assert!(!model_only_xml.contains(r#"w:lineRule="exact""#));
        assert!(!model_only_xml.contains(r#"w:lineRule="atLeast""#));
        assert!(!model_only_xml.contains("<w:tabs>"));

        let reopened = Document::open(&converted).unwrap();
        let Backend::Docx(state) = reopened.backend else {
            panic!("converted document must use the DOCX backend");
        };
        assert_eq!(state.model.blocks, normalized_blocks);
        assert_eq!(
            state.pagination_hints[0],
            crate::model::PaginationHint {
                keep_next: true,
                keep_lines: true,
                widow_control: false,
            }
        );
        assert_eq!(
            state.line_spacing_hints[0],
            Some(crate::model::LineSpacingHint::Exact(10.0))
        );
        let table_index = state
            .model
            .blocks
            .iter()
            .position(|block| matches!(block, Block::Table(_)))
            .expect("converted top-level table");
        assert!(state.table_row_pagination[table_index][0].cant_split);
        assert_eq!(
            state.table_cell_pagination[table_index][0][0][0],
            Some(crate::model::PaginationHint {
                keep_next: true,
                keep_lines: true,
                widow_control: false,
            })
        );
        assert_eq!(
            state.table_cell_line_spacing[table_index][0][0][0],
            Some(crate::model::LineSpacingHint::AtLeast(20.0))
        );
        let nested_hints = state.table_nested_pagination[table_index][0][0][3]
            .as_ref()
            .expect("converted nested table hints");
        assert!(nested_hints.cells[0][0][0]
            .as_ref()
            .is_some_and(|hint| hint.keep_next));
        assert_eq!(
            nested_hints.cell_line_spacing[0][0][0],
            Some(crate::model::LineSpacingHint::Exact(30.0))
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_docx_direct_body_tab_stops_roundtrip_through_fresh_conversion() {
        let source = minimal_docx_with_styles(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:pPr><w:pStyle w:val="Normal"/><w:keepNext/><w:shd w:val="clear" w:fill="DDEEFF"/><w:tabs>
                    <w:tab w:val="clear" w:pos="720"/>
                    <w:tab w:val="right" w:pos="2160" w:leader="underscore"/>
                    <w:tab w:val="decimal" w:pos="2880" w:leader="heavy"/>
                    <w:tab w:val="bar" w:pos="3600" w:leader="middleDot"/>
                    <w:tab w:val="left" w:pos="4320" w:leader="dot"/>
                </w:tabs><w:bidi/><w:spacing w:line="240" w:lineRule="auto"/><w:ind w:left="360"/><w:jc w:val="right"/></w:pPr><w:r><w:t>top-tabs</w:t><w:tab/><w:t>tail</w:t></w:r></w:p>
                <w:tbl><w:tr><w:tc>
                    <w:p><w:pPr><w:tabs><w:tab w:val="clear" w:pos="720"/><w:tab w:val="center" w:pos="900"/><w:tab w:val="clear" w:pos="1440"/></w:tabs></w:pPr><w:r><w:t>cell-tabs</w:t><w:tab/><w:t>tail</w:t></w:r></w:p>
                    <w:tbl><w:tr><w:tc><w:p><w:pPr><w:tabs><w:tab w:val="right" w:pos="1800" w:leader="dot"/></w:tabs></w:pPr><w:r><w:t>nested-tabs</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
                </w:tc></w:tr></w:tbl>
                <w:sectPr/>
            </w:body></w:document>"#,
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/><w:pPr><w:tabs>
                    <w:tab w:val="left" w:pos="720" w:leader="dot"/>
                    <w:tab w:val="center" w:pos="1440" w:leader="hyphen"/>
                </w:tabs></w:pPr></w:style>
            </w:styles>"#,
        );
        let opened = Document::open(&source).unwrap();
        let converted = opened.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");
        let top = docx_paragraph_with_text(&document_xml, "top-tabs");

        let top_tabs = concat!(
            r#"<w:tabs><w:tab w:val="center" w:pos="1440" w:leader="hyphen"/>"#,
            r#"<w:tab w:val="right" w:pos="2160" w:leader="underscore"/>"#,
            r#"<w:tab w:val="decimal" w:pos="2880" w:leader="heavy"/>"#,
            r#"<w:tab w:val="bar" w:pos="3600" w:leader="middleDot"/>"#,
            r#"<w:tab w:val="left" w:pos="4320" w:leader="dot"/></w:tabs>"#,
        );
        assert!(top.contains(top_tabs), "{top}");
        let positions = [
            "<w:pStyle",
            "<w:keepNext",
            "<w:shd",
            "<w:tabs>",
            "<w:bidi",
            "<w:spacing",
            "<w:ind",
            "<w:jc",
        ]
        .map(|marker| {
            top.find(marker)
                .unwrap_or_else(|| panic!("missing {marker}: {top}"))
        });
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]), "{top}");

        let cell = docx_paragraph_with_text(&document_xml, "cell-tabs");
        assert!(
            cell.contains(r#"<w:tabs><w:tab w:val="center" w:pos="900"/></w:tabs>"#),
            "{cell}"
        );
        let nested = docx_paragraph_with_text(&document_xml, "nested-tabs");
        assert!(
            nested.contains(r#"<w:tab w:val="right" w:pos="1800" w:leader="dot"/>"#),
            "{nested}"
        );
        assert_eq!(converted, opened.to_docx());

        let model_only_xml = docx_part(&write_docx(&opened.model()), "word/document.xml");
        assert!(!model_only_xml.contains("<w:tabs>"), "{model_only_xml}");

        let reopened = Document::open(&converted).unwrap();
        let Backend::Docx(state) = reopened.backend else {
            panic!("converted document must use the DOCX backend");
        };
        assert_eq!(
            state.tab_stops[0],
            vec![
                crate::model::TabStop {
                    position_pt: 72.0,
                    alignment: crate::model::TabAlignment::Center,
                    leader: crate::model::TabLeader::Hyphen,
                },
                crate::model::TabStop {
                    position_pt: 108.0,
                    alignment: crate::model::TabAlignment::Right,
                    leader: crate::model::TabLeader::Underscore,
                },
                crate::model::TabStop {
                    position_pt: 144.0,
                    alignment: crate::model::TabAlignment::Decimal,
                    leader: crate::model::TabLeader::Heavy,
                },
                crate::model::TabStop {
                    position_pt: 180.0,
                    alignment: crate::model::TabAlignment::Bar,
                    leader: crate::model::TabLeader::MiddleDot,
                },
                crate::model::TabStop {
                    position_pt: 216.0,
                    alignment: crate::model::TabAlignment::Left,
                    leader: crate::model::TabLeader::Dot,
                },
            ]
        );
        let table_index = state
            .model
            .blocks
            .iter()
            .position(|block| matches!(block, Block::Table(_)))
            .expect("converted top-level table");
        assert_eq!(
            state.table_cell_tab_stops[table_index][0][0][0],
            vec![crate::model::TabStop {
                position_pt: 45.0,
                alignment: crate::model::TabAlignment::Center,
                leader: crate::model::TabLeader::None,
            }]
        );
        let nested_hints = state.table_nested_pagination[table_index][0][0][1]
            .as_ref()
            .expect("converted nested table hints");
        assert_eq!(
            nested_hints.cell_tabs[0][0][0],
            vec![
                crate::model::TabStop {
                    position_pt: 36.0,
                    alignment: crate::model::TabAlignment::Left,
                    leader: crate::model::TabLeader::Dot,
                },
                crate::model::TabStop {
                    position_pt: 72.0,
                    alignment: crate::model::TabAlignment::Center,
                    leader: crate::model::TabLeader::Hyphen,
                },
                crate::model::TabStop {
                    position_pt: 90.0,
                    alignment: crate::model::TabAlignment::Right,
                    leader: crate::model::TabLeader::Dot,
                },
            ]
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_docx_table_cell_column_break_roundtrips_through_fresh_conversion() {
        let source = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/><w:tblBorders><w:top w:val="single" w:sz="4" w:color="auto"/><w:left w:val="single" w:sz="4" w:color="auto"/><w:bottom w:val="single" w:sz="4" w:color="auto"/><w:right w:val="single" w:sz="4" w:color="auto"/><w:insideH w:val="single" w:sz="4" w:color="auto"/><w:insideV w:val="single" w:sz="4" w:color="auto"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="9000"/></w:tblGrid><w:tr><w:tc><w:p><w:r><w:t>A</w:t><w:br w:type="column"/><w:t>B</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
                <w:sectPr><w:cols w:num="2"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let opened = Document::open(&source).unwrap();
        let source_blocks = opened.model().blocks;
        let converted = opened.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");

        assert_eq!(
            document_xml.matches(r#"<w:br w:type="column"/>"#).count(),
            1,
            "{document_xml}"
        );
        assert!(!document_xml.contains("<w:br/>"), "{document_xml}");
        assert_eq!(converted, opened.to_docx());
        assert_eq!(
            Document::open(&converted).unwrap().model().blocks,
            source_blocks
        );

        let model_only_xml = docx_part(&write_docx(&opened.model()), "word/document.xml");
        assert!(!model_only_xml.contains(r#"<w:br w:type="column"/>"#));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_docx_table_cell_column_breaks_follow_wrappers_and_merge_survivors() {
        let source = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/><w:tblBorders><w:top w:val="single" w:sz="4" w:color="auto"/><w:left w:val="single" w:sz="4" w:color="auto"/><w:bottom w:val="single" w:sz="4" w:color="auto"/><w:right w:val="single" w:sz="4" w:color="auto"/><w:insideH w:val="single" w:sz="4" w:color="auto"/><w:insideV w:val="single" w:sz="4" w:color="auto"/></w:tblBorders></w:tblPr><w:tblGrid><w:gridCol w:w="4500"/><w:gridCol w:w="4500"/></w:tblGrid>
                    <w:tr>
                        <w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr><w:p><w:r><w:t>A</w:t><w:br w:type="column"/><w:t>B</w:t></w:r><w:r><w:rPr><w:vanish/></w:rPr><w:br w:type="column"/><w:t>HIDDEN</w:t></w:r></w:p><w:p><w:sdt><w:sdtPr><w:tag w:val="cell-break"/></w:sdtPr><w:sdtContent><w:r><w:t>C</w:t><w:br w:type=" column "/><w:t>D</w:t></w:r></w:sdtContent></w:sdt></w:p></w:tc>
                        <w:tc><w:p><w:r><w:t>E</w:t><w:br/><w:t>F</w:t><w:br w:type="column"/><w:t>G</w:t></w:r></w:p></w:tc>
                    </w:tr>
                    <w:tr>
                        <w:tc><w:tcPr><w:vMerge/></w:tcPr><w:p><w:r><w:t>DROP</w:t><w:br w:type="column"/><w:t>LEAK</w:t></w:r></w:p></w:tc>
                        <w:tc><w:p><w:r><w:t>TAIL</w:t></w:r></w:p></w:tc>
                    </w:tr>
                </w:tbl>
                <w:sectPr><w:cols w:num="2"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let opened = Document::open(&source).unwrap();
        let Backend::Docx(state) = &opened.backend else {
            panic!("fixture must open through the DOCX backend");
        };
        assert_eq!(
            state.table_cell_column_break_offsets,
            vec![vec![
                vec![vec![vec![1], vec![1]], vec![vec![3]]],
                vec![vec![vec![]]],
            ]]
        );
        let source_blocks = opened.model().blocks;
        let converted = opened.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");

        assert_eq!(
            document_xml.matches(r#"<w:br w:type="column"/>"#).count(),
            3,
            "{document_xml}"
        );
        assert_eq!(document_xml.matches("<w:br/>").count(), 2, "{document_xml}");
        assert!(document_xml.contains("<w:sdt>"), "{document_xml}");
        assert!(!document_xml.contains("DROP"), "{document_xml}");
        assert!(!document_xml.contains("LEAK"), "{document_xml}");
        assert_eq!(converted, opened.to_docx());
        assert_eq!(
            Document::open(&converted).unwrap().model().blocks,
            source_blocks
        );

        let model_only_xml = docx_part(&write_docx(&opened.model()), "word/document.xml");
        assert!(!model_only_xml.contains(r#"<w:br w:type="column"/>"#));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_docx_nested_table_layout_hints_roundtrip_through_fresh_conversion() {
        let source = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl><w:tr><w:tc>
                    <w:tbl><w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc>
                        <w:p><w:pPr><w:keepNext/><w:keepLines/><w:widowControl w:val="off"/><w:spacing w:line="240" w:lineRule="exact"/><w:tabs><w:tab w:val="right" w:pos="1440" w:leader="dot"/></w:tabs></w:pPr>
                            <w:r><w:t>A</w:t><w:tab/><w:t>B</w:t><w:br w:type="column"/><w:t>C</w:t></w:r>
                        </w:p>
                    </w:tc></w:tr></w:tbl>
                    <w:p><w:r><w:t>outer tail</w:t></w:r></w:p>
                </w:tc></w:tr></w:tbl>
                <w:sectPr><w:cols w:num="2"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let opened = Document::open(&source).unwrap();
        let converted = opened.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");

        assert!(document_xml.contains("<w:cantSplit/>"), "{document_xml}");
        assert!(document_xml.contains("<w:keepNext/>"), "{document_xml}");
        assert!(document_xml.contains("<w:keepLines/>"), "{document_xml}");
        assert!(
            document_xml.contains(r#"<w:widowControl w:val="0"/>"#),
            "{document_xml}"
        );
        assert!(
            document_xml.contains(r#"w:line="240" w:lineRule="exact""#),
            "{document_xml}"
        );
        assert!(
            document_xml.contains(r#"<w:tab w:val="right" w:pos="1440" w:leader="dot"/>"#),
            "{document_xml}"
        );
        assert_eq!(
            document_xml.matches(r#"<w:br w:type="column"/>"#).count(),
            1,
            "{document_xml}"
        );
        assert_eq!(converted, opened.to_docx());

        let model_only = write_docx(&opened.model());
        let model_only_xml = docx_part(&model_only, "word/document.xml");
        for marker in [
            "<w:cantSplit/>",
            "<w:keepNext/>",
            "<w:keepLines/>",
            r#"w:lineRule="exact""#,
            r#"w:leader="dot""#,
            r#"<w:br w:type="column"/>"#,
        ] {
            assert!(!model_only_xml.contains(marker), "{model_only_xml}");
        }
        assert_eq!(model_only_xml.matches("<w:br/>").count(), 1);
        assert_eq!(
            Document::open(&converted).unwrap().model().blocks,
            Document::open(&model_only).unwrap().model().blocks
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_docx_deep_nested_table_hints_follow_wrappers_and_merge_survivors() {
        let source = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl><w:tr><w:tc>
                    <w:tbl>
                        <w:tr><w:tc><w:tcPr><w:vMerge w:val="restart"/></w:tcPr>
                            <w:sdt><w:sdtContent><w:tbl><w:tr><w:trPr><w:cantSplit/></w:trPr><w:tc>
                                <w:p><w:pPr><w:keepNext/><w:spacing w:line="280" w:lineRule="atLeast"/><w:tabs><w:tab w:val="center" w:pos="1080" w:leader="hyphen"/></w:tabs></w:pPr>
                                    <w:r><w:t>D</w:t><w:tab/><w:t>E</w:t><w:br w:type="column"/><w:t>F</w:t></w:r>
                                </w:p>
                            </w:tc></w:tr></w:tbl></w:sdtContent></w:sdt>
                            <w:p><w:r><w:t>survivor tail</w:t></w:r></w:p>
                        </w:tc></w:tr>
                        <w:tr><w:tc><w:tcPr><w:vMerge/></w:tcPr>
                            <w:tbl><w:tr><w:tc><w:p><w:pPr><w:keepLines/><w:spacing w:line="999" w:lineRule="exact"/></w:pPr><w:r><w:t>LEAK</w:t><w:br w:type="column"/><w:t>DROP</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
                        </w:tc></w:tr>
                    </w:tbl>
                    <w:p><w:r><w:t>outer tail</w:t></w:r></w:p>
                </w:tc></w:tr></w:tbl>
                <w:sectPr><w:cols w:num="2"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let opened = Document::open(&source).unwrap();
        let converted = opened.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");

        assert_eq!(document_xml.matches("<w:cantSplit/>").count(), 1);
        assert_eq!(document_xml.matches("<w:keepNext/>").count(), 1);
        assert!(
            document_xml.contains(r#"w:line="280" w:lineRule="atLeast""#),
            "{document_xml}"
        );
        assert!(
            document_xml.contains(r#"<w:tab w:val="center" w:pos="1080" w:leader="hyphen"/>"#),
            "{document_xml}"
        );
        assert_eq!(
            document_xml.matches(r#"<w:br w:type="column"/>"#).count(),
            1,
            "{document_xml}"
        );
        assert!(!document_xml.contains("LEAK"), "{document_xml}");
        assert!(!document_xml.contains("DROP"), "{document_xml}");
        assert!(!document_xml.contains(r#"w:line="999""#), "{document_xml}");
        assert_eq!(converted, opened.to_docx());

        let model_only = write_docx(&opened.model());
        let model_only_xml = docx_part(&model_only, "word/document.xml");
        assert!(!model_only_xml.contains("<w:cantSplit/>"));
        assert!(!model_only_xml.contains("<w:keepNext/>"));
        assert!(!model_only_xml.contains(r#"w:lineRule="atLeast""#));
        assert!(!model_only_xml.contains(r#"w:leader="hyphen""#));
        assert!(!model_only_xml.contains(r#"<w:br w:type="column"/>"#));
        assert_eq!(
            Document::open(&converted).unwrap().model().blocks,
            Document::open(&model_only).unwrap().model().blocks
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_legacy_table_cell_column_breaks_roundtrip_through_fresh_conversion() {
        let legacy = Document::open(&legacy_table_cell_column_break_conversion_doc()).unwrap();
        let assembled = legacy_build_output_from_doc_state(match &legacy.backend {
            Backend::Doc(state) => state,
            Backend::Docx(_) => panic!("fixture must open through the legacy backend"),
        });
        assert_eq!(
            assembled.table_cell_column_break_offsets,
            vec![vec![vec![vec![vec![1], vec![1]]]], Vec::new()]
        );

        let converted = legacy.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");
        assert_eq!(
            document_xml.matches(r#"<w:br w:type="column"/>"#).count(),
            2,
            "{document_xml}"
        );
        assert!(!document_xml.contains("<w:br/>"), "{document_xml}");
        assert_eq!(converted, legacy.to_docx());

        let model_only = write_docx(&legacy.model());
        let model_only_xml = docx_part(&model_only, "word/document.xml");
        assert!(!model_only_xml.contains(r#"<w:br w:type="column"/>"#));
        assert_eq!(model_only_xml.matches("<w:br/>").count(), 2);
        assert_eq!(
            Document::open(&converted).unwrap().model().blocks,
            Document::open(&model_only).unwrap().model().blocks
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn opened_document_manual_column_breaks_roundtrip_through_fresh_conversion() {
        let source = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p><w:r><w:t>A</w:t><w:br w:type="column"/><w:t>B</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:br w:type=" column "/><w:t>C</w:t><w:br/><w:t>D</w:t></w:r><w:r><w:rPr><w:vanish/></w:rPr><w:br w:type="column"/><w:t>hidden</w:t></w:r></w:p>
                <w:sectPr><w:cols w:num="2"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let opened = Document::open(&source).unwrap();
        let source_blocks = opened.model().blocks;
        let converted = opened.to_docx();
        let document_xml = docx_part(&converted, "word/document.xml");
        assert_eq!(
            document_xml.matches(r#"<w:br w:type="column"/>"#).count(),
            2,
            "{document_xml}"
        );
        assert_eq!(document_xml.matches("<w:br/>").count(), 2, "{document_xml}");
        assert_eq!(converted, opened.to_docx());

        let model_only = write_docx(&opened.model());
        let model_only_xml = docx_part(&model_only, "word/document.xml");
        assert!(!model_only_xml.contains(r#"<w:br w:type="column"/>"#));

        let reopened = Document::open(&converted).unwrap();
        assert_eq!(reopened.model().blocks, source_blocks);
        #[cfg(feature = "render")]
        {
            let Backend::Docx(state) = reopened.backend else {
                panic!("converted document must use the DOCX backend");
            };
            assert_eq!(state.column_break_offsets[0], vec![1, 3]);
        }

        let legacy_text = "L\u{000e}M\u{000e}N\r";
        let legacy_end = legacy_text.encode_utf16().count() as u32;
        let mut legacy_section = Vec::new();
        push_section_column_count(&mut legacy_section, 1);
        let legacy_bytes = legacy_doc_with_section_page_grpprls(
            legacy_text,
            &[0, legacy_end],
            &[legacy_section.as_slice()],
        );
        let legacy = Document::open(&legacy_bytes).unwrap();
        let legacy_blocks = legacy.model().blocks;
        let legacy_converted = legacy.to_docx();
        let legacy_xml = docx_part(&legacy_converted, "word/document.xml");
        assert_eq!(
            legacy_xml.matches(r#"<w:br w:type="column"/>"#).count(),
            2,
            "{legacy_xml}"
        );
        assert_eq!(legacy_converted, legacy.to_docx());
        assert_eq!(
            Document::open(&legacy_converted).unwrap().model().blocks,
            legacy_blocks
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn legacy_doc_row_no_split_roundtrips_to_docx() {
        let convert = |properties: &[(u16, u8)]| {
            let legacy = Document::open(&legacy_row_pagination_doc(properties)).unwrap();
            let converted = legacy.to_docx();
            let document_xml = docx_part(&converted, "word/document.xml");
            (legacy, converted, document_xml)
        };
        let (modern, modern_docx, modern_xml) = convert(&[(0x3466, 1)]);
        assert!(
            modern_xml.contains("<w:tr><w:trPr><w:cantSplit/></w:trPr>"),
            "{modern_xml}"
        );
        assert_eq!(modern_docx, modern.to_docx());

        let (_, _, compatibility_xml) = convert(&[(0x3403, 1)]);
        assert!(
            compatibility_xml.contains("<w:tr><w:trPr><w:cantSplit/></w:trPr>"),
            "{compatibility_xml}"
        );
        let (modern_off, modern_off_docx, modern_off_xml) = convert(&[(0x3403, 1), (0x3466, 0)]);
        assert!(!modern_off_xml.contains("<w:cantSplit"), "{modern_off_xml}");
        assert_eq!(modern_off_docx, modern_off.to_docx());

        let model_only_xml = docx_part(&write_docx(&modern.model()), "word/document.xml");
        assert!(!model_only_xml.contains("<w:cantSplit"));

        #[cfg(feature = "render")]
        {
            let reopened_hint = |bytes: &[u8]| {
                let reopened = Document::open(bytes).unwrap();
                let Backend::Docx(state) = reopened.backend else {
                    panic!("converted document must use the DOCX backend");
                };
                state
                    .table_row_pagination
                    .into_iter()
                    .find(|rows| !rows.is_empty())
                    .and_then(|rows| rows.first().copied())
                    .expect("converted table row hint")
            };
            assert!(reopened_hint(&modern_docx).cant_split);
            assert!(!reopened_hint(&modern_off_docx).cant_split);
        }
    }

    #[cfg(feature = "render")]
    #[test]
    fn opened_legacy_doc_layout_uses_direct_row_pagination_hints() {
        let splittable = Document::open(&legacy_row_pagination_doc(&[])).unwrap();
        let modern = Document::open(&legacy_row_pagination_doc(&[(0x3466, 1)])).unwrap();
        let compatibility = Document::open(&legacy_row_pagination_doc(&[(0x3403, 1)])).unwrap();
        let modern_off =
            Document::open(&legacy_row_pagination_doc(&[(0x3403, 1), (0x3466, 0)])).unwrap();
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let first_table_hint = |document: &Document| match &document.backend {
            Backend::Doc(state) => {
                assert!(!state.papx.is_empty(), "synthetic PAPX must parse");
                legacy_build_output_from_doc_state(state)
                    .table_row_pagination
                    .into_iter()
                    .find(|rows| !rows.is_empty())
                    .and_then(|rows| rows.first().copied())
                    .expect("synthetic legacy table row hint")
            }
            #[cfg(feature = "docx")]
            Backend::Docx(_) => unreachable!("synthetic fixture is OLE"),
        };

        assert!(!first_table_hint(&splittable).cant_split);
        assert!(first_table_hint(&modern).cant_split);
        assert!(first_table_hint(&compatibility).cant_split);
        assert!(!first_table_hint(&modern_off).cant_split);

        let model = splittable.model();
        let model_layout = layout_pages_with_fonts(&model, &fonts).unwrap();
        let splittable_layout = splittable.layout_pages_with_fonts(&fonts).unwrap();
        let modern_layout = modern.layout_pages_with_fonts(&fonts).unwrap();
        let compatibility_layout = compatibility.layout_pages_with_fonts(&fonts).unwrap();
        let modern_off_layout = modern_off.layout_pages_with_fonts(&fonts).unwrap();

        assert_eq!(
            (
                model_layout.pages,
                splittable_layout.pages,
                modern_layout.pages,
                compatibility_layout.pages,
                modern_off_layout.pages,
            ),
            (3, 2, 3, 3, 2),
            "model-only and explicit no-split rows keep together; default and modern-off \
             legacy source rows split"
        );
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_layout_uses_private_table_row_pagination_hints() {
        let make_document = |row_properties: &str| {
            let xml = format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                    <w:p><w:r><w:t>seed</w:t></w:r></w:p>
                    <w:tbl><w:tr>{row_properties}<w:tc>
                        <w:p><w:pPr><w:spacing w:line="400" w:lineRule="exact"/></w:pPr><w:r><w:t>one</w:t></w:r></w:p>
                        <w:p><w:pPr><w:spacing w:line="400" w:lineRule="exact"/></w:pPr><w:r><w:t>two</w:t></w:r></w:p>
                        <w:p><w:pPr><w:spacing w:line="400" w:lineRule="exact"/></w:pPr><w:r><w:t>three</w:t></w:r></w:p>
                        <w:p><w:r><w:t>four</w:t></w:r></w:p>
                        <w:p><w:r><w:t>five</w:t></w:r></w:p>
                    </w:tc></w:tr></w:tbl>
                    <w:p><w:r><w:t>after</w:t></w:r></w:p>
                    <w:sectPr><w:pgSz w:w="4400" w:h="2400"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
                </w:body></w:document>"#
            );
            Document::open(&minimal_docx(&xml)).unwrap()
        };
        let splittable = make_document("");
        let kept = make_document("<w:trPr><w:cantSplit/></w:trPr>");
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        let model_pages = layout_pages_with_fonts(&splittable.model(), &fonts)
            .unwrap()
            .pages;
        let splittable_pages = splittable.layout_pages_with_fonts(&fonts).unwrap().pages;
        let kept_pages = kept.layout_pages_with_fonts(&fonts).unwrap().pages;

        assert_eq!(
            (model_pages, splittable_pages, kept_pages),
            (3, 2, 3),
            "model-only and direct cantSplit rows keep together; the default source row splits"
        );
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_layout_uses_private_table_cell_pagination_hints() {
        let make_document = |pagination: &str| {
            let xml = format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                    <w:p><w:pPr><w:spacing w:line="480"/></w:pPr><w:r><w:t>seed</w:t></w:r></w:p>
                    <w:tbl><w:tr><w:tc>
                        <w:p><w:pPr>{pagination}<w:widowControl w:val="off"/></w:pPr>
                            <w:r><w:t>one</w:t><w:br/><w:t>two</w:t><w:br/><w:t>three</w:t></w:r>
                        </w:p>
                    </w:tc></w:tr></w:tbl>
                    <w:p><w:pPr><w:widowControl w:val="off"/></w:pPr><w:r><w:t>after</w:t></w:r></w:p>
                    <w:sectPr><w:pgSz w:w="4400" w:h="2000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
                </w:body></w:document>"#
            );
            Document::open(&minimal_docx(&xml)).unwrap()
        };
        let splittable = make_document(r#"<w:keepLines w:val="off"/>"#);
        let kept = make_document("<w:keepLines/>");
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        let model_pages = layout_pages_with_fonts(&splittable.model(), &fonts)
            .unwrap()
            .pages;
        let splittable_pages = splittable.layout_pages_with_fonts(&fonts).unwrap().pages;
        let kept_pages = kept.layout_pages_with_fonts(&fonts).unwrap().pages;

        assert_eq!(
            (model_pages, splittable_pages, kept_pages),
            (3, 2, 3),
            "model-only and kept rows stay together; explicit keepLines off permits splitting"
        );
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_cell_spacing_moves_table_and_modeled_page_field_deterministically() {
        let make_document = |spacing: &str| {
            let xml = format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                    <w:p><w:r><w:t>seed one</w:t></w:r></w:p>
                    <w:p><w:r><w:t>seed two</w:t></w:r></w:p>
                    <w:tbl><w:tr><w:tc><w:p><w:pPr>{spacing}</w:pPr>
                        <w:r><w:t>cell</w:t></w:r>
                    </w:p></w:tc></w:tr></w:tbl>
                    <w:sectPr><w:pgSz w:w="4400" w:h="2200"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
                </w:body></w:document>"#
            );
            Document::open(&minimal_docx(&xml)).unwrap()
        };
        let compact = make_document("");
        let spaced = make_document(r#"<w:spacing w:before="220" w:after="140"/>"#);
        let Block::Table(spaced_table) = &spaced.model().blocks[2] else {
            panic!("third block must be the synthetic DOCX table");
        };
        let Block::Paragraph(spaced_cell_paragraph) = &spaced_table.rows[0].cells[0].blocks[0]
        else {
            panic!("synthetic DOCX cell must contain a paragraph");
        };
        assert_eq!(spaced_cell_paragraph.props.spacing.before_pt, Some(11.0));
        assert_eq!(spaced_cell_paragraph.props.spacing.after_pt, Some(7.0));

        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];
        let compact_layout = compact.layout_pages_with_fonts(&fonts).unwrap();
        let spaced_layout = spaced.layout_pages_with_fonts(&fonts).unwrap();
        assert_eq!(
            spaced_layout,
            spaced.layout_pages_with_fonts(&fonts).unwrap()
        );
        assert_eq!(compact_layout.block_pages[2], Some(1));
        assert_eq!(spaced_layout.block_pages[2], Some(2));

        let compact_pdf = compact
            .try_to_pdf_with_fonts_and_report(&fonts)
            .unwrap()
            .pdf;
        let spaced_pdf = spaced.try_to_pdf_with_fonts_and_report(&fonts).unwrap().pdf;
        assert_ne!(spaced_pdf, compact_pdf);
        assert_eq!(
            spaced_pdf,
            spaced.try_to_pdf_with_fonts_and_report(&fonts).unwrap().pdf
        );

        let mut compact_page_model = compact.model();
        let mut spaced_page_model = spaced.model();
        for model in [&mut compact_page_model, &mut spaced_page_model] {
            let Block::Table(table) = &mut model.blocks[2] else {
                panic!("third block must remain the synthetic DOCX table");
            };
            let Block::Paragraph(paragraph) = &mut table.rows[0].cells[0].blocks[0] else {
                panic!("synthetic DOCX cell must remain a paragraph");
            };
            paragraph.runs[0].field = FieldRole::Simple {
                instruction: "PAGE".to_string(),
            };
            paragraph.runs[0].text = "stale".to_string();
        }
        assert_eq!(
            layout_pages_with_fonts(&compact_page_model, &fonts)
                .unwrap()
                .page_fields,
            [Some(1)]
        );
        assert_eq!(
            layout_pages_with_fonts(&spaced_page_model, &fonts)
                .unwrap()
                .page_fields,
            [Some(2)]
        );
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_layout_uses_private_wrapped_table_cell_pagination_hints() {
        let make_document = |pagination: &str| {
            let xml = format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                    <w:p><w:pPr><w:spacing w:line="480"/></w:pPr><w:r><w:t>seed</w:t></w:r></w:p>
                    <w:tbl><w:tr><w:tc><w:sdt><w:sdtContent>
                        <w:p><w:pPr>{pagination}<w:widowControl w:val="off"/></w:pPr>
                            <w:r><w:t>one</w:t><w:br/><w:t>two</w:t><w:br/><w:t>three</w:t></w:r>
                        </w:p>
                    </w:sdtContent></w:sdt></w:tc></w:tr></w:tbl>
                    <w:p><w:pPr><w:widowControl w:val="off"/></w:pPr><w:r><w:t>after</w:t></w:r></w:p>
                    <w:sectPr><w:pgSz w:w="4400" w:h="2000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
                </w:body></w:document>"#
            );
            Document::open(&minimal_docx(&xml)).unwrap()
        };
        let splittable = make_document(r#"<w:keepLines w:val="off"/>"#);
        let kept = make_document("<w:keepLines/>");
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        let splittable_pages = splittable.layout_pages_with_fonts(&fonts).unwrap().pages;
        let kept_pages = kept.layout_pages_with_fonts(&fonts).unwrap().pages;

        assert_eq!(
            (splittable_pages, kept_pages),
            (2, 3),
            "wrapped cell paragraphs must retain the same keepLines behavior as direct ones"
        );
    }

    #[cfg(all(feature = "docx", feature = "render"))]
    #[test]
    fn opened_docx_layout_applies_bounded_top_and_bottom_wrap() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><w:body>
                <w:p><w:r><w:t>anchor</w:t><w:drawing><wp:anchor simplePos="0" behindDoc="0" layoutInCell="1">
                    <wp:positionH relativeFrom="page"><wp:posOffset>0</wp:posOffset></wp:positionH>
                    <wp:positionV relativeFrom="page"><wp:posOffset>571500</wp:posOffset></wp:positionV>
                    <wp:extent cx="254000" cy="317500"/><wp:wrapTopAndBottom/><wp:docPr id="1" name="Wrapped"/>
                    <a:graphic><a:graphicData/></a:graphic>
                </wp:anchor></w:drawing></w:r></w:p>
                <w:p><w:r><w:t>following</w:t></w:r></w:p>
                <w:sectPr><w:pgSz w:w="4400" w:h="2000"/><w:pgMar w:top="400" w:right="400" w:bottom="400" w:left="400"/></w:sectPr>
            </w:body></w:document>"#,
        );
        let document = Document::open(&bytes).unwrap();
        let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

        assert_eq!(document.floating_shapes().len(), 1);
        assert_eq!(
            document.floating_shapes()[0]
                .wrapping
                .as_ref()
                .map(|wrapping| wrapping.kind.as_str()),
            Some("topAndBottom")
        );
        let raw_model_layout = layout_pages_with_fonts(&document.model(), &fonts).unwrap();
        let opened_document_layout = document.layout_pages_with_fonts(&fonts).unwrap();
        let rendered = document.try_to_pdf_with_fonts_and_report(&fonts).unwrap();

        assert_eq!(raw_model_layout.block_pages, vec![Some(1), Some(1)]);
        assert_eq!(opened_document_layout.block_pages, vec![Some(1), Some(2)]);
        assert_eq!(rendered.report.pages, opened_document_layout.pages);
        assert!(rendered.pdf.starts_with(b"%PDF-"));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn edit_session_refresh_failure_restores_package_and_read_views() {
        let mut document = Document::try_new().unwrap();
        let before_parts = unzip_parts(&document.save().unwrap());
        let before_text = document.main_text().to_string();

        let edits = document.edit_session().unwrap();
        let Backend::Docx(state) = &mut edits.document.backend else {
            unreachable!("try_new creates a DOCX backend");
        };
        state.package.ensure_relationship(
            "",
            "https://example.invalid/relationships/missing",
            "missing-part.bin",
        );
        let error = edits.commit().unwrap_err();

        assert!(
            error.to_string().contains("targets missing part"),
            "{error}"
        );
        assert_eq!(unzip_parts(&document.save().unwrap()), before_parts);
        assert_eq!(document.main_text(), before_text);
        assert!(document.edited_parts().is_empty());
        document.refresh_read_view().unwrap();
    }

    #[cfg(feature = "docx")]
    #[test]
    fn explicit_refresh_failure_preserves_package_and_read_views() {
        let mut document = Document::try_new().unwrap();
        let before_text = document.main_text().to_string();
        let Backend::Docx(state) = &mut document.backend else {
            unreachable!("try_new creates a DOCX backend");
        };
        let missing_relationship_id = state.package.ensure_relationship(
            "",
            "https://example.invalid/relationships/missing",
            "missing-part.bin",
        );
        let touched = state.package.touched_parts();

        let error = document.refresh_read_view().unwrap_err();

        assert!(
            error.to_string().contains("targets missing part"),
            "{error}"
        );
        assert_eq!(document.main_text(), before_text);
        let Backend::Docx(state) = &document.backend else {
            unreachable!("try_new creates a DOCX backend");
        };
        assert_eq!(state.package.touched_parts(), touched);
        assert!(state
            .package
            .rels_for("")
            .iter()
            .any(|relationship| relationship.id == missing_relationship_id));
    }

    #[cfg(feature = "docx")]
    #[test]
    fn atomic_body_block_edits_preserve_package_and_reopen_in_order() {
        let bytes = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:p data-id="A"><w:r><w:t>A</w:t></w:r><w:unknown keep="1"/></w:p>
                <w:tbl data-id="B"><w:tr><w:tc><w:p><w:r><w:t>B</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
                <w:sdt data-id="C"><w:sdtPr/><w:sdtContent><w:p><w:r><w:t>C</w:t></w:r></w:p></w:sdtContent></w:sdt>
                <w:sectPr/>
            </w:body></w:document>"#,
        );
        let original_parts = unzip_parts(&bytes);
        let mut document = Document::open(&bytes).unwrap();

        assert_eq!(
            document.body_blocks().unwrap(),
            vec![
                BodyBlockInfo {
                    index: 0,
                    kind: BodyBlockKind::Paragraph,
                },
                BodyBlockInfo {
                    index: 1,
                    kind: BodyBlockKind::Table,
                },
                BodyBlockInfo {
                    index: 2,
                    kind: BodyBlockKind::ContentControl,
                },
            ]
        );

        document.move_body_block(0, 2).unwrap();
        assert_eq!(document.edited_parts(), vec!["word/document.xml"]);
        assert_eq!(document.main_text(), "A\nB\nC");
        assert_eq!(
            document
                .body_blocks()
                .unwrap()
                .into_iter()
                .map(|block| block.kind)
                .collect::<Vec<_>>(),
            vec![
                BodyBlockKind::Table,
                BodyBlockKind::ContentControl,
                BodyBlockKind::Paragraph,
            ]
        );
        let moved = document.save().unwrap();
        let moved_parts = unzip_parts(&moved);
        for (name, payload) in &original_parts {
            if name != "word/document.xml" {
                assert_eq!(moved_parts.get(name), Some(payload), "changed part {name}");
            }
        }
        let moved_xml = String::from_utf8(moved_parts["word/document.xml"].clone()).unwrap();
        let b = moved_xml.find("data-id=\"B\"").unwrap();
        let c = moved_xml.find("data-id=\"C\"").unwrap();
        let a = moved_xml.find("data-id=\"A\"").unwrap();
        assert!(b < c && c < a, "unexpected body order: {moved_xml}");
        assert!(moved_xml.contains("<w:unknown keep=\"1\"/>"));

        let mut reopened = Document::open(&moved).unwrap();
        assert_eq!(reopened.main_text(), "B\nC\nA");
        reopened.remove_body_block(1).unwrap();
        let removed = reopened.save().unwrap();
        let removed_xml =
            String::from_utf8(unzip_parts(&removed)["word/document.xml"].clone()).unwrap();
        assert!(!removed_xml.contains("data-id=\"C\""));
        assert_eq!(Document::open(&removed).unwrap().main_text(), "B\nA");
    }

    #[cfg(feature = "docx")]
    #[test]
    fn atomic_body_block_edits_are_transactional_and_noop_stays_raw() {
        let safe = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>A</w:t></w:r></w:p><w:p><w:r><w:t>B</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        );
        let safe_xml = unzip_parts(&safe)["word/document.xml"].clone();
        let mut noop = Document::open(&safe).unwrap();
        noop.move_body_block(0, 0).unwrap();
        assert!(noop.edited_parts().is_empty());
        assert_eq!(
            unzip_parts(&noop.save().unwrap())["word/document.xml"],
            safe_xml
        );

        let hazardous = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:bookmarkStart w:id="7"/><w:r><w:t>A</w:t></w:r></w:p><w:p><w:r><w:t>B</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p><w:sectPr/></w:body></w:document>"#,
        );
        let hazardous_xml = unzip_parts(&hazardous)["word/document.xml"].clone();
        let mut document = Document::open(&hazardous).unwrap();
        assert!(document.body_blocks().is_err());
        assert!(document.remove_body_block(0).is_err());
        assert!(document.move_body_block(0, 9).is_err());
        assert!(document.edited_parts().is_empty());
        assert_eq!(
            unzip_parts(&document.save().unwrap())["word/document.xml"],
            hazardous_xml
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn body_paragraph_insertion_rolls_back_on_probe_node_budget_failure() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>A</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);
        let before = unzip_parts(&bytes);
        let mut document = Document::open(&bytes).unwrap();
        let document_nodes = xmltree::XmlTree::parse(doc_xml.as_bytes())
            .unwrap()
            .node_count();
        xmltree::set_test_node_budget(document_nodes + 1);

        let result = document.insert_body_paragraph(1, "Too many nodes");

        xmltree::reset_test_node_budget();
        assert!(result.is_err());
        assert!(document.edited_parts().is_empty());
        assert_eq!(unzip_parts(&document.save().unwrap()), before);
    }

    #[cfg(feature = "docx")]
    #[test]
    fn body_paragraph_insertion_rolls_back_on_second_pass_graft_failure() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>A</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);
        let before = unzip_parts(&bytes);
        let mut document = Document::open(&bytes).unwrap();
        xmltree::set_test_fail_commit_after(1);

        let result = document.insert_body_paragraph(1, "Clone must not commit");

        xmltree::reset_test_fail_commit();
        assert!(result.is_err());
        assert!(document.edited_parts().is_empty());
        assert_eq!(unzip_parts(&document.save().unwrap()), before);
    }

    #[cfg(feature = "docx")]
    #[test]
    fn body_paragraph_insertion_rolls_back_on_part_size_failure() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>A</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);
        let before = unzip_parts(&bytes);
        let max_original_part = before.values().map(Vec::len).max().unwrap();
        let oversized_text = "x".repeat(max_original_part + 1);
        let mut document = Document::open(&bytes).unwrap();
        crate::opc::set_test_max_part(max_original_part as u64);

        let result = document.insert_body_paragraph(1, &oversized_text);

        crate::opc::reset_test_max_part();
        let error = result.unwrap_err();
        assert!(
            error
                .to_string()
                .contains("word/document.xml exceeds the per-part size budget"),
            "{error}"
        );
        assert!(document.edited_parts().is_empty());
        assert_eq!(unzip_parts(&document.save().unwrap()), before);
    }

    #[cfg(feature = "docx")]
    #[test]
    fn edit_reuses_case_variant_document_override() {
        use zip::write::SimpleFileOptions;

        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        let ct = format!(
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/Document.xml" ContentType="{CT_DOCUMENT_MAIN}"/></Types>"#
        );
        for (n, b) in [
            ("[Content_Types].xml", ct.as_str()),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/document.xml",
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document>"#,
            ),
        ] {
            zw.start_file(n, opt).unwrap();
            zw.write_all(b.as_bytes()).unwrap();
        }
        let bytes = zw.finish().unwrap().into_inner();
        let mut doc = Document::open(&bytes).unwrap();

        assert_eq!(doc.replace_body_text("OLD", "NEW").unwrap(), 1);

        let saved = doc.save().unwrap();
        let parts = unzip_parts(&saved);
        let ct = String::from_utf8(parts["[Content_Types].xml"].clone()).unwrap();
        assert_eq!(
            ct.to_ascii_lowercase()
                .matches(r#"partname="/word/document.xml""#)
                .count(),
            1,
            "edit duplicated a case-variant document Override: {ct}"
        );
        assert!(Document::open(&saved).is_ok(), "saved output must reopen");
    }

    /// Body & `sectPr` anchoring is **namespace-aware**, so a
    /// foreign `<x:body>` / `<x:sectPr>` cannot misdirect an image insert.
    #[cfg(feature = "docx")]
    #[test]
    fn add_image_anchors_are_namespace_aware() {
        // (a) A document with only a FOREIGN <x:body> (no WML body) must be rejected,
        // not treated as a body.
        let foreign_body = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:x="urn:x"><x:body><x:p/></x:body></w:document>"#,
        );
        let mut d = Document::open(&foreign_body).unwrap();
        assert!(
            d.add_image_png(&tiny_png(), "image1.png").is_err(),
            "foreign <x:body> wrongly accepted as a body"
        );

        // (b) A WML body whose LAST child is a foreign <x:sectPr> after the real
        // <w:sectPr>: the image must land before the real w:sectPr, not the x:sectPr.
        let mixed = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:x="urn:x"><w:body><w:p><w:r><w:t>t</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="11906" w:h="16838"/></w:sectPr><x:sectPr/></w:body></w:document>"#,
        );
        let mut d2 = Document::open(&mixed).unwrap();
        d2.add_image_png(&tiny_png(), "image1.png").unwrap();
        let body = String::from_utf8(unzip_parts(&d2.save().unwrap())["word/document.xml"].clone())
            .unwrap();
        let draw = body.find("<w:drawing").expect("drawing inserted");
        let real_sect = body.find("<w:sectPr").expect("w:sectPr present");
        assert!(
            draw < real_sect,
            "image must precede the real w:sectPr (not the foreign x:sectPr):\n{body}"
        );

        // (c) A nested WML `<w:body>` (not a child of w:document) must NOT be mistaken
        // for the real body — the image goes into the document's direct-child body.
        let nested = minimal_docx(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:foo><w:body><w:p><w:r><w:t>FAKE</w:t></w:r></w:p></w:body></w:foo><w:body><w:p><w:r><w:t>REAL</w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#,
        );
        let mut d3 = Document::open(&nested).unwrap();
        d3.add_image_png(&tiny_png(), "image1.png").unwrap();
        let body3 =
            String::from_utf8(unzip_parts(&d3.save().unwrap())["word/document.xml"].clone())
                .unwrap();
        let real = body3.find("REAL").expect("real body present");
        let drew = body3.find("<w:drawing").expect("drawing inserted");
        assert!(
            drew > real,
            "image went into the nested fake body, not the document's real body:\n{body3}"
        );
        assert!(body3.contains("FAKE"), "nested body content lost");
    }

    /// `replace_body_text` is anchored to the document body — a
    /// `w:t` that is a SIBLING of `w:body` (malformed/extension input) is not edited.
    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_is_scoped_to_body() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:t>OUTSIDE</w:t><w:body><w:p><w:r><w:t>INSIDE</w:t></w:r></w:p></w:body></w:document>"#;
        let mut doc = Document::open(&minimal_docx(doc_xml)).unwrap();
        // The out-of-body run is not matched (count 0, no-op).
        assert_eq!(doc.replace_body_text("OUTSIDE", "X").unwrap(), 0);
        // The in-body run is.
        assert_eq!(doc.replace_body_text("INSIDE", "EDITED").unwrap(), 1);
        let body =
            String::from_utf8(unzip_parts(&doc.save().unwrap())["word/document.xml"].clone())
                .unwrap();
        assert!(
            body.contains("OUTSIDE") && body.contains("EDITED"),
            "out-of-body text must be untouched, in-body text edited: {body}"
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_only_edits_selected_alternate_content_branch() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:compat="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body><compat:AlternateContent><compat:Choice Requires="w14"><w:p><w:r><w:t>OLD</w:t></w:r></w:p></compat:Choice><compat:Fallback><w:p><w:r><w:t>OLD</w:t></w:r></w:p></compat:Fallback></compat:AlternateContent></w:body></w:document>"#;
        let mut doc = Document::open(&minimal_docx(doc_xml)).unwrap();

        assert_eq!(doc.replace_body_text("OLD", "NEW").unwrap(), 1);

        let body =
            String::from_utf8(unzip_parts(&doc.save().unwrap())["word/document.xml"].clone())
                .unwrap();
        assert_eq!(body.matches("<w:t>NEW</w:t>").count(), 1, "{body}");
        assert_eq!(body.matches("<w:t>OLD</w:t>").count(), 1, "{body}");
    }

    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_skips_deleted_revision_text() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:del w:id="1"><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:del><w:moveFrom w:id="2"><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:moveFrom><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document>"#;
        let mut doc = Document::open(&minimal_docx(doc_xml)).unwrap();

        assert_eq!(doc.replace_body_text("OLD", "NEW").unwrap(), 1);

        let body =
            String::from_utf8(unzip_parts(&doc.save().unwrap())["word/document.xml"].clone())
                .unwrap();
        assert!(
            body.contains(r#"<w:del w:id="1"><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:del>"#),
            "deleted text changed: {body}"
        );
        assert!(
            body.contains(
                r#"<w:moveFrom w:id="2"><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:moveFrom>"#
            ),
            "moved-from text changed: {body}"
        );
        assert!(
            body.contains("<w:p><w:r><w:t>NEW</w:t></w:r></w:p>"),
            "current text not changed: {body}"
        );
    }

    /// A misplaced XML declaration makes `document.xml` malformed for editing, even if the
    /// lenient read view can extract the body text. The element-tree editor must keep it
    /// passthrough-only rather than serializing a still-invalid edited part.
    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_rejects_late_xml_declaration() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document><?xml version="1.0"?>"#;
        let bytes = minimal_docx(doc_xml);
        let before = unzip_parts(&bytes);
        let mut doc = Document::open(&bytes).unwrap();

        assert!(
            doc.replace_body_text("OLD", "NEW").is_err(),
            "malformed document.xml must be read-only for element-tree edits"
        );

        let after = unzip_parts(&doc.save().unwrap());
        assert_eq!(
            after["word/document.xml"], before["word/document.xml"],
            "failed edit must leave malformed document.xml byte-identical"
        );
    }

    /// `replace_body_text` matches `w:t` text held as CDATA.
    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_matches_cdata() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t><![CDATA[OLD]]></w:t></w:r></w:p></w:body></w:document>"#;
        let mut doc = Document::open(&minimal_docx(doc_xml)).unwrap();
        let n = doc.replace_body_text("OLD", "NEW").unwrap();
        assert_eq!(n, 1, "CDATA w:t text not matched");
        let body =
            String::from_utf8(unzip_parts(&doc.save().unwrap())["word/document.xml"].clone())
                .unwrap();
        assert!(
            body.contains("NEW") && !body.contains("OLD"),
            "CDATA text not replaced: {body}"
        );
    }

    /// Edited text must serialize as XML-valid character data even when caller input
    /// contains Rust-valid but XML-forbidden scalar values.
    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_drops_xml_forbidden_scalars() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document>"#;
        let mut doc = Document::open(&minimal_docx(doc_xml)).unwrap();
        let n = doc.replace_body_text("OLD", "A\u{FFFF}B\u{FFFE}C").unwrap();
        assert_eq!(n, 1);
        let saved = doc.save().unwrap();
        let body = String::from_utf8(unzip_parts(&saved)["word/document.xml"].clone()).unwrap();
        assert!(
            body.contains("<w:t>ABC</w:t>"),
            "forbidden XML scalar leaked into document.xml: {body:?}"
        );
        assert_eq!(Document::open(&saved).unwrap().text(), "ABC");
    }

    /// See [`replace_body_text_is_scoped_to_body`]; `add_image_png` is
    /// transactional even when the body insertion
    /// would fail. With the node budget lowered so the drawing fragment can't fit, the
    /// call errors and leaves the package untouched — no media part, content-type, or
    /// relationship is added.
    #[cfg(feature = "docx")]
    #[test]
    fn add_image_png_rolls_back_on_budget_failure() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>x</w:t></w:r></w:p></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);
        let before = unzip_parts(&bytes);
        // Budget just above the body's node count: parsing document.xml still succeeds,
        // but grafting the (multi-node) drawing fragment would exceed it.
        let doc_nodes = xmltree::XmlTree::parse(doc_xml.as_bytes())
            .unwrap()
            .node_count();
        xmltree::set_test_node_budget(doc_nodes + 1);
        let mut doc = Document::open(&bytes).unwrap();
        let r = doc.add_image_png(&tiny_png(), "image1.png");
        xmltree::reset_test_node_budget(); // back to production MAX_NODES before asserting
        assert!(r.is_err(), "over-budget image insert should error");
        let after = unzip_parts(&doc.save().unwrap());
        assert!(
            !after.contains_key("word/media/image1.png"),
            "media part leaked after a failed insert"
        );
        let rels = String::from_utf8_lossy(
            after
                .get("word/_rels/document.xml.rels")
                .map(|v| v.as_slice())
                .unwrap_or(b""),
        );
        assert!(!rels.contains("image1.png"), "image rel leaked: {rels}");
        // document.xml is unchanged (no orphaned drawing).
        assert_eq!(
            before.get("word/document.xml"),
            after.get("word/document.xml"),
            "document.xml changed despite a failed insert"
        );
    }

    /// `add_image_png` stays transactional even after a prior
    /// `replace_body_text` left detached arena nodes — the budget is preflighted against
    /// the LIVE tree, so an over-budget insert errors BEFORE the media part/rel are added
    /// (no orphaned package change).
    #[cfg(feature = "docx")]
    #[test]
    fn add_image_png_transactional_after_prior_edit() {
        // The `OLD` run has an extra element child; replacing its text detaches that
        // child (it stays in the arena, uncounted by a fresh re-parse).
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD<w:noBreakHyphen/></w:t></w:r></w:p><w:sectPr/></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);
        let mut doc = Document::open(&bytes).unwrap();
        assert_eq!(doc.replace_body_text("OLD", "NEW").unwrap(), 1);
        // Budget = live arena count + 1: promotion is fine, but the multi-node drawing
        // fragment can't fit, so the insert must be rejected before any package mutation.
        let live = doc.docx_node_count();
        xmltree::set_test_node_budget(live + 1);
        let r = doc.add_image_png(&tiny_png(), "image1.png");
        xmltree::reset_test_node_budget();
        assert!(r.is_err(), "over-budget insert after an edit should error");
        let after = unzip_parts(&doc.save().unwrap());
        assert!(
            !after.contains_key("word/media/image1.png"),
            "media part leaked after a failed insert"
        );
        let rels = String::from_utf8_lossy(
            after
                .get("word/_rels/document.xml.rels")
                .map(|v| v.as_slice())
                .unwrap_or(b""),
        );
        assert!(!rels.contains("image1.png"), "image rel leaked: {rels}");
        let body = String::from_utf8_lossy(&after["word/document.xml"]);
        assert!(body.contains("NEW"), "prior edit lost");
        assert!(!body.contains("<w:drawing"), "drawing leaked: {body}");
    }

    /// An edit can't produce a part/package over the size budget that
    /// the crate would later refuse to open. Staged edits validate the resulting package
    /// before commit and leave the original package untouched on failure.
    #[cfg(feature = "docx")]
    #[test]
    fn edits_respect_part_size_budget() {
        // add_image_png: oversize image rejected before mutation (budget lowered AFTER
        // open, so opening the doc itself is unaffected).
        let mut doc = Document::open(&docx_rich_body()).unwrap();
        crate::opc::set_test_max_part(8); // tiny_png is 68 bytes > 8
        let r = doc.add_image_png(&tiny_png(), "image1.png");
        crate::opc::reset_test_max_part();
        assert!(r.is_err(), "oversize image should be rejected");
        let parts = unzip_parts(&doc.save().unwrap());
        assert!(
            !parts.contains_key("word/media/image1.png"),
            "rejected image leaked"
        );

        // replace_body_text: an over-budget staged document.xml is rejected before commit.
        let original = docx_rich_body();
        let before = unzip_parts(&original);
        let mut doc2 = Document::open(&original).unwrap();
        crate::opc::set_test_max_part(8); // document.xml is far larger than 8 bytes
        let edit = doc2.replace_body_text("OLD", "NEW");
        crate::opc::reset_test_max_part();
        assert!(edit.is_err(), "over-budget edit should fail before commit");
        assert!(doc2.edited_parts().is_empty());
        let after = unzip_parts(&doc2.save().unwrap());
        assert_eq!(after, before, "failed edit changed package payloads");
    }

    /// add_image_png rejects a part name longer than the OPC limit,
    /// so an edit can't produce a package `Document::open` would reject.
    #[cfg(feature = "docx")]
    #[test]
    fn add_image_png_rejects_overlong_name() {
        let long = format!("{}.png", "a".repeat(5000)); // valid charset, far over MAX_NAME_LEN
        let mut doc = Document::open(&docx_rich_body()).unwrap();
        assert!(doc.add_image_png(&tiny_png(), &long).is_err());
        // A normal name still works (sanity).
        let mut ok = Document::open(&docx_rich_body()).unwrap();
        assert!(ok.add_image_png(&tiny_png(), "image1.png").is_ok());
    }

    /// An edit REPAIRS a mistyped `word/document.xml` content type
    /// (a generic `application/xml` override) to the WML main+xml type, so the saved file
    /// stays Word-openable — the documented intentional `[Content_Types].xml` rewrite.
    #[cfg(feature = "docx")]
    #[test]
    fn edit_repairs_mistyped_document_content_type() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        for (n, b) in [
            // document.xml is mistyped as the generic application/xml (resolves, but wrong).
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/document.xml",
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document>"#,
            ),
        ] {
            zw.start_file(n, opt).unwrap();
            zw.write_all(b.as_bytes()).unwrap();
        }
        let bytes = zw.finish().unwrap().into_inner();
        let mut doc = Document::open(&bytes).unwrap();
        assert_eq!(doc.replace_body_text("OLD", "NEW").unwrap(), 1);
        let after = unzip_parts(&doc.save().unwrap());
        let ct = String::from_utf8_lossy(&after["[Content_Types].xml"]);
        assert!(
            ct.contains("wordprocessingml.document.main+xml"),
            "document.xml content type not repaired: {ct}"
        );
    }

    /// A package with NO [Content_Types].xml opens read-only — the
    /// body reads, but edits are refused (regenerating content types from nothing would
    /// leave referenced parts untyped, producing a file Word rejects).
    #[cfg(feature = "docx")]
    #[test]
    fn missing_content_types_is_read_only() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        // No [Content_Types].xml. document.xml references styles.xml via rels.
        for (n, b) in [
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/_rels/document.xml.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/></Relationships>"#,
            ),
            (
                "word/styles.xml",
                r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#,
            ),
            (
                "word/document.xml",
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document>"#,
            ),
        ] {
            zw.start_file(n, opt).unwrap();
            zw.write_all(b.as_bytes()).unwrap();
        }
        let bytes = zw.finish().unwrap().into_inner();
        let mut doc = Document::open(&bytes).unwrap(); // opens for read
        assert!(doc.text().contains("OLD"));
        assert!(
            doc.add_image_png(&tiny_png(), "i.png").is_err(),
            "editing a CT-less package must be refused"
        );
        assert!(doc.replace_body_text("OLD", "NEW").is_err());
    }

    /// A malformed UNRELATED `.rels` doesn't block the read path —
    /// `Document::open` succeeds, the body reads, a no-op save preserves the raw malformed
    /// part; only EDITS (which would regenerate metadata lossily) are refused.
    #[cfg(feature = "docx")]
    #[test]
    fn malformed_unrelated_rels_opens_read_only() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let bad_rels = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#; // unclosed root
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        for (n, b) in [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/document.xml",
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document>"#,
            ),
            ("word/_rels/header1.xml.rels", bad_rels),
        ] {
            zw.start_file(n, opt).unwrap();
            zw.write_all(b.as_bytes()).unwrap();
        }
        let bytes = zw.finish().unwrap().into_inner();
        let mut doc = Document::open(&bytes).unwrap(); // opens despite the malformed .rels
        assert!(doc.text().contains("OLD"), "body should still read");
        assert!(
            doc.replace_body_text("OLD", "NEW").is_err(),
            "edit must refuse"
        );
        assert!(
            doc.add_image_png(&tiny_png(), "i.png").is_err(),
            "edit must refuse"
        );
        let after = unzip_parts(&doc.save().unwrap()); // no-op save still works
        assert_eq!(
            after
                .get("word/_rels/header1.xml.rels")
                .map(|v| v.as_slice()),
            Some(bad_rels.as_bytes()),
            "malformed unrelated .rels not preserved verbatim"
        );
    }

    /// A same-value `replace_body_text("X","X")` is a no-op — it
    /// returns 0 and leaves `document.xml` byte-identical (no canonicalizing promotion).
    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_same_value_is_noop() {
        let orig = docx_rich_body();
        let before = unzip_parts(&orig);
        let mut doc = Document::open(&orig).unwrap();
        assert_eq!(doc.replace_body_text("OLD", "OLD").unwrap(), 0);
        let after = unzip_parts(&doc.save().unwrap());
        assert_eq!(
            before.get("word/document.xml"),
            after.get("word/document.xml"),
            "same-value replace canonicalized document.xml"
        );
    }

    /// A failed `add_image_png` preflight does NOT promote/
    /// canonicalize `document.xml` — non-canonical input (single-quoted attrs) is left
    /// byte-identical (the preflight reads without dirtying a still-`Raw` part).
    #[cfg(feature = "docx")]
    #[test]
    fn failed_add_image_leaves_noncanonical_xml_byte_identical() {
        // Single-quoted xmlns + no w:body ⇒ add_image_png fails the body check.
        let doc_xml = "<w:document xmlns:w='http://schemas.openxmlformats.org/wordprocessingml/2006/main'></w:document>";
        let bytes = minimal_docx(doc_xml);
        let before = unzip_parts(&bytes);
        let mut doc = Document::open(&bytes).unwrap();
        assert!(doc.add_image_png(&tiny_png(), "i.png").is_err());
        let after = unzip_parts(&doc.save().unwrap());
        assert_eq!(
            before.get("word/document.xml"),
            after.get("word/document.xml"),
            "failed insert canonicalized document.xml"
        );
        assert!(
            String::from_utf8_lossy(&after["word/document.xml"]).contains("xmlns:w='"),
            "single-quoted attrs were rewritten despite the insert failing"
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn fill_content_control_markers_respect_exact_node_budget_atomically() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:sdt><w:sdtPr><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Old</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);
        let before = unzip_parts(&bytes);
        let doc_nodes = xmltree::XmlTree::parse(doc_xml.as_bytes())
            .unwrap()
            .node_count();
        let fragment_nodes = xmltree::wml_anchored_text_run_content_node_count("A\tB\nC").unwrap();

        xmltree::set_test_node_budget(doc_nodes + fragment_nodes - 1);
        let mut rejected = Document::open(&bytes).unwrap();
        let over = rejected.fill_content_control_by_tag("client-name", "A\tB\nC");
        xmltree::reset_test_node_budget();
        assert!(over.is_err(), "over-budget marker fill should error");
        assert_eq!(
            unzip_parts(&rejected.save().unwrap()),
            before,
            "failed marker preflight changed the package"
        );

        xmltree::set_test_node_budget(doc_nodes + fragment_nodes);
        let mut accepted = Document::open(&bytes).unwrap();
        let exact = accepted.fill_content_control_by_tag("client-name", "A\tB\nC");
        xmltree::reset_test_node_budget();
        assert_eq!(exact.unwrap(), 1, "exact node-budget boundary rejected");
        let body =
            String::from_utf8(unzip_parts(&accepted.save().unwrap())["word/document.xml"].clone())
                .unwrap();
        assert!(
            body.contains("<w:t>A</w:t><w:tab/><w:t>B</w:t><w:br/><w:t>C</w:t>"),
            "exact-boundary marker fill missing: {body}"
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn fill_template_fields_respects_cumulative_body_marker_budget() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:sdt><w:sdtPr><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Old client</w:t></w:r></w:p></w:sdtContent></w:sdt><w:p><w:fldSimple w:instr=" MERGEFIELD project-name "><w:r><w:t>Old project</w:t></w:r></w:fldSimple></w:p></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);
        let before = unzip_parts(&bytes);
        let doc_nodes = xmltree::XmlTree::parse(doc_xml.as_bytes())
            .unwrap()
            .node_count();
        let values = [
            ("client-name", "Client\tValue"),
            ("project-name", "Project\nValue"),
        ];
        let fragment_nodes = values
            .iter()
            .map(|(_, value)| xmltree::wml_anchored_text_run_content_node_count(value).unwrap())
            .sum::<usize>();

        xmltree::set_test_node_budget(doc_nodes + fragment_nodes - 1);
        let mut rejected = Document::open(&bytes).unwrap();
        let over = rejected.fill_template_fields(values);
        xmltree::reset_test_node_budget();
        assert!(
            over.is_err(),
            "cumulative body marker overflow should error"
        );
        assert_eq!(
            unzip_parts(&rejected.save().unwrap()),
            before,
            "failed cumulative body preflight changed the package"
        );

        xmltree::set_test_node_budget(doc_nodes + fragment_nodes);
        let mut accepted = Document::open(&bytes).unwrap();
        let exact = accepted.fill_template_fields(values);
        xmltree::reset_test_node_budget();
        assert_eq!(exact.unwrap(), 2, "exact cumulative body budget rejected");
        let body =
            String::from_utf8(unzip_parts(&accepted.save().unwrap())["word/document.xml"].clone())
                .unwrap();
        assert!(
            body.contains("<w:tab/>") && body.contains("<w:br/>"),
            "{body}"
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn fill_template_fields_respects_story_marker_budget_and_attribute_cap() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let document_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><w:body><w:sectPr><w:headerReference w:type="default" r:id="rIdHeader"/></w:sectPr></w:body></w:document>"#;
        let header_xml = r#"<w:hdr xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:sdt><w:sdtPr><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t w:id="1" w:rsid="2">Old client</w:t></w:r></w:p></w:sdtContent></w:sdt><w:p><w:fldSimple w:instr=" MERGEFIELD project-name "><w:r><w:t w:id="3" w:rsid="4">Old project</w:t></w:r></w:fldSimple></w:p></w:hdr>"#;
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        for (name, body) in [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/_rels/document.xml.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdHeader" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/></Relationships>"#,
            ),
            ("word/document.xml", document_xml),
            ("word/header1.xml", header_xml),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        let before = unzip_parts(&bytes);
        let document_nodes = xmltree::XmlTree::parse(document_xml.as_bytes())
            .unwrap()
            .node_count();
        let header_nodes = xmltree::XmlTree::parse(header_xml.as_bytes())
            .unwrap()
            .node_count();
        let values = [
            ("client-name", " Client\tValue "),
            ("project-name", " Project\nValue "),
        ];
        let fragment_nodes = values
            .iter()
            .map(|(_, value)| xmltree::wml_anchored_text_run_content_node_count(value).unwrap())
            .sum::<usize>();
        let rejected_budget = header_nodes + fragment_nodes - 1;
        assert!(rejected_budget >= document_nodes);

        xmltree::set_test_max_attrs(2);
        xmltree::set_test_node_budget(rejected_budget);
        let mut rejected = Document::open(&bytes).unwrap();
        let over = rejected.fill_template_fields(values);
        xmltree::reset_test_node_budget();
        xmltree::set_test_max_attrs(65_536);
        assert!(over.is_err(), "story marker overflow should error");
        assert_eq!(
            unzip_parts(&rejected.save().unwrap()),
            before,
            "failed story preflight changed the package"
        );

        xmltree::set_test_max_attrs(2);
        xmltree::set_test_node_budget(header_nodes + fragment_nodes);
        let mut accepted = Document::open(&bytes).unwrap();
        let exact = accepted.fill_template_fields(values);
        xmltree::reset_test_node_budget();
        xmltree::set_test_max_attrs(65_536);
        assert_eq!(
            exact.unwrap(),
            2,
            "exact story budget or marker attribute path rejected"
        );
        assert_eq!(accepted.edited_parts(), ["word/header1.xml"]);
        let parts = unzip_parts(&accepted.save().unwrap());
        assert_eq!(parts["word/document.xml"], before["word/document.xml"]);
        let header = String::from_utf8(parts["word/header1.xml"].clone()).unwrap();
        assert!(
            header.contains("<w:tab/>") && header.contains("<w:br/>"),
            "{header}"
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn fill_content_control_markers_bypass_original_text_attribute_cap() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:sdt><w:sdtPr><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t w:id="1" w:rsid="2">Old</w:t></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);

        xmltree::set_test_max_attrs(2);
        let mut marked = Document::open(&bytes).unwrap();
        let marker_result =
            marked.fill_content_control_by_tag("client-name", " Leading\tTrailing ");
        xmltree::set_test_max_attrs(65_536);
        assert_eq!(
            marker_result.unwrap(),
            1,
            "marker fragments should not add xml:space to the original w:t"
        );
        let body =
            String::from_utf8(unzip_parts(&marked.save().unwrap())["word/document.xml"].clone())
                .unwrap();
        assert!(
            body.contains(
                r#"<w:t xml:space="preserve"> Leading</w:t><w:tab/><w:t xml:space="preserve">Trailing </w:t>"#
            ),
            "marker edge whitespace was not preserved: {body}"
        );

        let before = unzip_parts(&bytes);
        xmltree::set_test_max_attrs(2);
        let mut plain = Document::open(&bytes).unwrap();
        let plain_result = plain.fill_content_control_by_tag("client-name", " Plain ");
        xmltree::set_test_max_attrs(65_536);
        assert!(
            plain_result.is_err(),
            "plain edge whitespace should still require original xml:space capacity"
        );
        assert_eq!(
            unzip_parts(&plain.save().unwrap()),
            before,
            "failed attribute preflight changed the package"
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn fill_content_control_rolls_back_after_marker_cleanup_failure() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:sdt><w:sdtPr><w:tag w:val="client-name"/></w:sdtPr><w:sdtContent><w:p><w:r><w:t>Old</w:t><w:tab/></w:r></w:p></w:sdtContent></w:sdt></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);
        let before = unzip_parts(&bytes);
        let mut doc = Document::open(&bytes).unwrap();

        xmltree::set_test_fail_commit_after(1);
        let result = doc.fill_content_control_by_tag("client-name", "New\tValue");
        xmltree::reset_test_fail_commit();

        assert!(
            result.is_err(),
            "fragment insertion failure should surface after marker cleanup"
        );
        assert_eq!(
            unzip_parts(&doc.save().unwrap()),
            before,
            "failed marker replacement leaked a partial cleanup"
        );
    }

    /// `replace_body_text` preflights the node budget for matches
    /// that lack a reusable text carrier (empty `<w:t/>`), so it can't grow the arena
    /// past the budget — it errors cleanly and leaves the document untouched.
    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_respects_node_budget() {
        // Two empty w:t runs: replacing "" with text would allocate a node for each.
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t/></w:r><w:r><w:t/></w:r></w:p></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);
        let before = unzip_parts(&bytes);
        let doc_nodes = xmltree::XmlTree::parse(doc_xml.as_bytes())
            .unwrap()
            .node_count();
        // Budget allows parsing but not the 2 new text nodes the replacement needs.
        xmltree::set_test_node_budget(doc_nodes + 1);
        let mut doc = Document::open(&bytes).unwrap();
        let r = doc.replace_body_text("", "X");
        xmltree::reset_test_node_budget();
        assert!(r.is_err(), "over-budget text replace should error");
        // Untouched: a no-op save preserves document.xml verbatim.
        let after = unzip_parts(&doc.save().unwrap());
        assert_eq!(
            before.get("word/document.xml"),
            after.get("word/document.xml"),
            "document.xml changed despite a failed (over-budget) replace"
        );
    }

    /// `replace_body_text` preflights the *attribute* budget the
    /// same way it preflights the node budget. A `w:t` already at the attribute cap whose
    /// replacement needs `xml:space="preserve"` (edge whitespace) would otherwise grow to
    /// cap+1 attributes — an element `XmlTree::parse` would reject. It errors cleanly up
    /// front and leaves the document untouched (transactional, parse/edit symmetry).
    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_respects_attribute_budget() {
        // A `w:t` with two attributes; with the cap lowered to 2 it parses but has no room
        // for a new `xml:space`. (w:document carries one xmlns attr; everything else ≤ 2.)
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t w:id="1" w:rsid="2">OLD</w:t></w:r></w:p></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);
        xmltree::set_test_max_attrs(2);
        let mut doc = Document::open(&bytes).unwrap();
        // Replacement WITH edge whitespace needs `xml:space` → over the attribute cap.
        let over = doc.replace_body_text("OLD", " NEW ");
        // A no-op-whitespace replacement needs no new attribute and still succeeds; it
        // finds "OLD" only because the failed attempt above left no partial edit behind.
        let within = doc.replace_body_text("OLD", "NEW");
        xmltree::set_test_max_attrs(65_536);
        assert!(over.is_err(), "over-attribute-budget replace should error");
        assert_eq!(
            within.unwrap(),
            1,
            "non-whitespace replace within budget should apply (and prove no partial edit)"
        );
    }

    /// The clone-and-swap path makes `add_image_png` all-or-nothing
    /// even when a commit-time tree edit fails (the now-fallible `try_reserve` path). Using
    /// the commit-fail seam, the fragment insert fails AFTER `add_related_part` has committed
    /// the media part + relationship on the clone — and the document must be byte-identical
    /// (no orphaned media part). This test FAILS if the edit mutates the package in place.
    #[cfg(feature = "docx")]
    #[test]
    fn add_image_png_rolls_back_on_commit_failure() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p/></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);
        let before = unzip_parts(&bytes);
        // Fail the first commit-time edit (the fragment insert) — it runs only AFTER
        // add_related_part has committed the media part + rel on the clone.
        xmltree::set_test_fail_commit_after(0);
        let mut doc = Document::open(&bytes).unwrap();
        let r = doc.add_image_png(&tiny_png(), "image1.png");
        xmltree::reset_test_fail_commit();
        assert!(r.is_err(), "a commit-time failure must surface as Err");
        let after = unzip_parts(&doc.save().unwrap());
        assert!(
            !after.contains_key("word/media/image1.png"),
            "rollback failed: media part orphaned after a failed image insert"
        );
        assert_eq!(
            before, after,
            "a failed add_image_png must leave the package byte-identical"
        );
    }

    /// the clone-and-swap also makes `replace_body_text` all-or-nothing.
    /// With two matching runs and the second run's commit edit forced to fail, NEITHER run
    /// may be rewritten — a partial "NEW" would mean the in-place mutation leaked. FAILS if
    /// the loop edits the live package directly.
    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_rolls_back_on_commit_failure() {
        let doc_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD</w:t></w:r><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document>"#;
        let bytes = minimal_docx(doc_xml);
        let before = unzip_parts(&bytes);
        // First run's edit succeeds, second fails — a mid-loop commit failure.
        xmltree::set_test_fail_commit_after(1);
        let mut doc = Document::open(&bytes).unwrap();
        let r = doc.replace_body_text("OLD", "NEW");
        xmltree::reset_test_fail_commit();
        assert!(r.is_err(), "a mid-loop commit failure must surface as Err");
        let after = unzip_parts(&doc.save().unwrap());
        let doc_after = String::from_utf8_lossy(after.get("word/document.xml").unwrap());
        assert!(
            !doc_after.contains("NEW"),
            "rollback failed: a partial edit ('NEW') leaked from a failed replace"
        );
        assert_eq!(
            before.get("word/document.xml"),
            after.get("word/document.xml"),
            "document.xml changed despite a failed replace"
        );
    }

    /// A malformed (truncated) `document.xml` makes element-tree
    /// edits fail cleanly, and a no-op save still preserves the raw part byte-for-byte
    /// (the editor never invents close tags to "repair" damaged input).
    #[cfg(feature = "docx")]
    #[test]
    fn malformed_document_xml_edit_errs_but_passthrough_preserves() {
        // Unclosed <w:t>/<w:r>/<w:p>/<w:body>/<w:document>.
        let truncated = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD"#;
        let bytes = minimal_docx(truncated);
        let before = unzip_parts(&bytes);

        let mut doc = Document::open(&bytes).unwrap();
        assert!(
            doc.replace_body_text("OLD", "NEW").is_err(),
            "edit on malformed XML must error"
        );
        assert!(
            doc.add_image_png(&tiny_png(), "image1.png").is_err(),
            "image insert on malformed XML must error"
        );
        // No edit took hold ⇒ save is a passthrough that preserves the raw part.
        let after = unzip_parts(&doc.save().unwrap());
        assert_eq!(
            before.get("word/document.xml"),
            after.get("word/document.xml"),
            "no-op save must preserve the malformed part verbatim"
        );
    }

    /// A `document.xml` that is tokenizable but not a single well-formed
    /// document — multiple top-level elements, or non-whitespace text outside the root — is
    /// passthrough-only. Edits must NOT promote-and-rewrite it (which would leave malformed
    /// multi-root XML); they error and the raw part is preserved byte-for-byte. This FAILS
    /// with a fragment-tolerant body lookup (which would edit the first `w:document`).
    #[cfg(feature = "docx")]
    #[test]
    fn multi_root_or_junk_document_xml_is_passthrough_only() {
        const NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
        let cases = [
            // Two top-level <w:document> elements.
            format!(
                r#"<w:document xmlns:w="{NS}"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document><w:document xmlns:w="{NS}"/>"#
            ),
            // Non-whitespace character data after the root element.
            format!(
                r#"<w:document xmlns:w="{NS}"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document>junk"#
            ),
        ];
        for body in cases {
            let bytes = minimal_docx(&body);
            let before = unzip_parts(&bytes);
            let mut doc = Document::open(&bytes).unwrap();
            assert!(
                doc.replace_body_text("OLD", "NEW").is_err(),
                "a non-single-document document.xml must not be editable"
            );
            assert!(
                doc.add_image_png(&tiny_png(), "image1.png").is_err(),
                "a non-single-document document.xml must not accept an image"
            );
            let after = unzip_parts(&doc.save().unwrap());
            assert_eq!(
                before.get("word/document.xml"),
                after.get("word/document.xml"),
                "passthrough must preserve the raw (malformed) document.xml"
            );
        }
    }

    /// PR5: element-tree edit (B) preserves unmodeled body content. Replacing a run
    /// keeps the content control, field, mc:AlternateContent shape, comment
    /// reference, AND the comments.xml satellite.
    #[cfg(feature = "docx")]
    #[test]
    fn edit_preserves_unmodeled_body() {
        let orig = docx_rich_body();
        let before = unzip_parts(&orig);
        let mut doc = Document::open(&orig).unwrap();

        let changed = doc.replace_body_text("OLD", "NEW").unwrap();
        assert_eq!(changed, 1, "expected exactly one run replaced");
        let saved = doc.save().unwrap();
        let after = unzip_parts(&saved);
        let body = String::from_utf8_lossy(&after["word/document.xml"]);

        assert!(
            body.contains("NEW") && !body.contains("OLD"),
            "edit not applied: {body}"
        );
        for needle in [
            "w:sdt",
            "SDT-CONTENT",
            "w:fldSimple",
            "w:instr=\" PAGE \"",
            "mc:AlternateContent",
            "mc:Choice",
            "w:commentReference",
        ] {
            assert!(body.contains(needle), "B edit dropped {needle}: {body}");
        }
        // The comments.xml satellite is untouched, byte-for-byte.
        assert_eq!(
            after.get("word/comments.xml"),
            before.get("word/comments.xml"),
            "comments.xml not preserved"
        );
        // Re-opens cleanly.
        assert!(Document::open(&saved).is_ok());
    }

    /// PR5: lazy promotion — a body edit re-serializes ONLY document.xml; every
    /// other part stays byte-identical.
    #[cfg(feature = "docx")]
    #[test]
    fn lazy_parse_byte_stable() {
        let orig = docx_rich_body();
        let before = unzip_parts(&orig);
        let mut doc = Document::open(&orig).unwrap();
        doc.replace_body_text("OLD", "NEW").unwrap();
        let after = unzip_parts(&doc.save().unwrap());

        for (name, bytes) in &before {
            if name == "word/document.xml" {
                assert_ne!(bytes, &after[name], "document.xml should have changed");
            } else {
                assert_eq!(Some(bytes), after.get(name), "{name} should be byte-stable");
            }
        }
    }

    /// PR5: inserting an image reconciles relationships transactionally — new media
    /// part + content-type + a non-colliding rId the body's blip references.
    #[cfg(feature = "docx")]
    #[test]
    fn insert_image_reconciles_rels() {
        let png = tiny_png();
        let mut doc = Document::open(&docx_rich_body()).unwrap();
        doc.add_image_png(&png, "image1.png").unwrap();
        let saved = doc.save().unwrap();
        let parts = unzip_parts(&saved);

        assert_eq!(
            parts.get("word/media/image1.png"),
            Some(&png),
            "media not added"
        );
        let ct = String::from_utf8_lossy(&parts["[Content_Types].xml"]);
        assert!(ct.contains("image/png"), "png content-type missing: {ct}");
        let rels = String::from_utf8_lossy(&parts["word/_rels/document.xml.rels"]);
        assert!(
            rels.contains("media/image1.png"),
            "image rel missing: {rels}"
        );
        let body = String::from_utf8_lossy(&parts["word/document.xml"]);
        assert!(
            body.contains("a:blip") && body.contains("r:embed"),
            "drawing missing: {body}"
        );
        let rid = {
            let i = body.find("r:embed=\"").unwrap() + 9;
            let s = &body[i..];
            s[..s.find('"').unwrap()].to_string()
        };

        // Structural assertions via the crate's own OPC parser (not substring checks):
        let pkg = crate::opc::Package::from_zip(&saved).unwrap();
        // The media part resolves to a content type (Override or png Default).
        assert!(
            pkg.part_has_content_type("word/media/image1.png"),
            "media part has no resolvable content type"
        );
        // Exactly one image relationship, its Id is the blip's rId, and every rId on
        // document.xml is unique (no dangling/colliding reference).
        let doc_rels = pkg.rels_for("word/document.xml");
        let imgs: Vec<_> = doc_rels
            .iter()
            .filter(|r| r.rel_type.ends_with("/image") && !r.external)
            .collect();
        assert_eq!(imgs.len(), 1, "expected exactly one image rel");
        assert_eq!(imgs[0].id, rid, "blip rId does not match the image rel Id");
        assert!(
            imgs[0].target.ends_with("media/image1.png"),
            "image rel target wrong"
        );
        let mut ids: Vec<&String> = doc_rels.iter().map(|r| &r.id).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), n, "duplicate rIds on document.xml");

        // Re-opens and the image is extractable through the reader.
        let re = Document::open(&saved).unwrap();
        assert!(
            !re.images().is_empty(),
            "inserted image not extractable on reopen"
        );
    }

    /// An inserted image goes BEFORE the body's final `w:sectPr`,
    /// which OOXML requires to stay last.
    #[cfg(feature = "docx")]
    #[test]
    fn add_image_inserts_before_sectpr() {
        let mut doc = Document::new(); // blank template ends its body with sectPr
        doc.add_image_png(&tiny_png(), "image1.png").unwrap();
        let saved = doc.save().unwrap();
        let body = String::from_utf8(unzip_parts(&saved)["word/document.xml"].clone()).unwrap();
        let sect = body.rfind("<w:sectPr").expect("sectPr present");
        let draw = body.find("<w:drawing").expect("drawing inserted");
        assert!(draw < sect, "image must precede the final sectPr:\n{body}");
        // sectPr is still the last body-level element (nothing after its close).
        let tail = &body[body.rfind("</w:sectPr>").unwrap()..];
        assert!(
            !tail.contains("<w:p"),
            "a paragraph follows sectPr (invalid order): {tail}"
        );
        assert_eq!(Document::open(&saved).unwrap().images().len(), 1);
    }

    /// `replace_body_text` edits WordprocessingML `w:t` only, not
    /// DrawingML `a:t` inside shapes/charts.
    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_skips_drawingml_text() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        for (n, b) in [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/document.xml",
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p><w:p><w:r><w:drawing><a:t>OLD</a:t></w:drawing></w:r></w:p></w:body></w:document>"#,
            ),
        ] {
            zw.start_file(n, opt).unwrap();
            zw.write_all(b.as_bytes()).unwrap();
        }
        let bytes = zw.finish().unwrap().into_inner();
        let mut doc = Document::open(&bytes).unwrap();
        let n = doc.replace_body_text("OLD", "NEW").unwrap();
        assert_eq!(n, 1, "should edit only the w:t run");
        let body =
            String::from_utf8(unzip_parts(&doc.save().unwrap())["word/document.xml"].clone())
                .unwrap();
        assert!(body.contains("<w:t>NEW</w:t>"), "w:t not edited: {body}");
        assert!(
            body.contains("<a:t>OLD</a:t>"),
            "a:t wrongly edited: {body}"
        );
    }

    /// `try_new` is a non-panicking constructor.
    #[cfg(feature = "docx")]
    #[test]
    fn try_new_yields_valid_blank() {
        let doc = Document::try_new().unwrap();
        assert!(doc.text().trim().is_empty());
        assert!(Document::open(&doc.save().unwrap()).is_ok());
    }

    /// Helper: a one-part `.docx` wrapping the given `<w:body>` inner XML.
    #[cfg(feature = "docx")]
    fn docx_with_body_xml(document_xml: &str) -> Vec<u8> {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        for (n, b) in [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            ("word/document.xml", document_xml),
        ] {
            zw.start_file(n, opt).unwrap();
            zw.write_all(b.as_bytes()).unwrap();
        }
        zw.finish().unwrap().into_inner()
    }

    /// `replace_body_text` resolves namespaces — it
    /// skips a bare `<t>` under a `w:drawing` that binds DrawingML as the DEFAULT
    /// namespace, while still editing the real `w:t`.
    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_skips_default_ns_drawingml() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p><w:p><w:r><w:drawing xmlns="http://schemas.openxmlformats.org/drawingml/2006/main"><t>OLD</t></w:drawing></w:r></w:p></w:body></w:document>"#;
        let mut doc = Document::open(&docx_with_body_xml(xml)).unwrap();
        let n = doc.replace_body_text("OLD", "NEW").unwrap();
        assert_eq!(n, 1, "should edit only the WordprocessingML w:t");
        let body =
            String::from_utf8(unzip_parts(&doc.save().unwrap())["word/document.xml"].clone())
                .unwrap();
        assert!(body.contains("<w:t>NEW</w:t>"), "w:t not edited: {body}");
        assert!(
            body.contains("<t>OLD</t>"),
            "default-ns DrawingML text wrongly edited: {body}"
        );
    }

    /// Namespace resolution edits genuine `w:t` even inside a text
    /// box nested under `w:drawing` (which the earlier blanket-skip approach missed).
    #[cfg(feature = "docx")]
    #[test]
    fn replace_body_text_edits_textbox_wml() {
        let xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:drawing><wps:txbx xmlns:wps="urn:wps"><w:txbxContent><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:txbxContent></wps:txbx></w:drawing></w:r></w:p></w:body></w:document>"#;
        let mut doc = Document::open(&docx_with_body_xml(xml)).unwrap();
        let n = doc.replace_body_text("OLD", "NEW").unwrap();
        assert_eq!(n, 1, "text-box w:t should be editable");
        let body =
            String::from_utf8(unzip_parts(&doc.save().unwrap())["word/document.xml"].clone())
                .unwrap();
        assert!(
            body.contains("<w:t>NEW</w:t>"),
            "text-box w:t not edited: {body}"
        );
    }

    /// `add_image_png` rejects non-PNG, forged-framing, AND
    /// CRC-correct-but-semantically-invalid PNG bytes (the validator checks chunk CRCs,
    /// IHDR fields, and non-empty IDAT — a correct signature/CRC is not enough).
    #[cfg(feature = "docx")]
    #[test]
    fn add_image_png_rejects_non_png() {
        const SIG: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        // A correctly-framed, correctly-CRC'd chunk builder (uses the crate's own crc32).
        fn chunk(typ: &[u8; 4], data: &[u8]) -> Vec<u8> {
            let mut c = (data.len() as u32).to_be_bytes().to_vec();
            c.extend_from_slice(typ);
            c.extend_from_slice(data);
            let crc = super::crc32(&[&typ[..], data].concat());
            c.extend_from_slice(&crc.to_be_bytes());
            c
        }
        // The 11-byte zlib IDAT payload from a real 2×3 PNG.
        let real_idat = &[
            0x78u8, 0xDA, 0x63, 0x60, 0xC0, 0x02, 0x00, 0x00, 0x15, 0x00, 0x01,
        ];

        let mut bad: Vec<Vec<u8>> = vec![
            b"this is not a png".to_vec(),
            SIG.to_vec(), // signature only
            // first chunk spells IHDR but wrong length (5 ≠ 13)
            [SIG, &[0, 0, 0, 5], b"IHDR", &[0; 9]].concat(),
        ];
        // Bad CRC on an otherwise well-framed IHDR.
        {
            let mut v = SIG.to_vec();
            v.extend_from_slice(&[0, 0, 0, 13]);
            v.extend_from_slice(b"IHDR");
            v.extend_from_slice(&[0, 0, 0, 2, 0, 0, 0, 3, 8, 2, 0, 0, 0]);
            v.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]); // wrong CRC
            bad.push(v);
        }
        // CRC-CORRECT but IMPOSSIBLE color type (99): every CRC valid, still not a PNG.
        {
            let mut v = SIG.to_vec();
            v.extend(chunk(b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 3, 8, 99, 0, 0, 0]));
            v.extend(chunk(b"IDAT", real_idat));
            v.extend(chunk(b"IEND", &[]));
            bad.push(v);
        }
        // CRC-CORRECT valid IHDR but EMPTY IDAT (no image data).
        {
            let mut v = SIG.to_vec();
            v.extend(chunk(b"IHDR", &[0, 0, 0, 2, 0, 0, 0, 3, 8, 2, 0, 0, 0]));
            v.extend(chunk(b"IDAT", &[]));
            v.extend(chunk(b"IEND", &[]));
            bad.push(v);
        }
        // A real PNG with trailing junk after IEND.
        let mut trailing = tiny_png();
        trailing.extend_from_slice(b"junk");
        bad.push(trailing);

        for (i, b) in bad.iter().enumerate() {
            let mut doc = Document::open(&docx_rich_body()).unwrap();
            assert!(
                doc.add_image_png(b, "x.png").is_err(),
                "invalid PNG #{i} was accepted"
            );
        }
        // A genuinely valid PNG is accepted.
        let mut ok = Document::open(&docx_rich_body()).unwrap();
        assert!(ok.add_image_png(&tiny_png(), "x.png").is_ok());
    }

    /// `add_image_png` rejects unsafe names and existing parts.
    #[cfg(feature = "docx")]
    #[test]
    fn add_image_rejects_bad_names() {
        let png = tiny_png();
        for bad in ["../evil.png", "a/b.png", "dir/", "no-ext", "img.jpg", ""] {
            let mut doc = Document::open(&docx_rich_body()).unwrap();
            assert!(
                doc.add_image_png(&png, bad).is_err(),
                "accepted bad name {bad:?}"
            );
        }
        // Existing media name is rejected (no overwrite).
        let mut doc = Document::open(&docx_rich_body()).unwrap();
        doc.add_image_png(&png, "image1.png").unwrap();
        assert!(
            doc.add_image_png(&png, "image1.png").is_err(),
            "overwrote existing media"
        );
    }

    #[cfg(feature = "docx")]
    #[test]
    fn add_image_rejects_case_variant_existing_media_part() {
        use zip::write::SimpleFileOptions;

        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        for (n, b) in [
            (
                "[Content_Types].xml",
                format!(
                    r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="png" ContentType="image/png"/><Override PartName="/word/document.xml" ContentType="{CT_DOCUMENT_MAIN}"/></Types>"#
                )
                .into_bytes(),
            ),
            (
                "_rels/.rels",
                br#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_vec(),
            ),
            (
                "word/document.xml",
                br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>OLD</w:t></w:r></w:p></w:body></w:document>"#.to_vec(),
            ),
            ("word/media/Image1.png", tiny_png()),
        ] {
            zw.start_file(n, opt).unwrap();
            zw.write_all(&b).unwrap();
        }
        let bytes = zw.finish().unwrap().into_inner();
        let mut doc = Document::open(&bytes).unwrap();

        assert!(
            doc.add_image_png(&tiny_png(), "image1.png").is_err(),
            "case-variant media part must be treated as an existing part"
        );

        let parts = unzip_parts(&doc.save().unwrap());
        assert!(parts.contains_key("word/media/Image1.png"));
        assert!(
            !parts.contains_key("word/media/image1.png"),
            "failed insert left a case-variant duplicate media part"
        );
    }

    /// A failed `add_image_png` (no `w:body`) leaves the package
    /// unchanged — no orphaned media part or relationship.
    #[cfg(feature = "docx")]
    #[test]
    fn add_image_rolls_back_without_body() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        for (n, b) in [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/document.xml",
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"></w:document>"#,
            ),
        ] {
            zw.start_file(n, opt).unwrap();
            zw.write_all(b.as_bytes()).unwrap();
        }
        let bytes = zw.finish().unwrap().into_inner();
        let mut doc = Document::open(&bytes).unwrap();
        assert!(doc.add_image_png(&tiny_png(), "image1.png").is_err());
        // No orphaned media part nor image relationship was persisted.
        let parts = unzip_parts(&doc.save().unwrap());
        assert!(
            !parts.contains_key("word/media/image1.png"),
            "orphaned media"
        );
        let rels = parts
            .get("word/_rels/document.xml.rels")
            .map(|b| String::from_utf8_lossy(b).into_owned())
            .unwrap_or_default();
        assert!(!rels.contains("media/image1.png"), "orphaned rel: {rels}");
    }

    /// Two inserted images get distinct drawing ids, and insertion
    /// works when the host binds WordprocessingML as the default namespace (no `w:`).
    #[cfg(feature = "docx")]
    #[test]
    fn add_image_unique_ids_and_default_ns_host() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;
        let mut zw = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        // Host binds the main namespace as DEFAULT (elements have no `w:` prefix).
        for (n, b) in [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/document.xml",
                r#"<document xmlns="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><body><p><r><t>hi</t></r></p></body></document>"#,
            ),
        ] {
            zw.start_file(n, opt).unwrap();
            zw.write_all(b.as_bytes()).unwrap();
        }
        let bytes = zw.finish().unwrap().into_inner();
        let mut doc = Document::open(&bytes).unwrap();
        doc.add_image_png(&tiny_png(), "image1.png").unwrap();
        doc.add_image_png(&tiny_png(), "image2.png").unwrap();
        let saved = doc.save().unwrap();
        let parts = unzip_parts(&saved);
        let body = String::from_utf8_lossy(&parts["word/document.xml"]);
        // Two distinct docPr ids (1 and 2), not duplicated "1".
        assert!(body.contains(r#"docPr id="1""#), "first drawing id: {body}");
        assert!(
            body.contains(r#"docPr id="2""#),
            "second drawing id not unique: {body}"
        );
        // python-docx-grade validity: re-opens and both images extract.
        assert_eq!(Document::open(&saved).unwrap().images().len(), 2);
    }

    /// `set_part` corrects a stale/mismatched content-type override
    /// rather than leaving the wrong one.
    #[cfg(feature = "docx")]
    #[test]
    fn set_part_updates_mismatched_content_type() {
        let mut pkg = crate::opc::Package::from_zip(&docx_rich_body()).unwrap();
        // Re-type document.xml with a (deliberately wrong then) corrected override.
        pkg.set_part(
            "word/document.xml",
            b"<w:document/>".to_vec(),
            Some("application/xml"),
        );
        pkg.set_part(
            "word/document.xml",
            b"<w:document/>".to_vec(),
            Some(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
            ),
        );
        let ct = String::from_utf8(pkg.part("[Content_Types].xml").unwrap()).unwrap();
        assert_eq!(
            ct.matches("/word/document.xml").count(),
            1,
            "duplicate override for the same part: {ct}"
        );
        assert!(
            ct.contains("document.main+xml"),
            "override not corrected: {ct}"
        );
        assert!(!ct.contains(r#"PartName="/word/document.xml" ContentType="application/xml""#));
    }
}
