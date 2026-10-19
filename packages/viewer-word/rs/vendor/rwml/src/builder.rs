//! Ergonomic authoring helpers over [`crate::DocModel`].

use crate::model::{
    normalize_field_instruction, referenceable_bookmark_name, Align, AuthoredComment,
    AuthoredContentControl, AuthoredNote, AuthoredRevision, Block, Cell, CharProps, Chart,
    ChartKind, ChartSeries, ChartShape, Color, CustomXmlItem, DocGrid, DocGridType, DocModel,
    FieldRole, Image, ListInfo, PageNumberFormat, PageSetup, ParaProps, Paragraph, ParagraphStyle,
    Row, Run, SectionBreakKind, SectionSetup, Table, TableBorderSide, TableBorderStyle,
    TextDirection, VCell, WebExtensionTaskPane,
};
use crate::{NoteKind, RevisionKind};

/// Thin builder for an inline [`Run`] with character formatting.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunBuilder {
    run: Run,
}

impl RunBuilder {
    /// Start a text run.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            run: Run {
                text: text.into(),
                ..Run::default()
            },
        }
    }

    /// Mark the run as bold.
    pub fn bold(mut self) -> Self {
        self.run.props.bold = true;
        self
    }

    /// Mark the run as italic.
    pub fn italic(mut self) -> Self {
        self.run.props.italic = true;
        self
    }

    /// Mark the run as underlined.
    pub fn underline(mut self) -> Self {
        self.run.props.underline = true;
        self
    }

    /// Use small caps for the run.
    pub fn small_caps(mut self) -> Self {
        self.run.props.small_caps = true;
        self
    }

    /// Use all caps for the run.
    pub fn caps(mut self) -> Self {
        self.run.props.caps = true;
        self
    }

    /// Mark the run as right-to-left text.
    pub fn rtl(mut self) -> Self {
        self.run.props.rtl = true;
        self
    }

    /// Set the run font family.
    pub fn font(mut self, font: impl Into<String>) -> Self {
        let font = font.into().trim().to_string();
        self.run.props.font = (!font.is_empty()).then_some(font);
        self
    }

    /// Set the run size in Word half-points (`24` = 12pt).
    pub fn size_half_pt(mut self, size: u16) -> Self {
        self.run.props.size_half_pt = Some(size);
        self
    }

    /// Set the run text color.
    pub fn color(mut self, color: Color) -> Self {
        self.run.props.color = Some(color);
        self
    }

    /// Set the run highlight color name, such as `"yellow"`.
    pub fn highlight(mut self, highlight: impl Into<String>) -> Self {
        let highlight = highlight.into().trim().to_string();
        self.run.props.highlight = (!highlight.is_empty()).then_some(highlight);
        self
    }

    /// Mark the run as the cached result of a simple Word field.
    pub fn field(mut self, instruction: impl Into<String>) -> Self {
        let instruction = normalize_field_instruction(&instruction.into());
        if !instruction.is_empty() {
            self.run.field = FieldRole::Simple { instruction };
        }
        self
    }

    /// Mark the run as the cached result of a hyperlink-style `PAGEREF` field.
    pub fn page_ref(self, bookmark: impl Into<String>) -> Self {
        let bookmark = bookmark.into().trim().to_string();
        if referenceable_bookmark_name(&bookmark) {
            self.field(format!("PAGEREF {bookmark} \\h"))
        } else {
            self
        }
    }

    /// Mark the run as a hyperlink. A target beginning with `#` names an
    /// internal document bookmark; other targets use an external relationship.
    pub fn hyperlink(mut self, url: impl Into<String>) -> Self {
        self.run.field = hyperlink_field(url.into());
        self
    }

    /// Anchor an authored comment to this run.
    pub fn comment<C>(mut self, comment: C) -> Self
    where
        C: Into<AuthoredComment>,
    {
        self.run.comment = Some(comment.into());
        self
    }

    /// Mark the run as an authored tracked insertion/deletion.
    pub fn revision<R>(mut self, revision: R) -> Self
    where
        R: Into<AuthoredRevision>,
    {
        self.run.revision = Some(revision.into());
        self
    }

    /// Wrap the run in a generated plain text content control.
    pub fn content_control<C>(mut self, control: C) -> Self
    where
        C: Into<AuthoredContentControl>,
    {
        self.run.content_control = Some(control.into());
        self
    }

    /// Wrap this run in a generated bookmark.
    pub fn bookmark(mut self, name: impl Into<String>) -> Self {
        let name = name.into().trim().to_string();
        if referenceable_bookmark_name(&name) {
            self.run.bookmark = Some(name);
        }
        self
    }

    /// Anchor an authored footnote after this run.
    pub fn footnote(mut self, text: impl Into<String>) -> Self {
        self.run.note = Some(AuthoredNote {
            kind: NoteKind::Footnote,
            text: text.into(),
        });
        self
    }

    /// Anchor an authored endnote after this run.
    pub fn endnote(mut self, text: impl Into<String>) -> Self {
        self.run.note = Some(AuthoredNote {
            kind: NoteKind::Endnote,
            text: text.into(),
        });
        self
    }

    /// Finish and return the run.
    pub fn build(self) -> Run {
        self.run
    }
}

/// Thin builder for a paragraph with runs and paragraph-level layout.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParagraphBuilder {
    paragraph: Paragraph,
}

impl ParagraphBuilder {
    /// Start an empty paragraph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a paragraph containing one plain text run.
    pub fn text(text: impl Into<String>) -> Self {
        Self::new().push_run(plain_run(text.into(), CharProps::default()))
    }

    /// Replace the paragraph's runs.
    pub fn runs<I>(mut self, runs: I) -> Self
    where
        I: IntoIterator<Item = Run>,
    {
        self.paragraph.runs = runs.into_iter().collect();
        self
    }

    /// Append an already-built run.
    pub fn push_run(mut self, run: Run) -> Self {
        self.paragraph.runs.push(run);
        self
    }

    /// Mark the paragraph as a heading. Levels outside `1..=6` are clamped.
    pub fn heading_level(mut self, level: u8) -> Self {
        self.paragraph.props.heading_level = Some(level.clamp(1, 6));
        self
    }

    /// Apply a paragraph style id.
    pub fn style(mut self, style_id: impl Into<String>) -> Self {
        let style_id = style_id.into().trim().to_string();
        self.paragraph.props.style_id = (!style_id.is_empty()).then_some(style_id);
        self
    }

    /// Set paragraph alignment.
    pub fn align(mut self, align: Align) -> Self {
        self.paragraph.props.align = align;
        self
    }

    /// Set spacing before the paragraph in points.
    pub fn spacing_before_pt(mut self, before_pt: f32) -> Self {
        self.paragraph.props.spacing.before_pt = Some(before_pt);
        self
    }

    /// Set spacing after the paragraph in points.
    pub fn spacing_after_pt(mut self, after_pt: f32) -> Self {
        self.paragraph.props.spacing.after_pt = Some(after_pt);
        self
    }

    /// Set line height as a multiple of font size.
    pub fn line_pct(mut self, line_pct: f32) -> Self {
        self.paragraph.props.spacing.line_pct = Some(line_pct);
        self
    }

    /// Set left indent in points.
    pub fn indent_left_pt(mut self, left_pt: f32) -> Self {
        self.paragraph.props.indent.left_pt = Some(left_pt);
        self
    }

    /// Set right indent in points.
    pub fn indent_right_pt(mut self, right_pt: f32) -> Self {
        self.paragraph.props.indent.right_pt = Some(right_pt);
        self
    }

    /// Set first-line indent in points.
    pub fn first_line_pt(mut self, first_line_pt: f32) -> Self {
        self.paragraph.props.indent.first_line_pt = Some(first_line_pt);
        self
    }

    /// Set hanging indent in points.
    pub fn hanging_pt(mut self, hanging_pt: f32) -> Self {
        self.paragraph.props.indent.hanging_pt = Some(hanging_pt);
        self
    }

    /// Set paragraph background shading.
    pub fn shading(mut self, color: Color) -> Self {
        self.paragraph.props.shading = Some(color);
        self
    }

    /// Force the paragraph to begin on a new page.
    pub fn page_break_before(mut self) -> Self {
        self.paragraph.props.page_break_before = true;
        self
    }

    /// Use right-to-left paragraph layout.
    pub fn bidi(mut self) -> Self {
        self.paragraph.props.bidi = true;
        self
    }

    /// Finish and return the paragraph.
    pub fn build(self) -> Paragraph {
        self.paragraph
    }
}

impl From<ParagraphBuilder> for Paragraph {
    fn from(builder: ParagraphBuilder) -> Self {
        builder.build()
    }
}

/// Thin builder for a paragraph style definition.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParagraphStyleBuilder {
    style: ParagraphStyle,
}

impl ParagraphStyleBuilder {
    /// Start a paragraph style definition with a style id and display name.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            style: ParagraphStyle {
                id: id.into().trim().to_string(),
                name: name.into().trim().to_string(),
                ..ParagraphStyle::default()
            },
        }
    }

    /// Continue building from an existing style definition.
    pub fn from_style(style: ParagraphStyle) -> Self {
        Self { style }
    }

    /// Set the base style id.
    pub fn based_on(mut self, style_id: impl Into<String>) -> Self {
        let style_id = style_id.into().trim().to_string();
        self.style.based_on = (!style_id.is_empty()).then_some(style_id);
        self
    }

    /// Set the next paragraph style id.
    pub fn next(mut self, style_id: impl Into<String>) -> Self {
        let style_id = style_id.into().trim().to_string();
        self.style.next = (!style_id.is_empty()).then_some(style_id);
        self
    }

    /// Mark the style as a quick style.
    pub fn q_format(mut self) -> Self {
        self.style.q_format = true;
        self
    }

    /// Mark this style as a heading style. Levels outside `1..=9` are clamped.
    pub fn heading_level(mut self, level: u8) -> Self {
        self.style.heading_level = Some(level.clamp(1, 9));
        self
    }

    /// Set default paragraph alignment.
    pub fn align(mut self, align: Align) -> Self {
        self.style.align = align;
        self
    }

    /// Set default spacing before paragraphs in points.
    pub fn spacing_before_pt(mut self, before_pt: f32) -> Self {
        self.style.spacing.before_pt = Some(before_pt);
        self
    }

    /// Set default spacing after paragraphs in points.
    pub fn spacing_after_pt(mut self, after_pt: f32) -> Self {
        self.style.spacing.after_pt = Some(after_pt);
        self
    }

    /// Set default line height as a multiple of font size.
    pub fn line_pct(mut self, line_pct: f32) -> Self {
        self.style.spacing.line_pct = Some(line_pct);
        self
    }

    /// Set default left indent in points.
    pub fn indent_left_pt(mut self, left_pt: f32) -> Self {
        self.style.indent.left_pt = Some(left_pt);
        self
    }

    /// Set default right indent in points.
    pub fn indent_right_pt(mut self, right_pt: f32) -> Self {
        self.style.indent.right_pt = Some(right_pt);
        self
    }

    /// Set default first-line indent in points.
    pub fn first_line_pt(mut self, first_line_pt: f32) -> Self {
        self.style.indent.first_line_pt = Some(first_line_pt);
        self
    }

    /// Set default hanging indent in points.
    pub fn hanging_pt(mut self, hanging_pt: f32) -> Self {
        self.style.indent.hanging_pt = Some(hanging_pt);
        self
    }

    /// Set default paragraph background shading.
    pub fn shading(mut self, color: Color) -> Self {
        self.style.shading = Some(color);
        self
    }

    /// Mark default runs as bold.
    pub fn run_bold(mut self) -> Self {
        self.style.run.bold = true;
        self
    }

    /// Mark default runs as italic.
    pub fn run_italic(mut self) -> Self {
        self.style.run.italic = true;
        self
    }

    /// Mark default runs as underlined.
    pub fn run_underline(mut self) -> Self {
        self.style.run.underline = true;
        self
    }

    /// Set the default run font family.
    pub fn run_font(mut self, font: impl Into<String>) -> Self {
        let font = font.into().trim().to_string();
        self.style.run.font = (!font.is_empty()).then_some(font);
        self
    }

    /// Set the default run size in Word half-points (`24` = 12pt).
    pub fn run_size_half_pt(mut self, size: u16) -> Self {
        self.style.run.size_half_pt = Some(size);
        self
    }

    /// Set the default run text color.
    pub fn run_color(mut self, color: Color) -> Self {
        self.style.run.color = Some(color);
        self
    }

    /// Set the default run highlight color name, such as `"yellow"`.
    pub fn run_highlight(mut self, highlight: impl Into<String>) -> Self {
        let highlight = highlight.into().trim().to_string();
        self.style.run.highlight = (!highlight.is_empty()).then_some(highlight);
        self
    }

    /// Finish and return the style definition.
    pub fn build(self) -> ParagraphStyle {
        self.style
    }
}

impl From<ParagraphStyleBuilder> for ParagraphStyle {
    fn from(builder: ParagraphStyleBuilder) -> Self {
        builder.build()
    }
}

/// Thin builder for a generated Word comment body and metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommentBuilder {
    comment: AuthoredComment,
}

impl CommentBuilder {
    /// Start a generated comment with body text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            comment: AuthoredComment {
                text: text.into(),
                ..AuthoredComment::default()
            },
        }
    }

    /// Continue building from an existing generated comment.
    pub fn from_comment(comment: AuthoredComment) -> Self {
        Self { comment }
    }

    /// Set the comment author.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        let author = author.into().trim().to_string();
        self.comment.author = (!author.is_empty()).then_some(author);
        self
    }

    /// Set the comment author initials.
    pub fn initials(mut self, initials: impl Into<String>) -> Self {
        let initials = initials.into().trim().to_string();
        self.comment.initials = (!initials.is_empty()).then_some(initials);
        self
    }

    /// Set the comment timestamp.
    pub fn date(mut self, date: impl Into<String>) -> Self {
        let date = date.into().trim().to_string();
        self.comment.date = (!date.is_empty()).then_some(date);
        self
    }

    /// Set the parent comment id for a generated reply.
    pub fn parent_comment_id(mut self, id: impl Into<String>) -> Self {
        let id = id.into().trim().to_string();
        self.comment.parent_comment_id = (!id.is_empty()).then_some(id);
        self
    }

    /// Finish and return the generated comment metadata.
    pub fn build(self) -> AuthoredComment {
        self.comment
    }
}

impl From<CommentBuilder> for AuthoredComment {
    fn from(builder: CommentBuilder) -> Self {
        builder.build()
    }
}

/// Thin builder for generated tracked insertion/deletion metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionBuilder {
    revision: AuthoredRevision,
}

impl RevisionBuilder {
    /// Start a generated tracked insertion.
    pub fn insertion() -> Self {
        Self {
            revision: AuthoredRevision {
                kind: RevisionKind::Insertion,
                ..AuthoredRevision::default()
            },
        }
    }

    /// Start a generated tracked deletion.
    pub fn deletion() -> Self {
        Self {
            revision: AuthoredRevision {
                kind: RevisionKind::Deletion,
                ..AuthoredRevision::default()
            },
        }
    }

    /// Continue building from existing generated revision metadata.
    pub fn from_revision(revision: AuthoredRevision) -> Self {
        Self { revision }
    }

    /// Set the revision author.
    pub fn author(mut self, author: impl Into<String>) -> Self {
        let author = author.into().trim().to_string();
        self.revision.author = (!author.is_empty()).then_some(author);
        self
    }

    /// Set the revision timestamp.
    pub fn date(mut self, date: impl Into<String>) -> Self {
        let date = date.into().trim().to_string();
        self.revision.date = (!date.is_empty()).then_some(date);
        self
    }

    /// Finish and return the generated revision metadata.
    pub fn build(self) -> AuthoredRevision {
        self.revision
    }
}

impl Default for RevisionBuilder {
    fn default() -> Self {
        Self::insertion()
    }
}

impl From<RevisionBuilder> for AuthoredRevision {
    fn from(builder: RevisionBuilder) -> Self {
        builder.build()
    }
}

/// Thin builder for generated plain text content-control metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentControlBuilder {
    control: AuthoredContentControl,
}

impl ContentControlBuilder {
    /// Start a generated plain text content control.
    pub fn new() -> Self {
        Self::default()
    }

    /// Continue building from existing generated content-control metadata.
    pub fn from_content_control(control: AuthoredContentControl) -> Self {
        Self { control }
    }

    /// Set the content-control alias/title.
    pub fn alias(mut self, alias: impl Into<String>) -> Self {
        let alias = alias.into().trim().to_string();
        self.control.alias = (!alias.is_empty()).then_some(alias);
        self
    }

    /// Set the content-control tag.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        let tag = tag.into().trim().to_string();
        self.control.tag = (!tag.is_empty()).then_some(tag);
        self
    }

    /// Bind this content control to a custom XML item.
    pub fn data_binding(
        mut self,
        xpath: impl Into<String>,
        store_item_id: impl Into<String>,
    ) -> Self {
        let xpath = xpath.into().trim().to_string();
        let store_item_id = store_item_id.into().trim().to_string();
        if !xpath.is_empty() && !store_item_id.is_empty() {
            self.control.data_binding_xpath = Some(xpath);
            self.control.data_binding_store_item_id = Some(store_item_id);
        } else {
            self.control.data_binding_xpath = None;
            self.control.data_binding_store_item_id = None;
        }
        self
    }

    /// Finish and return the generated content-control metadata.
    pub fn build(self) -> AuthoredContentControl {
        self.control
    }
}

impl From<ContentControlBuilder> for AuthoredContentControl {
    fn from(builder: ContentControlBuilder) -> Self {
        builder.build()
    }
}

/// Thin builder for a table cell with block content and layout metadata.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CellBuilder {
    cell: Cell,
}

impl CellBuilder {
    /// Start an empty cell.
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a cell containing one plain paragraph.
    pub fn text(text: impl Into<String>) -> Self {
        Self::new().paragraph(text)
    }

    /// Replace the cell content with already-constructed blocks.
    pub fn blocks<I>(mut self, blocks: I) -> Self
    where
        I: IntoIterator<Item = Block>,
    {
        self.cell.blocks = blocks.into_iter().collect();
        self
    }

    /// Append one plain paragraph to the cell.
    pub fn paragraph(mut self, text: impl Into<String>) -> Self {
        self.cell.blocks.push(plain_paragraph(text.into()));
        self
    }

    /// Append one paragraph made from already-built runs.
    pub fn paragraph_runs<I>(mut self, runs: I) -> Self
    where
        I: IntoIterator<Item = Run>,
    {
        self.cell.blocks.push(paragraph(runs, ParaProps::default()));
        self
    }

    /// Append one rich paragraph to the cell.
    pub fn rich_paragraph<P>(mut self, paragraph: P) -> Self
    where
        P: Into<Paragraph>,
    {
        self.cell.blocks.push(Block::Paragraph(paragraph.into()));
        self
    }

    /// Append one nested table to the cell.
    pub fn rich_table<T>(mut self, table: T) -> Self
    where
        T: Into<Table>,
    {
        self.cell.blocks.push(Block::Table(table.into()));
        self
    }

    /// Append an already-constructed block to the cell.
    pub fn push_block(mut self, block: Block) -> Self {
        self.cell.blocks.push(block);
        self
    }

    /// Mark the cell as a header cell.
    pub fn header(mut self) -> Self {
        self.cell.is_header = true;
        self
    }

    /// Set the number of columns spanned by this cell.
    pub fn col_span(mut self, span: u16) -> Self {
        self.cell.col_span = span.max(1);
        self
    }

    /// Set the number of rows spanned by this cell.
    pub fn row_span(mut self, span: u16) -> Self {
        self.cell.row_span = span.max(1);
        self
    }

    /// Set the cell background color.
    pub fn shading(mut self, color: Color) -> Self {
        self.cell.shading = Some(color);
        self
    }

    /// Set vertical alignment for the cell content.
    pub fn valign(mut self, valign: VCell) -> Self {
        self.cell.valign = valign;
        self
    }

    /// Set the cell width as a fraction of table width.
    pub fn width_pct(mut self, width_pct: f32) -> Self {
        self.cell.width_pct = Some(width_pct);
        self
    }

    /// Set explicit per-cell margins in twips (top, right, bottom, left).
    pub fn margins_twips(mut self, top: u32, right: u32, bottom: u32, left: u32) -> Self {
        self.cell.margins = Some(crate::model::CellMargins {
            top,
            right,
            bottom,
            left,
        });
        self
    }

    /// Finish and return the cell.
    pub fn build(self) -> Cell {
        self.cell
    }
}

impl From<CellBuilder> for Cell {
    fn from(builder: CellBuilder) -> Self {
        builder.build()
    }
}

/// Thin builder for an embedded [`Image`] block.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ImageBuilder {
    image: Image,
}

impl ImageBuilder {
    /// Start an embedded image with caller-supplied bytes and MIME type.
    pub fn new(bytes: Vec<u8>, mime: impl Into<String>) -> Self {
        Self {
            image: Image {
                bytes: Some(bytes),
                mime: Some(mime.into()),
                ..Image::default()
            },
        }
    }

    /// Continue building from an existing image.
    pub fn from_image(image: Image) -> Self {
        Self { image }
    }

    /// Set the image alt text.
    pub fn alt(mut self, alt: impl Into<String>) -> Self {
        let alt = alt.into().trim().to_string();
        self.image.alt = (!alt.is_empty()).then_some(alt);
        self
    }

    /// Set the image dimensions in pixels.
    pub fn size_px(mut self, width_px: u32, height_px: u32) -> Self {
        self.image.width_px = Some(width_px);
        self.image.height_px = Some(height_px);
        self
    }

    /// Set the image width in pixels.
    pub fn width_px(mut self, width_px: u32) -> Self {
        self.image.width_px = Some(width_px);
        self
    }

    /// Set the image height in pixels.
    pub fn height_px(mut self, height_px: u32) -> Self {
        self.image.height_px = Some(height_px);
        self
    }

    /// Rotate the image clockwise by whole degrees.
    pub fn rotate_degrees(mut self, degrees: i32) -> Self {
        self.image.rotation_degrees = Some(degrees.rem_euclid(360));
        self
    }

    /// Emit this image as a page-relative floating anchor at the given EMU offset.
    pub fn floating_offset_emu(mut self, x_emu: i64, y_emu: i64) -> Self {
        self.image.floating_offset_emu = Some((x_emu, y_emu));
        self
    }

    /// Finish and return the image.
    pub fn build(self) -> Image {
        self.image
    }
}

impl From<ImageBuilder> for Image {
    fn from(builder: ImageBuilder) -> Self {
        builder.build()
    }
}

/// Thin builder for a block-level [`Chart`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChartBuilder {
    chart: Chart,
}

impl ChartBuilder {
    /// Start a clustered horizontal bar chart.
    pub fn bar() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Bar,
                ..Chart::default()
            },
        }
    }

    /// Start a stacked horizontal bar chart.
    pub fn stacked_bar() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::StackedBar,
                ..Chart::default()
            },
        }
    }

    /// Start a 100% stacked horizontal bar chart.
    pub fn percent_stacked_bar() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::PercentStackedBar,
                ..Chart::default()
            },
        }
    }

    /// Start a clustered 3-D horizontal bar chart.
    pub fn bar_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Bar3D,
                ..Chart::default()
            },
        }
    }

    /// Start a stacked 3-D horizontal bar chart.
    pub fn stacked_bar_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::StackedBar3D,
                ..Chart::default()
            },
        }
    }

    /// Start a 100% stacked 3-D horizontal bar chart.
    pub fn percent_stacked_bar_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::PercentStackedBar3D,
                ..Chart::default()
            },
        }
    }

    /// Start a clustered vertical column chart.
    pub fn column() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Column,
                ..Chart::default()
            },
        }
    }

    /// Start a stacked vertical column chart.
    pub fn stacked_column() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::StackedColumn,
                ..Chart::default()
            },
        }
    }

    /// Start a 100% stacked vertical column chart.
    pub fn percent_stacked_column() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::PercentStackedColumn,
                ..Chart::default()
            },
        }
    }

    /// Start a clustered 3-D vertical column chart.
    pub fn column_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Column3D,
                ..Chart::default()
            },
        }
    }

    /// Start a stacked 3-D vertical column chart.
    pub fn stacked_column_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::StackedColumn3D,
                ..Chart::default()
            },
        }
    }

    /// Start a 100% stacked 3-D vertical column chart.
    pub fn percent_stacked_column_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::PercentStackedColumn3D,
                ..Chart::default()
            },
        }
    }

    /// Start a line chart.
    pub fn line() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Line,
                ..Chart::default()
            },
        }
    }

    /// Start a line chart without point markers.
    pub fn line_no_markers() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::LineNoMarkers,
                ..Chart::default()
            },
        }
    }

    /// Start a smoothed line chart.
    pub fn smooth_line() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::SmoothLine,
                ..Chart::default()
            },
        }
    }

    /// Start a stacked line chart.
    pub fn stacked_line() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::StackedLine,
                ..Chart::default()
            },
        }
    }

    /// Start a 100% stacked line chart.
    pub fn percent_stacked_line() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::PercentStackedLine,
                ..Chart::default()
            },
        }
    }

    /// Start a 3-D line chart.
    pub fn line_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Line3D,
                ..Chart::default()
            },
        }
    }

    /// Start an area chart.
    pub fn area() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Area,
                ..Chart::default()
            },
        }
    }

    /// Start a stacked area chart.
    pub fn stacked_area() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::StackedArea,
                ..Chart::default()
            },
        }
    }

    /// Start a 100% stacked area chart.
    pub fn percent_stacked_area() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::PercentStackedArea,
                ..Chart::default()
            },
        }
    }

    /// Start a 3-D area chart.
    pub fn area_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Area3D,
                ..Chart::default()
            },
        }
    }

    /// Start a stacked 3-D area chart.
    pub fn stacked_area_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::StackedArea3D,
                ..Chart::default()
            },
        }
    }

    /// Start a 100% stacked 3-D area chart.
    pub fn percent_stacked_area_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::PercentStackedArea3D,
                ..Chart::default()
            },
        }
    }

    /// Start a radar chart.
    pub fn radar() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Radar,
                ..Chart::default()
            },
        }
    }

    /// Start a radar chart with explicit point markers.
    pub fn radar_with_markers() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::RadarWithMarkers,
                ..Chart::default()
            },
        }
    }

    /// Start a filled radar chart.
    pub fn filled_radar() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::FilledRadar,
                ..Chart::default()
            },
        }
    }

    /// Start a scatter chart.
    pub fn scatter() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Scatter,
                ..Chart::default()
            },
        }
    }

    /// Start a marker-only scatter chart.
    pub fn scatter_markers() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::ScatterMarkers,
                ..Chart::default()
            },
        }
    }

    /// Start a straight-line scatter chart without point markers.
    pub fn scatter_lines() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::ScatterLines,
                ..Chart::default()
            },
        }
    }

    /// Start a smoothed scatter chart with point markers.
    pub fn scatter_smooth() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::ScatterSmooth,
                ..Chart::default()
            },
        }
    }

    /// Start a smoothed scatter chart without point markers.
    pub fn scatter_smooth_no_markers() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::ScatterSmoothNoMarkers,
                ..Chart::default()
            },
        }
    }

    /// Start a bubble chart.
    pub fn bubble() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Bubble,
                ..Chart::default()
            },
        }
    }

    /// Start a 3-D bubble chart.
    pub fn bubble_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Bubble3D,
                ..Chart::default()
            },
        }
    }

    /// Start a pie chart using the first series as slice values.
    pub fn pie() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Pie,
                ..Chart::default()
            },
        }
    }

    /// Start an exploded pie chart using the first series as slice values.
    pub fn exploded_pie() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::ExplodedPie,
                ..Chart::default()
            },
        }
    }

    /// Start a 3-D pie chart using the first series as slice values.
    pub fn pie_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Pie3D,
                ..Chart::default()
            },
        }
    }

    /// Start an exploded 3-D pie chart using the first series as slice values.
    pub fn exploded_pie_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::ExplodedPie3D,
                ..Chart::default()
            },
        }
    }

    /// Start a pie-of-pie chart using the first series as slice values.
    pub fn pie_of_pie() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::PieOfPie,
                ..Chart::default()
            },
        }
    }

    /// Start a bar-of-pie chart using the first series as slice values.
    pub fn bar_of_pie() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::BarOfPie,
                ..Chart::default()
            },
        }
    }

    /// Start a doughnut chart using the first series as slice values.
    pub fn doughnut() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Doughnut,
                ..Chart::default()
            },
        }
    }

    /// Start an exploded doughnut chart using the first series as slice values.
    pub fn exploded_doughnut() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::ExplodedDoughnut,
                ..Chart::default()
            },
        }
    }

    /// Start a surface chart using category columns and series rows as a value grid.
    pub fn surface() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Surface,
                ..Chart::default()
            },
        }
    }

    /// Start a 3-D surface chart using category columns and series rows as a value grid.
    pub fn surface_3d() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Surface3D,
                ..Chart::default()
            },
        }
    }

    /// Start a high-low-close stock chart using date/category labels.
    pub fn stock_high_low_close() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::StockHighLowClose,
                ..Chart::default()
            },
        }
    }

    /// Start a stock chart using date/category labels and open/high/low/close-style series.
    pub fn stock() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Stock,
                ..Chart::default()
            },
        }
    }

    /// Start a waterfall chart using the first series as category deltas.
    pub fn waterfall() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Waterfall,
                ..Chart::default()
            },
        }
    }

    /// Start a treemap chart using the first series as rectangle sizes.
    pub fn treemap() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Treemap,
                ..Chart::default()
            },
        }
    }

    /// Start a sunburst chart using the first series as radial slice sizes.
    pub fn sunburst() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Sunburst,
                ..Chart::default()
            },
        }
    }

    /// Start a histogram chart using category bins and the first series as bin counts.
    pub fn histogram() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Histogram,
                ..Chart::default()
            },
        }
    }

    /// Start a box-and-whisker chart using the first series as distribution values.
    pub fn box_whisker() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::BoxWhisker,
                ..Chart::default()
            },
        }
    }

    /// Start a funnel chart using the first series as staged values.
    pub fn funnel() -> Self {
        Self {
            chart: Chart {
                kind: ChartKind::Funnel,
                ..Chart::default()
            },
        }
    }

    /// Continue building from an existing chart.
    pub fn from_chart(chart: Chart) -> Self {
        Self { chart }
    }

    /// Set the chart title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        let title = title.into().trim().to_string();
        self.chart.title = (!title.is_empty()).then_some(title);
        self
    }

    /// Set the category labels shared by all series.
    pub fn categories<I, S>(mut self, categories: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.chart.categories = categories.into_iter().map(Into::into).collect();
        self
    }

    /// Append a named numeric series.
    pub fn series<I, V>(mut self, name: impl Into<String>, values: I) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<f64>,
    {
        self.chart.series.push(ChartSeries {
            name: name.into(),
            values: values.into_iter().map(Into::into).collect(),
            bubble_sizes: Vec::new(),
        });
        self
    }

    /// Append a named bubble series with Y values and bubble sizes.
    pub fn bubble_series<I, V, J, S>(
        mut self,
        name: impl Into<String>,
        values: I,
        bubble_sizes: J,
    ) -> Self
    where
        I: IntoIterator<Item = V>,
        V: Into<f64>,
        J: IntoIterator<Item = S>,
        S: Into<f64>,
    {
        self.chart.series.push(ChartSeries {
            name: name.into(),
            values: values.into_iter().map(Into::into).collect(),
            bubble_sizes: bubble_sizes.into_iter().map(Into::into).collect(),
        });
        self
    }

    /// Set the chart drawing dimensions in pixels.
    pub fn size_px(mut self, width_px: u32, height_px: u32) -> Self {
        self.chart.width_px = Some(width_px);
        self.chart.height_px = Some(height_px);
        self
    }

    /// Set the chart drawing width in pixels.
    pub fn width_px(mut self, width_px: u32) -> Self {
        self.chart.width_px = Some(width_px);
        self
    }

    /// Set the chart drawing height in pixels.
    pub fn height_px(mut self, height_px: u32) -> Self {
        self.chart.height_px = Some(height_px);
        self
    }

    /// Set alternate text for the chart drawing.
    pub fn alt(mut self, alt: impl Into<String>) -> Self {
        let alt = alt.into().trim().to_string();
        self.chart.alt = (!alt.is_empty()).then_some(alt);
        self
    }

    /// Render surface-family charts as wireframes instead of filled surfaces.
    pub fn wireframe(mut self) -> Self {
        self.chart.wireframe = true;
        self
    }

    /// Set the shape style for 3-D bar and 3-D column charts.
    pub fn shape(mut self, shape: ChartShape) -> Self {
        self.chart.shape = shape;
        self
    }

    /// Finish and return the chart.
    pub fn build(self) -> Chart {
        self.chart
    }
}

impl From<ChartBuilder> for Chart {
    fn from(builder: ChartBuilder) -> Self {
        builder.build()
    }
}

/// Thin builder for rich [`Table`] values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableBuilder {
    table: Table,
}

impl TableBuilder {
    /// Start an empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Continue building from an existing table.
    pub fn from_table(table: Table) -> Self {
        Self { table }
    }

    /// Set the number of leading rows that repeat as table headers.
    pub fn header_rows(mut self, header_rows: usize) -> Self {
        self.table.header_rows = header_rows;
        self
    }

    /// Set column widths as fractions of the table width.
    pub fn col_widths_pct<I>(mut self, widths: I) -> Self
    where
        I: IntoIterator<Item = f32>,
    {
        self.table.col_widths_pct = widths.into_iter().collect();
        self
    }

    /// Set the table width as a fraction of the available width.
    pub fn width_pct(mut self, width_pct: f32) -> Self {
        self.table.width_pct = Some(width_pct);
        self
    }

    /// Use Word's fixed table layout algorithm.
    pub fn fixed_layout(mut self) -> Self {
        self.table.fixed_layout = true;
        self
    }

    /// Use visual right-to-left table ordering.
    pub fn bidi_visual(mut self) -> Self {
        self.table.bidi_visual = true;
        self
    }

    /// Set table indentation in twips.
    pub fn indent_twips(mut self, indent_twips: i32) -> Self {
        self.table.indent_twips = Some(indent_twips);
        self
    }

    /// Set table alignment.
    pub fn align(mut self, align: Align) -> Self {
        self.table.align = Some(align);
        self
    }

    /// Set a uniform table border color.
    pub fn border_color(mut self, color: Color) -> Self {
        self.table.border_color = Some(color);
        self
    }

    /// Set a table border color for one physical side.
    pub fn border_side_color(mut self, side: TableBorderSide, color: Color) -> Self {
        self.table.border_colors.set(side, color);
        self
    }

    /// Set the uniform table border width in eighths of a point.
    pub fn border_size_eighths(mut self, size: u16) -> Self {
        self.table.border_size_eighths = Some(size.max(1));
        self
    }

    /// Set a table border width for one physical side, in eighths of a point.
    pub fn border_side_size_eighths(mut self, side: TableBorderSide, size: u16) -> Self {
        self.table.border_sizes.set(side, size);
        self
    }

    /// Set the uniform table border style.
    pub fn border_style(mut self, style: TableBorderStyle) -> Self {
        self.table.border_style = Some(style);
        self
    }

    /// Set a table border style for one physical side.
    pub fn border_side_style(mut self, side: TableBorderSide, style: TableBorderStyle) -> Self {
        self.table.border_styles.set(side, style);
        self
    }

    /// Append a row of cells.
    pub fn row<I, C>(mut self, cells: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<Cell>,
    {
        self.table.rows.push(Row {
            cells: cells.into_iter().map(Into::into).collect(),
        });
        self
    }

    /// Append an already-constructed row.
    pub fn push_row(mut self, row: Row) -> Self {
        self.table.rows.push(row);
        self
    }

    /// Finish and return the table.
    pub fn build(mut self) -> Table {
        for row in self.table.rows.iter_mut().take(self.table.header_rows) {
            for cell in &mut row.cells {
                cell.is_header = true;
            }
        }
        self.table
    }
}

impl From<TableBuilder> for Table {
    fn from(builder: TableBuilder) -> Self {
        builder.build()
    }
}

/// Thin builder for creating a [`DocModel`] without filling every struct field by
/// hand.
///
/// This is an authoring convenience only: it creates a fresh semantic model for
/// [`crate::write_docx`] / [`crate::try_write_docx`]. It does not preserve an
/// existing `.docx` package; use [`crate::Document`] preservation edit APIs for
/// that.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocBuilder {
    model: DocModel,
}

impl DocBuilder {
    /// Start a new empty document model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Continue building from an existing model.
    pub fn from_model(model: DocModel) -> Self {
        Self { model }
    }

    /// Set document title metadata.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        let title = title.into().trim().to_string();
        self.model.setup.title = (!title.is_empty()).then_some(title);
        self
    }

    /// Set document subject metadata.
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        let subject = subject.into().trim().to_string();
        self.model.setup.subject = (!subject.is_empty()).then_some(subject);
        self
    }

    /// Set document creator metadata.
    pub fn creator(mut self, creator: impl Into<String>) -> Self {
        let creator = creator.into().trim().to_string();
        self.model.setup.creator = (!creator.is_empty()).then_some(creator);
        self
    }

    /// Set document description metadata.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        let description = description.into().trim().to_string();
        self.model.setup.description = (!description.is_empty()).then_some(description);
        self
    }

    /// Set document keywords metadata.
    pub fn keywords(mut self, keywords: impl Into<String>) -> Self {
        let keywords = keywords.into().trim().to_string();
        self.model.setup.keywords = (!keywords.is_empty()).then_some(keywords);
        self
    }

    /// Set document category metadata.
    pub fn category(mut self, category: impl Into<String>) -> Self {
        let category = category.into().trim().to_string();
        self.model.setup.category = (!category.is_empty()).then_some(category);
        self
    }

    /// Set document content-status metadata.
    pub fn content_status(mut self, content_status: impl Into<String>) -> Self {
        let content_status = content_status.into().trim().to_string();
        self.model.setup.content_status = (!content_status.is_empty()).then_some(content_status);
        self
    }

    /// Set document last-modified-by metadata.
    pub fn last_modified_by(mut self, last_modified_by: impl Into<String>) -> Self {
        let last_modified_by = last_modified_by.into().trim().to_string();
        self.model.setup.last_modified_by =
            (!last_modified_by.is_empty()).then_some(last_modified_by);
        self
    }

    /// Set document creation timestamp metadata.
    pub fn created(mut self, created: impl Into<String>) -> Self {
        let created = created.into().trim().to_string();
        self.model.setup.created = (!created.is_empty()).then_some(created);
        self
    }

    /// Set document last-modified timestamp metadata.
    pub fn modified(mut self, modified: impl Into<String>) -> Self {
        let modified = modified.into().trim().to_string();
        self.model.setup.modified = (!modified.is_empty()).then_some(modified);
        self
    }

    /// Set document last-printed timestamp metadata.
    pub fn last_printed(mut self, last_printed: impl Into<String>) -> Self {
        let last_printed = last_printed.into().trim().to_string();
        self.model.setup.last_printed = (!last_printed.is_empty()).then_some(last_printed);
        self
    }

    /// Set document revision-count metadata.
    pub fn revision(mut self, revision: impl Into<String>) -> Self {
        let revision = revision.into().trim().to_string();
        self.model.setup.revision = (!revision.is_empty()).then_some(revision);
        self
    }

    /// Set document version metadata.
    pub fn version(mut self, version: impl Into<String>) -> Self {
        let version = version.into().trim().to_string();
        self.model.setup.version = (!version.is_empty()).then_some(version);
        self
    }

    /// Set a string custom document property.
    pub fn custom_property(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        let name = name.into().trim().to_string();
        if !name.is_empty() {
            self.model.custom_properties.insert(name, value.into());
        }
        self
    }

    /// Add a raw custom XML data-store item.
    pub fn custom_xml_item(
        mut self,
        store_item_id: impl Into<String>,
        xml: impl Into<String>,
    ) -> Self {
        let store_item_id = store_item_id.into().trim().to_string();
        if !store_item_id.is_empty() {
            self.model.custom_xml_items.push(CustomXmlItem {
                store_item_id,
                xml: xml.into(),
            });
        }
        self
    }

    /// Set the Word 2010 document id emitted in `word/settings.xml`.
    pub fn document_id(mut self, id: impl Into<String>) -> Self {
        let id = id.into().trim().to_string();
        self.model.setup.document_id = (!id.is_empty()).then_some(id);
        self
    }

    /// Add an auto-show Office web-extension task pane.
    pub fn web_extension_task_pane(
        mut self,
        extension_id: impl Into<String>,
        reference_id: impl Into<String>,
        version: impl Into<String>,
        store: impl Into<String>,
        store_type: impl Into<String>,
    ) -> Self {
        let extension_id = extension_id.into().trim().to_string();
        let reference_id = reference_id.into().trim().to_string();
        let version = version.into().trim().to_string();
        let store = store.into().trim().to_string();
        let store_type = store_type.into().trim().to_string();
        if extension_id.is_empty()
            || reference_id.is_empty()
            || version.is_empty()
            || store.is_empty()
            || store_type.is_empty()
        {
            return self;
        }
        let mut properties = std::collections::BTreeMap::new();
        properties.insert(
            "Office.AutoShowTaskpaneWithDocument".to_string(),
            "true".to_string(),
        );
        self.model
            .setup
            .web_extension_task_panes
            .push(WebExtensionTaskPane {
                extension_id,
                reference_id,
                version,
                store,
                store_type,
                properties,
                dock_state: "right".to_string(),
                visible: true,
                width: 350,
                row: 0,
                locked: false,
            });
        self
    }

    /// Set page geometry and margins.
    pub fn page_setup(mut self, page: PageSetup) -> Self {
        self.model.setup.page = page;
        self
    }

    /// Set page size in points.
    pub fn page_size_pt(mut self, width_pt: f32, height_pt: f32) -> Self {
        self.model.setup.page.width_pt = width_pt;
        self.model.setup.page.height_pt = height_pt;
        self
    }

    /// Mark the page orientation as landscape. Width/height are left as supplied.
    pub fn landscape(mut self) -> Self {
        self.model.setup.page.landscape = true;
        self
    }

    /// Set a uniform page margin in points.
    pub fn margins_pt(mut self, margin_pt: f32) -> Self {
        self.model.setup.page.margin_pt = margin_pt;
        self.model.setup.page.margin_left_pt = None;
        self.model.setup.page.margin_right_pt = None;
        self.model.setup.page.margin_top_pt = None;
        self.model.setup.page.margin_bottom_pt = None;
        self
    }

    /// Set per-side page margins in points, in top/right/bottom/left order.
    pub fn margins_each_pt(
        mut self,
        top_pt: f32,
        right_pt: f32,
        bottom_pt: f32,
        left_pt: f32,
    ) -> Self {
        self.model.setup.page.margin_top_pt = Some(top_pt);
        self.model.setup.page.margin_right_pt = Some(right_pt);
        self.model.setup.page.margin_bottom_pt = Some(bottom_pt);
        self.model.setup.page.margin_left_pt = Some(left_pt);
        self
    }

    /// Emit page numbers in the generated footer.
    pub fn page_numbers(mut self) -> Self {
        self.model.setup.page_numbers = true;
        self
    }

    /// Disable generated page numbers for the current/final section.
    pub fn no_page_numbers(mut self) -> Self {
        self.model.setup.page_numbers = false;
        self
    }

    /// Restart displayed page numbering for the current/final section.
    pub fn page_number_start(mut self, start: u32) -> Self {
        self.model.setup.page_number_start = Some(start.max(1));
        self
    }

    /// Set the displayed page-number format for the current/final section.
    pub fn page_number_format(mut self, format: PageNumberFormat) -> Self {
        self.model.setup.page_number_format = Some(format);
        self
    }

    /// Set the number of text columns for the current/final section.
    pub fn columns(mut self, columns: u16) -> Self {
        self.model.setup.columns = Some(columns.max(1));
        self
    }

    /// Set the text flow direction for the current/final section.
    pub fn text_direction(mut self, direction: TextDirection) -> Self {
        self.model.setup.text_direction = Some(direction);
        self
    }

    /// Set a line-only document grid for the current/final section.
    pub fn doc_grid_lines(mut self, line_pitch: u32) -> Self {
        self.model.setup.doc_grid = Some(DocGrid {
            grid_type: DocGridType::Lines,
            line_pitch: Some(line_pitch),
            character_space: None,
        });
        self
    }

    /// Set a line-and-character document grid for the current/final section.
    pub fn doc_grid_lines_and_chars(mut self, line_pitch: u32, character_space: u32) -> Self {
        self.model.setup.doc_grid = Some(DocGrid {
            grid_type: DocGridType::LinesAndChars,
            line_pitch: Some(line_pitch),
            character_space: Some(character_space),
        });
        self
    }

    /// Set a character-only document grid for the current/final section.
    pub fn doc_grid_snap_to_chars(mut self, character_space: u32) -> Self {
        self.model.setup.doc_grid = Some(DocGrid {
            grid_type: DocGridType::SnapToChars,
            line_pitch: None,
            character_space: Some(character_space),
        });
        self
    }

    /// Enable distinct first-page section behavior.
    pub fn title_page(mut self) -> Self {
        self.model.setup.title_page = true;
        self
    }

    /// Add a paragraph style definition.
    pub fn paragraph_style<S>(mut self, style: S) -> Self
    where
        S: Into<ParagraphStyle>,
    {
        self.model.setup.styles.push(style.into());
        self
    }

    /// Add a plain paragraph.
    pub fn paragraph(mut self, text: impl Into<String>) -> Self {
        self.model.blocks.push(plain_paragraph(text.into()));
        self
    }

    /// Add a paragraph from already-built runs.
    pub fn paragraph_runs<I>(mut self, runs: I) -> Self
    where
        I: IntoIterator<Item = Run>,
    {
        self.model
            .blocks
            .push(paragraph(runs, ParaProps::default()));
        self
    }

    /// Add an already-built rich paragraph.
    pub fn rich_paragraph<P>(mut self, paragraph: P) -> Self
    where
        P: Into<Paragraph>,
    {
        self.model.blocks.push(Block::Paragraph(paragraph.into()));
        self
    }

    /// Add a heading paragraph. Levels outside `1..=6` are clamped to Word's
    /// built-in heading range.
    pub fn heading(mut self, level: u8, text: impl Into<String>) -> Self {
        self.model.blocks.push(heading(level, text.into()));
        self
    }

    /// Add a heading paragraph from already-built runs.
    pub fn heading_runs<I>(mut self, level: u8, runs: I) -> Self
    where
        I: IntoIterator<Item = Run>,
    {
        self.model.blocks.push(paragraph(
            runs,
            ParaProps {
                heading_level: Some(level.clamp(1, 6)),
                ..ParaProps::default()
            },
        ));
        self
    }

    /// Add one or more ordered list paragraphs at level 0.
    pub fn numbered_list<I, S>(self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.list_level(items, true, 0)
    }

    /// Add one or more ordered list paragraphs at a specific level.
    pub fn numbered_list_level<I, S>(self, level: u8, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.list_level(items, true, level)
    }

    /// Add one or more unordered list paragraphs at level 0.
    pub fn bullet_list<I, S>(self, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.list_level(items, false, 0)
    }

    /// Add one or more unordered list paragraphs at a specific level.
    pub fn bullet_list_level<I, S>(self, level: u8, items: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.list_level(items, false, level)
    }

    /// Add one or more list paragraphs at level 0.
    pub fn list<I, S>(self, items: I, ordered: bool) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.list_level(items, ordered, 0)
    }

    /// Add one or more list paragraphs at a specific level. Levels above 8 are
    /// clamped to the range declared by the generated `numbering.xml`.
    pub fn list_level<I, S>(mut self, items: I, ordered: bool, level: u8) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let level = level.min(8);
        self.model.blocks.extend(
            items
                .into_iter()
                .map(|text| list_paragraph(text.into(), ordered, level)),
        );
        self
    }

    /// Add a paragraph containing one hyperlink run. A target beginning with
    /// `#` names an internal document bookmark.
    pub fn hyperlink(mut self, text: impl Into<String>, url: impl Into<String>) -> Self {
        self.model
            .blocks
            .push(hyperlink_paragraph(text.into(), url.into()));
        self
    }

    /// Add a paragraph containing one simple field result.
    pub fn field(mut self, instruction: impl Into<String>, result: impl Into<String>) -> Self {
        let instruction = normalize_field_instruction(&instruction.into());
        let field = if instruction.is_empty() {
            FieldRole::None
        } else {
            FieldRole::Simple { instruction }
        };
        self.model.blocks.push(Block::Paragraph(Paragraph {
            props: ParaProps::default(),
            runs: vec![Run {
                text: result.into(),
                field,
                ..Run::default()
            }],
        }));
        self
    }

    /// Add a dirty table-of-contents field for heading levels in `start..=end`.
    pub fn toc_heading_range(mut self, start: u8, end: u8) -> Self {
        let start = start.clamp(1, 9);
        let end = end.clamp(1, 9);
        let (start, end) = if start <= end {
            (start, end)
        } else {
            (end, start)
        };
        self.model.blocks.push(Block::Paragraph(Paragraph {
            props: ParaProps::default(),
            runs: vec![Run {
                text: "Contents".to_string(),
                field: FieldRole::Simple {
                    instruction: format!(r#"TOC \o "{start}-{end}""#),
                },
                field_dirty: true,
                ..Run::default()
            }],
        }));
        self
    }

    /// Add a block image with caller-supplied bytes and MIME type.
    pub fn image(mut self, bytes: Vec<u8>, mime: impl Into<String>) -> Self {
        self.model.blocks.push(Block::Image(Image {
            bytes: Some(bytes),
            mime: Some(mime.into()),
            ..Image::default()
        }));
        self
    }

    /// Add an already-built rich image block.
    pub fn rich_image<I>(mut self, image: I) -> Self
    where
        I: Into<Image>,
    {
        self.model.blocks.push(Block::Image(image.into()));
        self
    }

    /// Add an already-built chart block.
    pub fn chart<C>(mut self, chart: C) -> Self
    where
        C: Into<Chart>,
    {
        self.model.blocks.push(Block::Chart(chart.into()));
        self
    }

    /// Add an explicit page break.
    pub fn page_break(mut self) -> Self {
        self.model.blocks.push(Block::PageBreak);
        self
    }

    /// Close the current section and start a new one on the next page.
    ///
    /// The section break snapshots the current page setup, headers, footers, and
    /// page-number setting. Later builder calls mutate the final section setup.
    pub fn section_break(self) -> Self {
        self.section_break_kind(SectionBreakKind::NextPage)
    }

    /// Close the current section and start a new one on the next even page.
    pub fn section_break_even_page(self) -> Self {
        self.section_break_kind(SectionBreakKind::EvenPage)
    }

    /// Close the current section and start a new one on the next odd page.
    pub fn section_break_odd_page(self) -> Self {
        self.section_break_kind(SectionBreakKind::OddPage)
    }

    fn section_break_kind(mut self, kind: SectionBreakKind) -> Self {
        let mut setup = SectionSetup::from(&self.model.setup);
        setup.section_break = Some(kind);
        self.model.blocks.push(Block::SectionBreak(setup));
        self
    }

    /// Add a simple text table with no header rows.
    pub fn table<R, C, S>(self, rows: R) -> Self
    where
        R: IntoIterator<Item = C>,
        C: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.table_with_header_rows(rows, 0)
    }

    /// Add a simple text table whose first row is styled as a header.
    pub fn table_with_header<R, C, S>(self, rows: R) -> Self
    where
        R: IntoIterator<Item = C>,
        C: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.table_with_header_rows(rows, 1)
    }

    /// Add a simple text table with `header_rows` leading rows marked as headers.
    pub fn table_with_header_rows<R, C, S>(mut self, rows: R, header_rows: usize) -> Self
    where
        R: IntoIterator<Item = C>,
        C: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let rows = rows
            .into_iter()
            .enumerate()
            .map(|(row_index, row)| Row {
                cells: row
                    .into_iter()
                    .map(|text| plain_cell(text.into(), row_index < header_rows))
                    .collect(),
            })
            .collect();
        self.model.blocks.push(Block::Table(Table {
            rows,
            header_rows,
            ..Table::default()
        }));
        self
    }

    /// Add an already-built rich table.
    pub fn rich_table<T>(mut self, table: T) -> Self
    where
        T: Into<Table>,
    {
        self.model.blocks.push(Block::Table(table.into()));
        self
    }

    /// Add a plain running header paragraph.
    pub fn header(mut self, text: impl Into<String>) -> Self {
        self.model.setup.header.push(plain_paragraph(text.into()));
        self
    }

    /// Add a first-page running header paragraph.
    pub fn first_header(mut self, text: impl Into<String>) -> Self {
        self.model.setup.title_page = true;
        self.model
            .setup
            .first_header
            .push(plain_paragraph(text.into()));
        self
    }

    /// Add an even-page running header paragraph.
    pub fn even_header(mut self, text: impl Into<String>) -> Self {
        self.model
            .setup
            .even_header
            .push(plain_paragraph(text.into()));
        self
    }

    /// Add a first-page running header paragraph from already-built runs.
    pub fn first_header_runs<I>(mut self, runs: I) -> Self
    where
        I: IntoIterator<Item = Run>,
    {
        self.model.setup.title_page = true;
        self.model
            .setup
            .first_header
            .push(paragraph(runs, ParaProps::default()));
        self
    }

    /// Add an even-page running header paragraph from already-built runs.
    pub fn even_header_runs<I>(mut self, runs: I) -> Self
    where
        I: IntoIterator<Item = Run>,
    {
        self.model
            .setup
            .even_header
            .push(paragraph(runs, ParaProps::default()));
        self
    }

    /// Add a running header paragraph from already-built runs.
    pub fn header_runs<I>(mut self, runs: I) -> Self
    where
        I: IntoIterator<Item = Run>,
    {
        self.model
            .setup
            .header
            .push(paragraph(runs, ParaProps::default()));
        self
    }

    /// Add an already-constructed block to the running header.
    pub fn push_header_block(mut self, block: Block) -> Self {
        self.model.setup.header.push(block);
        self
    }

    /// Add an already-constructed block to the first-page running header.
    pub fn push_first_header_block(mut self, block: Block) -> Self {
        self.model.setup.title_page = true;
        self.model.setup.first_header.push(block);
        self
    }

    /// Add an already-constructed block to the even-page running header.
    pub fn push_even_header_block(mut self, block: Block) -> Self {
        self.model.setup.even_header.push(block);
        self
    }

    /// Remove all running header blocks from the current/final section.
    pub fn clear_header(mut self) -> Self {
        self.model.setup.header.clear();
        self.model.setup.first_header.clear();
        self.model.setup.even_header.clear();
        self
    }

    /// Add a plain running footer paragraph.
    pub fn footer(mut self, text: impl Into<String>) -> Self {
        self.model.setup.footer.push(plain_paragraph(text.into()));
        self
    }

    /// Add a first-page running footer paragraph.
    pub fn first_footer(mut self, text: impl Into<String>) -> Self {
        self.model.setup.title_page = true;
        self.model
            .setup
            .first_footer
            .push(plain_paragraph(text.into()));
        self
    }

    /// Add an even-page running footer paragraph.
    pub fn even_footer(mut self, text: impl Into<String>) -> Self {
        self.model
            .setup
            .even_footer
            .push(plain_paragraph(text.into()));
        self
    }

    /// Add a first-page running footer paragraph from already-built runs.
    pub fn first_footer_runs<I>(mut self, runs: I) -> Self
    where
        I: IntoIterator<Item = Run>,
    {
        self.model.setup.title_page = true;
        self.model
            .setup
            .first_footer
            .push(paragraph(runs, ParaProps::default()));
        self
    }

    /// Add an even-page running footer paragraph from already-built runs.
    pub fn even_footer_runs<I>(mut self, runs: I) -> Self
    where
        I: IntoIterator<Item = Run>,
    {
        self.model
            .setup
            .even_footer
            .push(paragraph(runs, ParaProps::default()));
        self
    }

    /// Add a running footer paragraph from already-built runs.
    pub fn footer_runs<I>(mut self, runs: I) -> Self
    where
        I: IntoIterator<Item = Run>,
    {
        self.model
            .setup
            .footer
            .push(paragraph(runs, ParaProps::default()));
        self
    }

    /// Add an already-constructed block to the running footer.
    pub fn push_footer_block(mut self, block: Block) -> Self {
        self.model.setup.footer.push(block);
        self
    }

    /// Add an already-constructed block to the first-page running footer.
    pub fn push_first_footer_block(mut self, block: Block) -> Self {
        self.model.setup.title_page = true;
        self.model.setup.first_footer.push(block);
        self
    }

    /// Add an already-constructed block to the even-page running footer.
    pub fn push_even_footer_block(mut self, block: Block) -> Self {
        self.model.setup.even_footer.push(block);
        self
    }

    /// Remove all running footer blocks from the current/final section.
    pub fn clear_footer(mut self) -> Self {
        self.model.setup.footer.clear();
        self.model.setup.first_footer.clear();
        self.model.setup.even_footer.clear();
        self
    }

    /// Remove all running header and footer blocks from the current/final section.
    pub fn clear_header_footer(self) -> Self {
        self.clear_header().clear_footer()
    }

    /// Push an already-constructed block.
    pub fn push_block(mut self, block: Block) -> Self {
        self.model.blocks.push(block);
        self
    }

    /// Finish and return the model.
    pub fn build(self) -> DocModel {
        self.model
    }
}

impl From<DocBuilder> for DocModel {
    fn from(builder: DocBuilder) -> Self {
        builder.build()
    }
}

fn plain_paragraph(text: String) -> Block {
    paragraph(
        [plain_run(text, CharProps::default())],
        ParaProps::default(),
    )
}

fn heading(level: u8, text: String) -> Block {
    paragraph(
        [plain_run(text, CharProps::default())],
        ParaProps {
            heading_level: Some(level.clamp(1, 6)),
            ..ParaProps::default()
        },
    )
}

fn paragraph<I>(runs: I, props: ParaProps) -> Block
where
    I: IntoIterator<Item = Run>,
{
    Block::Paragraph(Paragraph {
        props,
        runs: runs.into_iter().collect(),
    })
}

fn list_paragraph(text: String, ordered: bool, level: u8) -> Block {
    Block::Paragraph(Paragraph {
        props: ParaProps {
            list: Some(ListInfo {
                level,
                ordered,
                label: String::new(),
                marker_font: None,
            }),
            ..ParaProps::default()
        },
        runs: vec![plain_run(text, CharProps::default())],
    })
}

fn hyperlink_paragraph(text: String, url: String) -> Block {
    Block::Paragraph(Paragraph {
        props: ParaProps::default(),
        runs: vec![Run {
            text,
            field: hyperlink_field(url),
            ..Run::default()
        }],
    })
}

fn hyperlink_field(url: String) -> FieldRole {
    let url = url.trim().to_string();
    if url.is_empty() {
        FieldRole::None
    } else {
        FieldRole::Hyperlink { url }
    }
}

fn plain_cell(text: String, is_header: bool) -> Cell {
    Cell {
        blocks: vec![plain_paragraph(text)],
        is_header,
        ..Cell::default()
    }
}

fn plain_run(text: String, props: CharProps) -> Run {
    Run {
        text,
        props,
        ..Run::default()
    }
}
