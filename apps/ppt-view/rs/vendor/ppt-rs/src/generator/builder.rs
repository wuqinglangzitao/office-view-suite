//! PPTX builder - orchestrates ZIP creation and file writing

use super::layout_parts::{STANDARD_LAYOUT_COUNT, create_slide_layout_xml};
use super::media_registry::MediaRegistry;
use super::memory_profile::estimate_output_capacity;
use super::notes_xml::*;
use super::package_cache::{self, print_affects_theme_parts};
use super::package_xml::{
    append_digital_signature_content_type, append_embedded_font_content_type,
    content_types_opening, create_content_types_xml_with_notes_and_charts,
    create_handout_master_rels_xml, create_pres_props_xml, create_presentation_rels_xml_full,
    create_presentation_rels_xml_full_with_fonts, create_presentation_xml,
    create_presentation_xml_with_fonts, create_rels_xml_with_signature, create_table_styles_xml,
    create_view_props_xml, table_styles_rel_id,
};
use super::props_xml::{create_app_props_xml, create_core_props_xml};
use super::slide_content::SlideContent;
use super::slide_xml::{create_slide_rels_xml, create_slide_xml, create_slide_xml_with_content};
use super::template::PptxTemplate;
use super::theme_xml::{
    create_layout_rels_xml, create_master_rels_xml, create_slide_master_xml, create_theme_xml,
};
use crate::core::append_usize;
use crate::exc::{PptxError, Result};
use crate::generator::charts::{
    chart_embedding_filename, create_chart_rels_xml, generate_chart_part_xml,
    reference_workbook_bytes,
};
use crate::generator::presentation_theme::office_theme_xml;
use crate::generator::slide_content::presentation_settings::PresentationSettings;
use crate::generator::slide_content::print_settings::PrintWhat;
use std::io::{Cursor, Seek, Write};
use zip::ZipWriter;
use zip::write::FileOptions;

fn zip_options() -> FileOptions {
    FileOptions::default()
}

/// First relationship id after layout (rId1) and optional notes slide.
fn slide_content_rel_start(has_notes: bool, image_count: usize) -> usize {
    2 + usize::from(has_notes) + image_count
}

fn build_media_registry(slides: &[SlideContent]) -> MediaRegistry {
    let mut registry = MediaRegistry::default();
    for slide in slides {
        for image in &slide.images {
            if let Some(bytes) = image.get_bytes() {
                registry.image_number(&bytes, &image.extension());
            }
        }
        register_slide_media(&mut registry, slide);
    }
    registry
}

/// Register video/audio bytes and their poster/placeholder frames so the
/// parts get written and rel targets resolve. Failures surface at plan time.
fn register_slide_media(registry: &mut MediaRegistry, slide: &SlideContent) {
    for video in &slide.videos {
        if let Ok((bytes, ext)) = read_media_source(&video.source) {
            registry.media_number(&bytes, &ext);
        }
        if let Ok((poster, ext)) = media_poster_bytes(video.poster.as_ref()) {
            registry.image_number(&poster, &ext);
        }
    }
    for audio in &slide.audios {
        if let Ok((bytes, ext)) = read_media_source(&audio.source) {
            registry.media_number(&bytes, &ext);
        }
        let (poster, ext) = media_poster_bytes(None).expect("placeholder poster is constant");
        registry.image_number(&poster, &ext);
    }
}

fn build_media_registry_lazy(slides: &dyn LazySlideSource) -> MediaRegistry {
    let mut registry = MediaRegistry::default();
    for i in 0..slides.slide_count() {
        if let Some(slide) = slides.generate_slide(i) {
            for image in &slide.images {
                if let Some(bytes) = image.get_bytes() {
                    registry.image_number(&bytes, &image.extension());
                }
            }
            register_slide_media(&mut registry, &slide);
        }
    }
    registry
}

fn slide_image_rel_targets(slide: &SlideContent, registry: &MediaRegistry) -> Vec<(usize, String)> {
    let mut images = Vec::with_capacity(slide.images.len());
    for image in &slide.images {
        if let Some(bytes) = image.get_bytes()
            && let Some(num) = registry.lookup_number(&bytes, &image.extension())
        {
            images.push((num, image.extension()));
        }
    }
    images
}

/// 1×1 dark-gray PNG used as the poster frame when a media element has no
/// poster image. `p:pic` requires a `p:blipFill`, so audio elements and
/// poster-less videos embed this placeholder.
pub const DEFAULT_MEDIA_POSTER_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0,
    0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 120, 156, 99, 112, 112, 112, 0, 0, 1,
    132, 0, 193, 131, 95, 105, 148, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];
const DEFAULT_MEDIA_POSTER_EXT: &str = "png";

/// Read a media source file and return its bytes plus lowercased extension.
fn read_media_source(source: &str) -> Result<(Vec<u8>, String)> {
    let ext = std::path::Path::new(source)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let bytes = std::fs::read(source)
        .map_err(|e| PptxError::Generic(format!("Failed to read media file \"{source}\": {e}")))?;
    Ok((bytes, ext))
}

/// Poster bytes for a media element: the configured poster image, or the
/// built-in 1×1 placeholder.
fn media_poster_bytes(poster: Option<&String>) -> Result<(Vec<u8>, String)> {
    match poster {
        Some(path) => read_media_source(path),
        None => Ok((
            DEFAULT_MEDIA_POSTER_PNG.to_vec(),
            DEFAULT_MEDIA_POSTER_EXT.to_string(),
        )),
    }
}

/// Per-slide media plan: relationship ids for the slide XML, prebuilt
/// relationship entries for the slide rels, and the poster image-rel
/// entries (appended after the slide's own images).
struct SlideMediaPlan {
    rel_ids: Vec<super::media::MediaRelIds>,
    rels_xml: Vec<String>,
    poster_entries: Vec<(usize, String)>,
}

impl SlideMediaPlan {
    fn empty() -> Self {
        Self {
            rel_ids: Vec::new(),
            rels_xml: Vec::new(),
            poster_entries: Vec::new(),
        }
    }
}

/// Plan the media relationships for one slide.
///
/// * `image_rel_start` — first image-band rId (`2 + has_notes`)
/// * `resolved_image_count` — images with resolvable bytes (rel order)
/// * `media_rid_base` — first free rId after layout/notes/images/charts/ink/comments
fn slide_media_plan(
    slide: &SlideContent,
    registry: &mut MediaRegistry,
    image_rel_start: usize,
    resolved_image_count: usize,
    media_rid_base: usize,
) -> Result<SlideMediaPlan> {
    if slide.videos.is_empty() && slide.audios.is_empty() {
        return Ok(SlideMediaPlan::empty());
    }

    let mut plan = SlideMediaPlan::empty();
    let mut rid = media_rid_base;

    let elements: Vec<(&String, Option<&String>, bool)> = slide
        .videos
        .iter()
        .map(|v| (&v.source, v.poster.as_ref(), true))
        .chain(slide.audios.iter().map(|a| (&a.source, None, false)))
        .collect();

    for (poster_slot, (source, poster, is_video)) in elements.into_iter().enumerate() {
        let (bytes, ext) = read_media_source(source)?;
        if is_video && super::media::VideoFormat::from_extension(&ext).is_none()
            || !is_video && super::media::AudioFormat::from_extension(&ext).is_none()
        {
            return Err(PptxError::Generic(format!(
                "Unsupported media extension \".{ext}\" for \"{source}\""
            )));
        }
        let media_num = registry.media_number(&bytes, &ext);

        let (poster_bytes, poster_ext) = media_poster_bytes(poster)?;
        let poster_num = registry.image_number(&poster_bytes, &poster_ext);
        plan.poster_entries.push((poster_num, poster_ext));

        let link_rid = format!("rId{rid}");
        let embed_rid = format!("rId{}", rid + 1);
        rid += 2;
        let poster_rid = format!(
            "rId{}",
            image_rel_start + resolved_image_count + poster_slot
        );

        let rel_type = if is_video {
            super::package_xml::VIDEO_REL_TYPE
        } else {
            super::package_xml::AUDIO_REL_TYPE
        };
        plan.rels_xml
            .push(super::package_xml::media_relationship_xml(
                &link_rid, rel_type, media_num, &ext,
            ));
        plan.rels_xml
            .push(super::package_xml::media_relationship_xml(
                &embed_rid,
                super::package_xml::MEDIA_REL_TYPE,
                media_num,
                &ext,
            ));

        plan.rel_ids.push(super::media::MediaRelIds {
            link_rid,
            embed_rid,
            poster_rid,
            is_video,
        });
    }
    Ok(plan)
}

/// Media rId base for a slide: first free id after layout/notes/images/
/// charts/ink/comments.
fn slide_media_rid_base(
    start_rid: usize,
    chart_count: usize,
    has_ink: bool,
    has_comments: bool,
) -> usize {
    start_rid + chart_count + usize::from(has_ink) + usize::from(has_comments)
}

/// Collect hyperlink relationship XML (`<Relationship .../>`) for every shape on
/// the slide whose hyperlink has an assigned relationship id. The slide XML
/// references these ids via `<a:hlinkClick r:id="..."/>`, so the matching
/// relationship must be present in the slide's `.rels` part.
fn slide_hyperlink_relationships(slide: &SlideContent) -> Vec<String> {
    slide
        .shapes
        .iter()
        .filter_map(|s| s.hyperlink.as_ref())
        .filter_map(|h| {
            h.r_id
                .as_ref()
                .map(|rid| crate::generator::generate_hyperlink_relationship_xml(h, rid))
        })
        .collect()
}

fn set_notes_part_path(path: &mut String, notes_part_num: usize) {
    path.clear();
    path.push_str("ppt/notesSlides/notesSlide");
    append_usize(path, notes_part_num);
    path.push_str(".xml");
}

#[cfg(debug_assertions)]
fn debug_assert_package_valid(bytes: &[u8]) {
    use crate::core::validate_package_bytes;
    let report = validate_package_bytes(bytes);
    debug_assert!(
        report.is_valid(),
        "generated PPTX failed package validation: {:?}",
        report.error_messages()
    );
}

#[cfg(not(debug_assertions))]
fn debug_assert_package_valid(_bytes: &[u8]) {}

/// Create a minimal but valid PPTX file
pub fn create_pptx(title: &str, slides: usize) -> Result<Vec<u8>> {
    let buffer = Vec::with_capacity(slides.saturating_mul(6_000).max(8_192));
    let cursor = Cursor::new(buffer);
    let mut zip = ZipWriter::new(cursor);
    let options = zip_options();

    write_package_files(&mut zip, &options, title, slides, None, None)?;

    let cursor = zip.finish()?;
    let bytes = cursor.into_inner();
    debug_assert_package_valid(&bytes);
    Ok(bytes)
}

/// Create a PPTX file with custom slide content
pub fn create_pptx_with_content(title: &str, slides: Vec<SlideContent>) -> Result<Vec<u8>> {
    create_pptx_with_settings(title, &slides, None)
}

/// Create a PPTX file with custom slide content, settings, and optional template deck.
pub fn create_pptx_with_template(
    title: &str,
    slides: &[SlideContent],
    template_path: &str,
    settings: Option<PresentationSettings>,
) -> Result<Vec<u8>> {
    let mut merged = settings.unwrap_or_default();
    merged.template_path = Some(template_path.to_string());
    create_pptx_with_settings(title, slides, Some(merged))
}

/// Resolve layout part index for a slide (respects template layout count).
fn resolve_layout_number(slide: &SlideContent, template: Option<&PptxTemplate>) -> usize {
    let requested = slide.layout.layout_number();
    template
        .map(|t| t.resolve_layout_number(requested))
        .unwrap_or(requested)
}

fn load_template(settings: Option<&PresentationSettings>) -> Result<Option<PptxTemplate>> {
    if let Some(path) = settings.and_then(|s| s.template_path.as_deref()) {
        Ok(Some(PptxTemplate::load(path)?))
    } else {
        Ok(None)
    }
}

/// Create a PPTX file with custom slide content and presentation-level settings
pub fn create_pptx_with_settings(
    title: &str,
    slides: &[SlideContent],
    settings: Option<PresentationSettings>,
) -> Result<Vec<u8>> {
    let buffer = Vec::with_capacity(estimate_output_capacity(slides.len(), Some(slides)));
    let cursor = Cursor::new(buffer);
    let mut zip = ZipWriter::new(cursor);
    let options = zip_options();

    write_package_files(
        &mut zip,
        &options,
        title,
        slides.len(),
        Some(slides),
        settings,
    )?;

    let cursor = zip.finish()?;
    let bytes = cursor.into_inner();
    debug_assert_package_valid(&bytes);
    Ok(bytes)
}

/// Create a PPTX file and write it directly to a writer (streaming API).
/// This is more memory-efficient for large presentations as it avoids
/// buffering the entire ZIP file in memory.
///
/// # Example
/// ```rust,no_run
/// use std::fs::File;
/// use ppt_rs::create_pptx_to_writer;
///
/// let file = File::create("output.pptx")?;
/// create_pptx_to_writer(file, "My Presentation", 10)?;
/// # Ok::<(), ppt_rs::PptxError>(())
/// ```
pub fn create_pptx_to_writer<W: Write + Seek>(writer: W, title: &str, slides: usize) -> Result<W> {
    let mut zip = ZipWriter::new(writer);
    let options = FileOptions::default();

    write_package_files(&mut zip, &options, title, slides, None, None)?;

    Ok(zip.finish()?)
}

/// Create a PPTX file with custom content and write it directly to a writer (streaming API).
/// This is more memory-efficient for large presentations.
///
/// # Example
/// ```rust,no_run
/// use std::fs::File;
/// use ppt_rs::{create_pptx_with_content_to_writer, SlideContent};
///
/// let file = File::create("output.pptx")?;
/// let slides = vec![
///     SlideContent::new("Title").add_bullet("Point 1"),
///     SlideContent::new("Slide 2").add_bullet("Point 2"),
/// ];
/// create_pptx_with_content_to_writer(file, "My Presentation", &slides, None)?;
/// # Ok::<(), ppt_rs::PptxError>(())
/// ```
pub fn create_pptx_with_content_to_writer<W: Write + Seek>(
    writer: W,
    title: &str,
    slides: &[SlideContent],
    settings: Option<PresentationSettings>,
) -> Result<W> {
    let mut zip = ZipWriter::new(writer);
    let options = FileOptions::default();

    write_package_files(
        &mut zip,
        &options,
        title,
        slides.len(),
        Some(slides),
        settings,
    )?;

    Ok(zip.finish()?)
}

/// Lazy slide source - allows generating slides on-demand instead of all at once.
/// This is useful for:
/// - Very large presentations that don't fit in memory
/// - Dynamically generated slide content
/// - Streaming data sources
///
/// # Example
/// ```rust,no_run
/// use std::fs::File;
/// use ppt_rs::{create_pptx_lazy_to_writer, LazySlideSource, SlideContent};
///
/// struct MySlideGenerator {
///     count: usize,
/// }
///
/// impl LazySlideSource for MySlideGenerator {
///     fn slide_count(&self) -> usize {
///         self.count
///     }
///
///     fn generate_slide(&self, index: usize) -> Option<SlideContent> {
///         if index < self.count {
///             Some(SlideContent::new(&format!("Slide {}", index + 1))
///                 .add_bullet(&format!("Content {}", index + 1)))
///         } else {
///             None
///         }
///     }
/// }
///
/// let file = File::create("output.pptx")?;
/// create_pptx_lazy_to_writer(file, "My Presentation", Box::new(MySlideGenerator { count: 100 }), None)?;
/// # Ok::<(), ppt_rs::PptxError>(())
/// ```
pub trait LazySlideSource {
    /// Return the total number of slides
    fn slide_count(&self) -> usize;

    /// Generate a slide by index (0-based). Return None if index is out of bounds.
    fn generate_slide(&self, index: usize) -> Option<SlideContent>;

    /// Notes and chart count from a single slide generation when both are needed.
    fn slide_features(&self, index: usize) -> Option<(bool, usize)> {
        self.generate_slide(index)
            .map(|s| (s.notes.is_some(), s.charts.len()))
    }

    /// Check if a slide has notes (default implementation checks the generated slide)
    fn slide_has_notes(&self, index: usize) -> bool {
        self.slide_features(index)
            .map(|(notes, _)| notes)
            .unwrap_or(false)
    }

    /// Get the number of charts in a slide (default implementation checks the generated slide)
    fn slide_chart_count(&self, index: usize) -> usize {
        self.slide_features(index)
            .map(|(_, charts)| charts)
            .unwrap_or(0)
    }
}

/// Create a PPTX file using lazy slide generation and write it directly to a writer.
/// This is the most memory-efficient API for large presentations.
///
/// # Example
/// ```rust,no_run
/// use std::fs::File;
/// use ppt_rs::{create_pptx_lazy_to_writer, LazySlideSource, SlideContent};
///
/// struct MySlideGenerator {
///     count: usize,
/// }
///
/// impl LazySlideSource for MySlideGenerator {
///     fn slide_count(&self) -> usize {
///         self.count
///     }
///
///     fn generate_slide(&self, index: usize) -> Option<SlideContent> {
///         if index < self.count {
///             Some(SlideContent::new(&format!("Slide {}", index + 1))
///                 .add_bullet(&format!("Content {}", index + 1)))
///         } else {
///             None
///         }
///     }
/// }
///
/// let file = File::create("output.pptx")?;
/// let generator = Box::new(MySlideGenerator { count: 1000 });
/// create_pptx_lazy_to_writer(file, "Large Presentation", generator, None)?;
/// # Ok::<(), ppt_rs::PptxError>(())
/// ```
pub fn create_pptx_lazy_to_writer<W: Write + Seek>(
    writer: W,
    title: &str,
    slides: Box<dyn LazySlideSource>,
    settings: Option<PresentationSettings>,
) -> Result<W> {
    let mut zip = ZipWriter::new(writer);
    let options = FileOptions::default();

    write_package_files_lazy(&mut zip, &options, title, slides.as_ref(), settings)?;

    Ok(zip.finish()?)
}

/// Chart metadata for slides
struct ChartInfo {
    total_charts: usize,
    slide_start_indices: Vec<usize>,
}

fn set_slide_xml_path(path: &mut String, slide_num: usize) {
    path.clear();
    path.push_str("ppt/slides/slide");
    append_usize(path, slide_num);
    path.push_str(".xml");
}

fn set_slide_rels_path(path: &mut String, slide_num: usize) {
    path.clear();
    path.push_str("ppt/slides/_rels/slide");
    append_usize(path, slide_num);
    path.push_str(".xml.rels");
}

fn push_chart_rid(rids: &mut Vec<String>, rel_num: usize) {
    let mut rid = String::with_capacity(8);
    rid.push_str("rId");
    append_usize(&mut rid, rel_num);
    rids.push(rid);
}

/// Collect chart metadata from slides (eager version)
fn collect_chart_info(slides: Option<&[SlideContent]>) -> ChartInfo {
    let mut total_charts = 0;
    let mut slide_start_indices = Vec::new();

    if let Some(slides) = slides {
        slide_start_indices.reserve(slides.len());
        for slide in slides {
            slide_start_indices.push(total_charts + 1);
            total_charts += slide.charts.len();
        }
    }

    ChartInfo {
        total_charts,
        slide_start_indices,
    }
}

/// Collect chart metadata from lazy slide source
fn collect_chart_info_lazy(slides: &dyn LazySlideSource) -> ChartInfo {
    let mut total_charts = 0;
    let mut slide_start_indices = Vec::with_capacity(slides.slide_count());

    for i in 0..slides.slide_count() {
        slide_start_indices.push(total_charts + 1);
        total_charts += slides
            .slide_features(i)
            .map(|(_, chart_count)| chart_count)
            .unwrap_or(0);
    }

    ChartInfo {
        total_charts,
        slide_start_indices,
    }
}

/// Whether print settings request handout master packaging.
fn uses_handouts(settings: Option<&PresentationSettings>) -> bool {
    settings
        .and_then(|s| s.print.as_ref())
        .map(|p| p.print_what == PrintWhat::Handouts)
        .unwrap_or(false)
}

/// Whether settings configure a digital signature package.
fn has_digital_signature(settings: Option<&PresentationSettings>) -> bool {
    settings
        .and_then(|s| s.digital_signature.as_ref())
        .is_some()
}

/// Whether settings configure embedded fonts.
fn has_embedded_fonts(settings: Option<&PresentationSettings>) -> bool {
    settings
        .and_then(|s| s.embedded_fonts.as_ref())
        .map(|f| !f.is_empty())
        .unwrap_or(false)
}

/// Borrow embedded fonts from settings, if any.
fn embedded_fonts(
    settings: Option<&PresentationSettings>,
) -> Option<&super::slide_content::embedded_fonts::EmbeddedFontList> {
    settings.and_then(|s| s.embedded_fonts.as_ref())
}

/// Prepare settings by assigning relationship IDs to embedded fonts.
/// Must be called after `has_notes` and `has_handout` are known.
fn prepare_settings(
    settings: &mut Option<PresentationSettings>,
    slide_count: usize,
    has_notes: bool,
    has_handout: bool,
) {
    if let Some(s) = settings
        && let Some(fonts) = s.embedded_fonts.as_mut()
    {
        let first_rid = table_styles_rel_id(slide_count, has_notes, has_handout) + 1;
        fonts.assign_relationship_ids(first_rid);
    }
}

/// Collect slide titles for `docProps/app.xml` (eager version).
///
/// Returns one title per slide, falling back to "Slide N" placeholders when no
/// custom slide content is available or a slide has an empty title.
fn collect_slide_titles(custom_slides: Option<&[SlideContent]>, slide_count: usize) -> Vec<String> {
    match custom_slides {
        Some(slides) => slides
            .iter()
            .enumerate()
            .map(|(i, s)| {
                if s.title.trim().is_empty() {
                    format!("Slide {}", i + 1)
                } else {
                    s.title.clone()
                }
            })
            .collect(),
        None => (0..slide_count)
            .map(|i| format!("Slide {}", i + 1))
            .collect(),
    }
}

/// Collect slide titles for `docProps/app.xml` (lazy version).
fn collect_slide_titles_lazy(slides: &dyn LazySlideSource, slide_count: usize) -> Vec<String> {
    let mut titles = Vec::with_capacity(slide_count);
    for i in 0..slide_count {
        let title = slides
            .generate_slide(i)
            .map(|s| {
                if s.title.trim().is_empty() {
                    format!("Slide {}", i + 1)
                } else {
                    s.title
                }
            })
            .unwrap_or_else(|| format!("Slide {}", i + 1));
        titles.push(title);
    }
    titles
}

/// Write content types XML
fn write_content_types<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    slide_count: usize,
    custom_slides: Option<&[SlideContent]>,
    chart_info: &ChartInfo,
    has_handout: bool,
    settings: Option<&PresentationSettings>,
) -> Result<()> {
    let media_registry = custom_slides.map(build_media_registry);
    let image_exts = media_registry
        .as_ref()
        .map(|registry| registry.extensions())
        .unwrap_or_default();
    let mut content_types = create_content_types_xml_with_notes_and_charts(
        slide_count,
        custom_slides,
        chart_info.total_charts,
        has_handout,
        &image_exts,
    );

    if has_digital_signature(settings) {
        append_digital_signature_content_type(&mut content_types);
    }
    if has_embedded_fonts(settings) {
        append_embedded_font_content_type(&mut content_types);
    }

    let ink_count = custom_slides
        .map(|slides| {
            slides
                .iter()
                .filter(|s| s.ink_annotations.is_some())
                .count()
        })
        .unwrap_or(0);
    super::package_xml::append_ink_content_types(&mut content_types, ink_count);

    let comment_count = custom_slides
        .map(|slides| slides.iter().filter(|s| !s.comments.is_empty()).count())
        .unwrap_or(0);
    let has_comment_authors = has_comment_authors(settings);
    super::package_xml::append_comment_content_types(
        &mut content_types,
        comment_count,
        has_comment_authors,
    );

    let media_exts = media_registry
        .as_ref()
        .map(|registry| registry.media_extensions())
        .unwrap_or_default();
    super::package_xml::append_media_content_type_defaults(&mut content_types, &media_exts);

    zip.start_file("[Content_Types].xml", *options)?;
    zip.write_all(content_types.as_bytes())?;
    Ok(())
}

/// Whether the presentation settings carry any comment authors.
fn has_comment_authors(settings: Option<&PresentationSettings>) -> bool {
    settings.is_some_and(|s| !s.comment_authors.is_empty())
}

/// Write presentation relationships in PowerPoint order.
fn write_presentation_relationships<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    slide_count: usize,
    has_notes: bool,
    has_handout: bool,
    settings: Option<&PresentationSettings>,
) -> Result<()> {
    let has_authors = has_comment_authors(settings);
    let mut pres_rels = if let Some(fonts) = embedded_fonts(settings) {
        create_presentation_rels_xml_full_with_fonts(slide_count, has_notes, has_handout, fonts)
    } else {
        create_presentation_rels_xml_full(slide_count, has_notes, has_handout)
    };
    if has_authors {
        super::package_xml::append_comment_authors_relationship(
            &mut pres_rels,
            super::package_xml::comment_authors_rel_id(slide_count, has_notes, has_handout),
        );
    }

    zip.start_file("ppt/_rels/presentation.xml.rels", *options)?;
    zip.write_all(pres_rels.as_bytes())?;
    Ok(())
}

/// Write presProps, viewProps, and tableStyles (always emitted).
fn write_standard_package_parts<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    settings: Option<&PresentationSettings>,
) -> Result<()> {
    let pres_props = create_pres_props_xml(settings);
    zip.start_file("ppt/presProps.xml", *options)?;
    zip.write_all(pres_props.as_bytes())?;

    let view_props = create_view_props_xml();
    zip.start_file("ppt/viewProps.xml", *options)?;
    zip.write_all(view_props.as_bytes())?;

    let table_styles = create_table_styles_xml();
    zip.start_file("ppt/tableStyles.xml", *options)?;
    zip.write_all(table_styles.as_bytes())?;
    Ok(())
}

/// Write handout master when print settings use handouts.
fn write_handout_master<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    settings: Option<&PresentationSettings>,
) -> Result<()> {
    use crate::generator::slide_content::print_settings::PrintSettings;

    let handout_xml = settings
        .and_then(|s| s.print.as_ref())
        .map(|p| p.to_handout_master_xml())
        .unwrap_or_else(|| PrintSettings::default().to_handout_master_xml());

    zip.start_file("ppt/handoutMasters/handoutMaster1.xml", *options)?;
    zip.write_all(handout_xml.as_bytes())?;

    zip.start_file("ppt/theme/theme3.xml", *options)?;
    zip.write_all(office_theme_xml().as_bytes())?;

    let rels = create_handout_master_rels_xml();
    zip.start_file("ppt/handoutMasters/_rels/handoutMaster1.xml.rels", *options)?;
    zip.write_all(rels.as_bytes())?;
    Ok(())
}

/// Write notes master files if needed
fn write_notes_master<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
) -> Result<()> {
    let notes_master = create_notes_master_xml();
    zip.start_file("ppt/notesMasters/notesMaster1.xml", *options)?;
    zip.write_all(notes_master.as_bytes())?;

    zip.start_file("ppt/theme/theme2.xml", *options)?;
    zip.write_all(office_theme_xml().as_bytes())?;

    let notes_master_rels = create_notes_master_rels_xml();
    zip.start_file("ppt/notesMasters/_rels/notesMaster1.xml.rels", *options)?;
    zip.write_all(notes_master_rels.as_bytes())?;
    Ok(())
}

/// Write theme and layout files
fn write_theme_and_layouts<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    settings: Option<&PresentationSettings>,
    template: Option<&PptxTemplate>,
) -> Result<()> {
    let print = settings.and_then(|s| s.print.as_ref());

    if let Some(tmpl) = template {
        for (path, data) in tmpl.parts() {
            zip.start_file(path, *options)?;
            zip.write_all(data)?;
        }
        if let Some(theme) = settings.and_then(|s| s.theme.as_ref()) {
            let theme_xml = create_theme_xml(Some(theme));
            zip.start_file("ppt/theme/theme1.xml", *options)?;
            zip.write_all(theme_xml.as_bytes())?;
        }
        return Ok(());
    }

    let use_cached_layouts = !print_affects_theme_parts(print);

    for n in 1..=STANDARD_LAYOUT_COUNT {
        zip.start_file(format!("ppt/slideLayouts/slideLayout{n}.xml"), *options)?;
        if use_cached_layouts {
            zip.write_all(package_cache::default_layout_xml(n).as_bytes())?;
        } else {
            let layout_xml = create_slide_layout_xml(n, print);
            zip.write_all(layout_xml.as_bytes())?;
        }

        zip.start_file(
            format!("ppt/slideLayouts/_rels/slideLayout{n}.xml.rels"),
            *options,
        )?;
        zip.write_all(create_layout_rels_xml().as_bytes())?;
    }

    zip.start_file("ppt/slideMasters/slideMaster1.xml", *options)?;
    if use_cached_layouts {
        zip.write_all(package_cache::default_slide_master_xml().as_bytes())?;
    } else {
        let slide_master = create_slide_master_xml(print);
        zip.write_all(slide_master.as_bytes())?;
    }

    zip.start_file("ppt/slideMasters/_rels/slideMaster1.xml.rels", *options)?;
    if use_cached_layouts {
        zip.write_all(package_cache::master_rels_xml().as_bytes())?;
    } else {
        let master_rels = create_master_rels_xml();
        zip.write_all(master_rels.as_bytes())?;
    }

    if let Some(theme) = settings.and_then(|s| s.theme.as_ref()) {
        let theme_xml = create_theme_xml(Some(theme));
        zip.start_file("ppt/theme/theme1.xml", *options)?;
        zip.write_all(theme_xml.as_bytes())?;
    } else {
        zip.start_file("ppt/theme/theme1.xml", *options)?;
        zip.write_all(office_theme_xml().as_bytes())?;
    }

    Ok(())
}

/// Write document properties
fn write_document_properties<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    title: &str,
    slide_count: usize,
    notes_count: usize,
    slide_titles: &[String],
) -> Result<()> {
    // Core properties
    let core_props = create_core_props_xml(title);
    zip.start_file("docProps/core.xml", *options)?;
    zip.write_all(core_props.as_bytes())?;

    // App properties
    let app_props = create_app_props_xml(slide_count, notes_count, slide_titles);
    zip.start_file("docProps/app.xml", *options)?;
    zip.write_all(app_props.as_bytes())?;

    Ok(())
}

/// Write all package files to the ZIP archive (eager version with Vec<SlideContent>)
fn write_package_files<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    title: &str,
    slide_count: usize,
    custom_slides: Option<&[SlideContent]>,
    mut settings: Option<PresentationSettings>,
) -> Result<()> {
    let has_notes = custom_slides
        .map(|slides| slides.iter().any(|s| s.notes.is_some()))
        .unwrap_or(false);
    let has_handout = uses_handouts(settings.as_ref());
    prepare_settings(&mut settings, slide_count, has_notes, has_handout);
    let has_signature = has_digital_signature(settings.as_ref());
    let template = load_template(settings.as_ref())?;

    let chart_info = collect_chart_info(custom_slides);

    // 1. Content types
    write_content_types(
        zip,
        options,
        slide_count,
        custom_slides,
        &chart_info,
        has_handout,
        settings.as_ref(),
    )?;

    // 2. Package relationships
    let rels = create_rels_xml_with_signature(has_signature);
    zip.start_file("_rels/.rels", *options)?;
    zip.write_all(rels.as_bytes())?;

    // 3. Presentation relationships
    write_presentation_relationships(
        zip,
        options,
        slide_count,
        has_notes,
        has_handout,
        settings.as_ref(),
    )?;

    // 4. Presentation document
    let mut presentation = if let Some(fonts) = embedded_fonts(settings.as_ref()) {
        create_presentation_xml_with_fonts(title, slide_count, has_notes, has_handout, fonts)
    } else {
        create_presentation_xml(title, slide_count, has_notes, has_handout)
    };
    if let Some(s) = settings.as_ref() {
        super::package_xml::append_section_lst_ext(&mut presentation, &s.sections, slide_count);
    }
    zip.start_file("ppt/presentation.xml", *options)?;
    zip.write_all(presentation.as_bytes())?;

    // 5. Standard package parts (presProps, viewProps, tableStyles)
    write_standard_package_parts(zip, options, settings.as_ref())?;

    // 6. Handout master (when printing handouts)
    if has_handout {
        write_handout_master(zip, options, settings.as_ref())?;
    }

    // 7. Slides
    write_slides(zip, options, slide_count, custom_slides)?;

    // 8. Slide relationships
    write_slide_relationships_extended(
        zip,
        options,
        custom_slides,
        &chart_info.slide_start_indices,
        slide_count,
        template.as_ref(),
    )?;

    // 9. Notes relationships and master
    if has_notes {
        write_notes_relationships(zip, options, custom_slides)?;
        write_notes_master(zip, options)?;
    }

    // 10. Theme and layouts
    write_theme_and_layouts(zip, options, settings.as_ref(), template.as_ref())?;

    // 11. Document properties
    let notes_count = custom_slides
        .map(|slides| slides.iter().filter(|s| s.notes.is_some()).count())
        .unwrap_or(0);
    let slide_titles = collect_slide_titles(custom_slides, slide_count);
    write_document_properties(zip, options, title, slide_count, notes_count, &slide_titles)?;

    // 12. Charts (with embedded workbooks)
    if chart_info.total_charts > 0 {
        write_charts(zip, options, custom_slides, &chart_info.slide_start_indices)?;
    }

    // 13. Images
    write_images(zip, options, custom_slides)?;

    // 14. Embedded font data parts
    if let Some(fonts) = embedded_fonts(settings.as_ref()) {
        write_embedded_font_parts(zip, options, fonts)?;
    }

    // 15. Digital signature package parts
    if has_signature {
        write_digital_signature_parts(zip, options, settings.as_ref())?;
    }

    // 16. Comment authors part
    if let Some(s) = settings.as_ref().filter(|s| !s.comment_authors.is_empty()) {
        zip.start_file("ppt/commentAuthors.xml", *options)?;
        zip.write_all(s.comment_authors.to_xml().as_bytes())?;
    }

    Ok(())
}

/// Write all package files to the ZIP archive (lazy version with LazySlideSource)
fn write_package_files_lazy<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    title: &str,
    slides: &dyn LazySlideSource,
    mut settings: Option<PresentationSettings>,
) -> Result<()> {
    let slide_count = slides.slide_count();
    let has_notes = (0..slide_count).any(|i| {
        slides
            .slide_features(i)
            .map(|(notes, _)| notes)
            .unwrap_or(false)
    });
    let has_handout = uses_handouts(settings.as_ref());
    prepare_settings(&mut settings, slide_count, has_notes, has_handout);
    let has_signature = has_digital_signature(settings.as_ref());
    let template = load_template(settings.as_ref())?;

    let chart_info = collect_chart_info_lazy(slides);

    // 1. Content types (lazy version)
    write_content_types_lazy(
        zip,
        options,
        slide_count,
        slides,
        &chart_info,
        has_handout,
        settings.as_ref(),
    )?;

    // 2. Package relationships
    let rels = create_rels_xml_with_signature(has_signature);
    zip.start_file("_rels/.rels", *options)?;
    zip.write_all(rels.as_bytes())?;

    // 3. Presentation relationships
    write_presentation_relationships(
        zip,
        options,
        slide_count,
        has_notes,
        has_handout,
        settings.as_ref(),
    )?;

    // 4. Presentation document
    let mut presentation = if let Some(fonts) = embedded_fonts(settings.as_ref()) {
        create_presentation_xml_with_fonts(title, slide_count, has_notes, has_handout, fonts)
    } else {
        create_presentation_xml(title, slide_count, has_notes, has_handout)
    };
    if let Some(s) = settings.as_ref() {
        super::package_xml::append_section_lst_ext(&mut presentation, &s.sections, slide_count);
    }
    zip.start_file("ppt/presentation.xml", *options)?;
    zip.write_all(presentation.as_bytes())?;

    // 5. Standard package parts
    write_standard_package_parts(zip, options, settings.as_ref())?;

    // 6. Handout master
    if has_handout {
        write_handout_master(zip, options, settings.as_ref())?;
    }

    // 7–8–12. Slides, relationships, and charts (single pass per slide)
    write_slide_packages_lazy(
        zip,
        options,
        slides,
        &chart_info.slide_start_indices,
        template.as_ref(),
    )?;

    // 9. Notes relationships and master (lazy version)
    if has_notes {
        write_notes_relationships_lazy(zip, options, slides)?;
        write_notes_master(zip, options)?;
    }

    // 10. Theme and layouts
    write_theme_and_layouts(zip, options, settings.as_ref(), template.as_ref())?;

    // 11. Document properties
    let notes_count = (0..slide_count)
        .filter(|i| {
            slides
                .slide_features(*i)
                .map(|(notes, _)| notes)
                .unwrap_or(false)
        })
        .count();
    let slide_titles = collect_slide_titles_lazy(slides, slide_count);
    write_document_properties(zip, options, title, slide_count, notes_count, &slide_titles)?;

    // 12. Images
    write_images_lazy(zip, options, slides)?;

    // 13. Embedded font data parts
    if let Some(fonts) = embedded_fonts(settings.as_ref()) {
        write_embedded_font_parts(zip, options, fonts)?;
    }

    // 14. Digital signature package parts
    if has_signature {
        write_digital_signature_parts(zip, options, settings.as_ref())?;
    }

    // 15. Comment authors part
    if let Some(s) = settings.as_ref().filter(|s| !s.comment_authors.is_empty()) {
        zip.start_file("ppt/commentAuthors.xml", *options)?;
        zip.write_all(s.comment_authors.to_xml().as_bytes())?;
    }

    Ok(())
}

/// Write content types for lazy slides
fn write_content_types_lazy<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    slide_count: usize,
    slides: &dyn LazySlideSource,
    chart_info: &ChartInfo,
    has_handout: bool,
    settings: Option<&PresentationSettings>,
) -> Result<()> {
    let notes_count = (0..slide_count)
        .filter(|i| {
            slides
                .slide_features(*i)
                .map(|(notes, _)| notes)
                .unwrap_or(false)
        })
        .count();

    let ink_count = (0..slide_count)
        .filter(|i| {
            slides
                .generate_slide(*i)
                .map(|s| s.ink_annotations.is_some())
                .unwrap_or(false)
        })
        .count();

    let media_registry = build_media_registry_lazy(slides);
    let media_exts = media_registry.extensions();

    let mut content_types = content_types_opening(&media_exts, chart_info.total_charts);

    for i in 1..=slide_count {
        content_types.push_str(&format!(
            "\n<Override PartName=\"/ppt/slides/slide{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.slide+xml\"/>"
        ));
    }

    if notes_count > 0 {
        let mut notes_index = 0usize;
        for i in 0..slide_count {
            if slides
                .slide_features(i)
                .map(|(notes, _)| notes)
                .unwrap_or(false)
            {
                notes_index += 1;
                content_types.push_str(&format!(
                    "\n<Override PartName=\"/ppt/notesSlides/notesSlide{notes_index}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.notesSlide+xml\"/>"
                ));
            }
        }
        content_types.push_str("\n<Override PartName=\"/ppt/notesMasters/notesMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.notesMaster+xml\"/>");
        content_types.push_str("\n<Override PartName=\"/ppt/theme/theme2.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>");
    }

    if has_handout {
        content_types.push_str("\n<Override PartName=\"/ppt/handoutMasters/handoutMaster1.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.presentationml.handoutMaster+xml\"/>");
        content_types.push_str("\n<Override PartName=\"/ppt/theme/theme3.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.theme+xml\"/>");
    }

    for i in 1..=chart_info.total_charts {
        content_types.push_str(&format!(
            "\n<Override PartName=\"/ppt/charts/chart{i}.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.drawingml.chart+xml\"/>"
        ));
        content_types.push_str(&format!(
            "\n<Override PartName=\"/ppt/embeddings/{}\" ContentType=\"application/vnd.openxmlformats-officedocument.spreadsheetml.sheet\"/>",
            chart_embedding_filename(i)
        ));
    }

    content_types.push_str(
        r#"
<Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/>
<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>
<Override PartName="/ppt/tableStyles.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.tableStyles+xml"/>
<Override PartName="/ppt/viewProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.viewProps+xml"/>
<Override PartName="/ppt/presProps.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presProps+xml"/>
<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>"#,
    );
    super::layout_parts::append_layout_content_type_overrides(
        &mut content_types,
        STANDARD_LAYOUT_COUNT,
    );

    // Close the Types element BEFORE the append_* helpers run — they insert
    // their entries at rfind("</Types>"). Pushing the close tag after them
    // (the old order) made every append a silent no-op in the lazy path.
    content_types.push_str("\n</Types>");

    if has_digital_signature(settings) {
        append_digital_signature_content_type(&mut content_types);
    }
    if has_embedded_fonts(settings) {
        append_embedded_font_content_type(&mut content_types);
    }

    super::package_xml::append_ink_content_types(&mut content_types, ink_count);

    let comment_count = (0..slide_count)
        .filter(|i| {
            slides
                .generate_slide(*i)
                .map(|s| !s.comments.is_empty())
                .unwrap_or(false)
        })
        .count();
    let has_authors = has_comment_authors(settings);
    super::package_xml::append_comment_content_types(
        &mut content_types,
        comment_count,
        has_authors,
    );

    super::package_xml::append_media_content_type_defaults(
        &mut content_types,
        &media_registry.media_extensions(),
    );

    zip.start_file("[Content_Types].xml", *options)?;
    zip.write_all(content_types.as_bytes())?;
    Ok(())
}

/// Write a chart part with rels and embedded Excel workbook.
fn write_chart_package<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    chart_idx: usize,
    chart: &crate::generator::charts::Chart,
) -> Result<()> {
    let chart_xml = generate_chart_part_xml(chart);
    zip.start_file(format!("ppt/charts/chart{chart_idx}.xml"), *options)?;
    zip.write_all(chart_xml.as_bytes())?;

    let embedding_name = chart_embedding_filename(chart_idx);
    let rels_xml = create_chart_rels_xml(&embedding_name);
    zip.start_file(
        format!("ppt/charts/_rels/chart{chart_idx}.xml.rels"),
        *options,
    )?;
    zip.write_all(rels_xml.as_bytes())?;

    zip.start_file(format!("ppt/embeddings/{embedding_name}"), *options)?;
    zip.write_all(reference_workbook_bytes())?;
    Ok(())
}

/// Write slide XML, relationships, and chart parts in one pass (lazy version).
fn write_slide_packages_lazy<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    slides: &dyn LazySlideSource,
    slide_chart_start_indices: &[usize],
    template: Option<&PptxTemplate>,
) -> Result<()> {
    let mut media_registry = build_media_registry_lazy(slides);
    let mut slide_path = String::with_capacity(48);
    let mut rels_path = String::with_capacity(56);
    let mut notes_part_num = 0usize;
    let mut ink_part_num = 0usize;
    let mut comment_part_num = 0usize;

    for (i, &start_chart_idx) in slide_chart_start_indices.iter().enumerate() {
        let Some(slide) = slides.generate_slide(i) else {
            continue;
        };
        let slide_num = i + 1;
        let layout_number = resolve_layout_number(&slide, template);
        let resolved_images = slide_image_rel_targets(&slide, &media_registry);
        let poster_count = slide.videos.len() + slide.audios.len();
        let image_band_len = resolved_images.len() + poster_count;

        let start_rid = slide_content_rel_start(slide.notes.is_some(), image_band_len);
        let mut chart_rids = Vec::with_capacity(slide.charts.len());
        for j in 0..slide.charts.len() {
            push_chart_rid(&mut chart_rids, start_rid + j);
        }

        let ink_rel_id = if let Some(ref ink) = slide.ink_annotations {
            ink_part_num += 1;
            let ink_xml = ink.part_xml();
            zip.start_file(format!("ppt/ink/ink{ink_part_num}.xml"), *options)?;
            zip.write_all(ink_xml.as_bytes())?;
            Some(format!("rId{}", start_rid + slide.charts.len()))
        } else {
            None
        };

        let has_comments = !slide.comments.is_empty();
        if has_comments {
            comment_part_num += 1;
            let comments_xml = slide.comments.to_xml();
            zip.start_file(
                format!("ppt/comments/comment{comment_part_num}.xml"),
                *options,
            )?;
            zip.write_all(comments_xml.as_bytes())?;
        }
        let comment_part = has_comments.then_some(comment_part_num);

        let media_plan = slide_media_plan(
            &slide,
            &mut media_registry,
            2 + usize::from(slide.notes.is_some()),
            resolved_images.len(),
            slide_media_rid_base(
                start_rid,
                slide.charts.len(),
                slide.ink_annotations.is_some(),
                has_comments,
            ),
        )?;

        let slide_xml = create_slide_xml_with_content(
            slide_num,
            &slide,
            &chart_rids,
            ink_rel_id.as_deref(),
            &media_plan.rel_ids,
        );
        set_slide_xml_path(&mut slide_path, slide_num);
        zip.start_file(&slide_path, *options)?;
        zip.write_all(slide_xml.as_bytes())?;

        let notes_part = if slide.notes.is_some() {
            notes_part_num += 1;
            Some(notes_part_num)
        } else {
            None
        };

        if let Some(ref notes) = slide.notes {
            let notes_xml = create_notes_xml(slide_num, notes);
            set_notes_part_path(&mut slide_path, notes_part_num);
            zip.start_file(&slide_path, *options)?;
            zip.write_all(notes_xml.as_bytes())?;
        }

        let mut chart_rels = Vec::with_capacity(slide.charts.len());
        for j in 0..slide.charts.len() {
            let mut rid = String::with_capacity(8);
            rid.push_str("rId");
            append_usize(&mut rid, start_rid + j);
            let mut target = String::with_capacity(24);
            target.push_str("../charts/chart");
            append_usize(&mut target, start_chart_idx + j);
            target.push_str(".xml");
            chart_rels.push((rid, target));
        }

        let ink_rel_tuple = ink_rel_id.map(|_| (start_rid + slide.charts.len(), ink_part_num));

        let comment_rel = comment_part.map(|num| {
            (
                start_rid + slide.charts.len() + usize::from(slide.ink_annotations.is_some()),
                num,
            )
        });

        let mut images = resolved_images;
        images.extend(media_plan.poster_entries.iter().cloned());

        let slide_rels = super::package_xml::create_slide_rels_xml_with_images(
            layout_number,
            slide.notes.is_some(),
            notes_part.unwrap_or(1),
            &chart_rels,
            &images,
            &slide_hyperlink_relationships(&slide),
            ink_rel_tuple,
            comment_rel,
            &media_plan.rels_xml,
        );
        set_slide_rels_path(&mut rels_path, slide_num);
        zip.start_file(&rels_path, *options)?;
        zip.write_all(slide_rels.as_bytes())?;

        for (j, chart) in slide.charts.iter().enumerate() {
            let chart_idx = start_chart_idx + j;
            write_chart_package(zip, options, chart_idx, chart)?;
        }
    }

    Ok(())
}

/// Write notes relationship files (lazy version)
fn write_notes_relationships_lazy<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    slides: &dyn LazySlideSource,
) -> Result<()> {
    let mut notes_part_num = 0usize;
    for i in 0..slides.slide_count() {
        if slides
            .slide_features(i)
            .map(|(notes, _)| notes)
            .unwrap_or(false)
        {
            notes_part_num += 1;
            let slide_num = i + 1;
            let notes_rels = create_notes_rels_xml(slide_num);
            zip.start_file(
                format!("ppt/notesSlides/_rels/notesSlide{notes_part_num}.xml.rels"),
                *options,
            )?;
            zip.write_all(notes_rels.as_bytes())?;
        }
    }
    Ok(())
}

/// Write slide XML files (eager version)
fn write_slides<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    slide_count: usize,
    custom_slides: Option<&[SlideContent]>,
) -> Result<()> {
    let mut zip_path = String::with_capacity(48);

    match custom_slides {
        Some(slides) => {
            let mut media_registry = build_media_registry(slides);
            let mut notes_part_num = 0usize;
            let mut ink_part_num = 0usize;
            let mut comment_part_num = 0usize;
            for (i, slide) in slides.iter().enumerate() {
                let slide_num = i + 1;

                let resolved_images = slide_image_rel_targets(slide, &media_registry);
                let poster_count = slide.videos.len() + slide.audios.len();

                let mut chart_rids = Vec::with_capacity(slide.charts.len());
                let start_rid = slide_content_rel_start(
                    slide.notes.is_some(),
                    resolved_images.len() + poster_count,
                );
                for j in 0..slide.charts.len() {
                    push_chart_rid(&mut chart_rids, start_rid + j);
                }

                let ink_rel_id = if let Some(ref ink) = slide.ink_annotations {
                    ink_part_num += 1;
                    let ink_xml = ink.part_xml();
                    zip.start_file(format!("ppt/ink/ink{ink_part_num}.xml"), *options)?;
                    zip.write_all(ink_xml.as_bytes())?;
                    Some(format!("rId{}", start_rid + slide.charts.len()))
                } else {
                    None
                };

                let has_comments = !slide.comments.is_empty();
                if has_comments {
                    comment_part_num += 1;
                    let comments_xml = slide.comments.to_xml();
                    zip.start_file(
                        format!("ppt/comments/comment{comment_part_num}.xml"),
                        *options,
                    )?;
                    zip.write_all(comments_xml.as_bytes())?;
                }

                let media_plan = slide_media_plan(
                    slide,
                    &mut media_registry,
                    2 + usize::from(slide.notes.is_some()),
                    resolved_images.len(),
                    slide_media_rid_base(
                        start_rid,
                        slide.charts.len(),
                        slide.ink_annotations.is_some(),
                        has_comments,
                    ),
                )?;

                let slide_xml = create_slide_xml_with_content(
                    slide_num,
                    slide,
                    &chart_rids,
                    ink_rel_id.as_deref(),
                    &media_plan.rel_ids,
                );
                set_slide_xml_path(&mut zip_path, slide_num);
                zip.start_file(&zip_path, *options)?;
                zip.write_all(slide_xml.as_bytes())?;

                if let Some(notes) = &slide.notes {
                    notes_part_num += 1;
                    let notes_xml = create_notes_xml(slide_num, notes);
                    set_notes_part_path(&mut zip_path, notes_part_num);
                    zip.start_file(&zip_path, *options)?;
                    zip.write_all(notes_xml.as_bytes())?;
                }
            }
        }
        None => {
            for i in 1..=slide_count {
                let slide_xml = create_slide_xml(i, "Presentation");
                set_slide_xml_path(&mut zip_path, i);
                zip.start_file(&zip_path, *options)?;
                zip.write_all(slide_xml.as_bytes())?;
            }
        }
    }
    Ok(())
}

/// Write slide relationship files with notes and charts (eager version)
fn write_slide_relationships_extended<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    custom_slides: Option<&[SlideContent]>,
    slide_chart_start_indices: &[usize],
    slide_count: usize,
    template: Option<&PptxTemplate>,
) -> Result<()> {
    let mut media_registry = custom_slides.map(build_media_registry).unwrap_or_default();
    let mut notes_part_num = 0usize;

    match custom_slides {
        Some(slides) => {
            let mut zip_path = String::with_capacity(56);
            let mut ink_part_num = 0usize;
            let mut comment_part_num = 0usize;
            for (i, slide) in slides.iter().enumerate() {
                let slide_num = i + 1;
                let layout_number = resolve_layout_number(slide, template);
                let notes_part = if slide.notes.is_some() {
                    notes_part_num += 1;
                    Some(notes_part_num)
                } else {
                    None
                };

                let mut chart_rels = Vec::with_capacity(slide.charts.len());
                let start_chart_idx = slide_chart_start_indices[i];
                let resolved_images = slide_image_rel_targets(slide, &media_registry);
                let poster_count = slide.videos.len() + slide.audios.len();
                let start_rid = slide_content_rel_start(
                    slide.notes.is_some(),
                    resolved_images.len() + poster_count,
                );

                for j in 0..slide.charts.len() {
                    let mut rid = String::with_capacity(8);
                    rid.push_str("rId");
                    append_usize(&mut rid, start_rid + j);
                    let mut target = String::with_capacity(24);
                    target.push_str("../charts/chart");
                    append_usize(&mut target, start_chart_idx + j);
                    target.push_str(".xml");
                    chart_rels.push((rid, target));
                }

                let ink_rel_tuple = if slide.ink_annotations.is_some() {
                    ink_part_num += 1;
                    Some((start_rid + slide.charts.len(), ink_part_num))
                } else {
                    None
                };

                let has_comments = !slide.comments.is_empty();
                let comment_rel = if has_comments {
                    comment_part_num += 1;
                    Some((
                        start_rid
                            + slide.charts.len()
                            + usize::from(slide.ink_annotations.is_some()),
                        comment_part_num,
                    ))
                } else {
                    None
                };

                let media_plan = slide_media_plan(
                    slide,
                    &mut media_registry,
                    2 + usize::from(slide.notes.is_some()),
                    resolved_images.len(),
                    slide_media_rid_base(
                        start_rid,
                        slide.charts.len(),
                        slide.ink_annotations.is_some(),
                        has_comments,
                    ),
                )?;

                let mut images = resolved_images;
                images.extend(media_plan.poster_entries.iter().cloned());

                let slide_rels = super::package_xml::create_slide_rels_xml_with_images(
                    layout_number,
                    slide.notes.is_some(),
                    notes_part.unwrap_or(1),
                    &chart_rels,
                    &images,
                    &slide_hyperlink_relationships(slide),
                    ink_rel_tuple,
                    comment_rel,
                    &media_plan.rels_xml,
                );
                set_slide_rels_path(&mut zip_path, slide_num);
                zip.start_file(&zip_path, *options)?;
                zip.write_all(slide_rels.as_bytes())?;
            }
        }
        None => {
            let mut zip_path = String::with_capacity(56);
            for i in 1..=slide_count {
                let slide_rels = create_slide_rels_xml();
                set_slide_rels_path(&mut zip_path, i);
                zip.start_file(&zip_path, *options)?;
                zip.write_all(slide_rels.as_bytes())?;
            }
        }
    }
    Ok(())
}

/// Write chart files (eager version)
fn write_charts<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    custom_slides: Option<&[SlideContent]>,
    slide_chart_start_indices: &[usize],
) -> Result<()> {
    if let Some(slides) = custom_slides {
        for (i, slide) in slides.iter().enumerate() {
            let start_chart_idx = slide_chart_start_indices[i];
            for (j, chart) in slide.charts.iter().enumerate() {
                let chart_idx = start_chart_idx + j;
                write_chart_package(zip, options, chart_idx, chart)?;
            }
        }
    }
    Ok(())
}

/// Write notes relationship files (eager version)
fn write_notes_relationships<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    custom_slides: Option<&[SlideContent]>,
) -> Result<()> {
    if let Some(slides) = custom_slides {
        let mut notes_part_num = 0usize;
        for (i, slide) in slides.iter().enumerate() {
            if slide.notes.is_some() {
                notes_part_num += 1;
                let slide_num = i + 1;
                let notes_rels = create_notes_rels_xml(slide_num);
                zip.start_file(
                    format!("ppt/notesSlides/_rels/notesSlide{notes_part_num}.xml.rels"),
                    *options,
                )?;
                zip.write_all(notes_rels.as_bytes())?;
            }
        }
    }
    Ok(())
}

/// Write embedded font data parts to `ppt/fonts/`.
fn write_embedded_font_parts<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    fonts: &super::slide_content::embedded_fonts::EmbeddedFontList,
) -> Result<()> {
    for font in fonts.fonts() {
        zip.start_file(font.part_name(), *options)?;
        zip.write_all(&font.data)?;
    }
    Ok(())
}

/// Write image files to ppt/media/
fn write_images<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    custom_slides: Option<&[SlideContent]>,
) -> Result<()> {
    if let Some(slides) = custom_slides {
        let registry = build_media_registry(slides);
        for (i, (bytes, ext)) in registry.files().iter().enumerate() {
            let filename = format!("ppt/media/image{}.{}", i + 1, ext);
            zip.start_file(filename, *options)?;
            zip.write_all(bytes)?;
        }
        write_media_files(zip, options, &registry)?;
    }
    Ok(())
}

/// Write digital signature parts (`_xmlsignatures/`).
fn write_digital_signature_parts<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    settings: Option<&PresentationSettings>,
) -> Result<()> {
    let signature = settings
        .and_then(|s| s.digital_signature.as_ref())
        .expect("digital signature parts requested but none configured");

    // `origin.sigs` is the signature origin relationships part.
    zip.start_file("_xmlsignatures/origin.sigs", *options)?;
    zip.write_all(signature.to_origin_xml().as_bytes())?;

    zip.start_file("_xmlsignatures/sig1.xml", *options)?;
    zip.write_all(signature.to_signature_xml().as_bytes())?;

    Ok(())
}

/// Write image files from a lazy slide source.
fn write_images_lazy<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    slides: &dyn LazySlideSource,
) -> Result<()> {
    let registry = build_media_registry_lazy(slides);
    for (i, (bytes, ext)) in registry.files().iter().enumerate() {
        let filename = format!("ppt/media/image{}.{}", i + 1, ext);
        zip.start_file(filename, *options)?;
        zip.write_all(bytes)?;
    }
    write_media_files(zip, options, &registry)?;
    Ok(())
}

/// Write video/audio parts (`ppt/media/mediaN.{ext}`).
fn write_media_files<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    options: &FileOptions,
    registry: &MediaRegistry,
) -> Result<()> {
    for (i, (bytes, ext)) in registry.media_files().iter().enumerate() {
        let filename = format!("ppt/media/media{}.{}", i + 1, ext);
        zip.start_file(filename, *options)?;
        zip.write_all(bytes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Read};

    /// A simple test slide source that generates numbered slides
    struct TestSlideSource {
        count: usize,
        with_notes: bool,
    }

    impl LazySlideSource for TestSlideSource {
        fn slide_count(&self) -> usize {
            self.count
        }

        fn generate_slide(&self, index: usize) -> Option<SlideContent> {
            if index < self.count {
                let mut slide = SlideContent::new(&format!("Slide {}", index + 1))
                    .add_bullet(&format!("Point {}", index + 1));

                if self.with_notes {
                    slide.notes = Some("Speaker notes here".to_string());
                }

                Some(slide)
            } else {
                None
            }
        }

        fn slide_has_notes(&self, index: usize) -> bool {
            self.with_notes && index < self.count
        }

        fn slide_chart_count(&self, _index: usize) -> usize {
            0
        }
    }

    #[test]
    fn test_create_pptx_to_writer() {
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let result = create_pptx_to_writer(cursor, "Test Presentation", 3);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_pptx_with_digital_signature() {
        use crate::generator::slide_content::{DigitalSignature, SignerInfo};

        let signature = DigitalSignature::new(SignerInfo::new("Alice"));
        let settings = PresentationSettings::new().digital_signature(signature);
        let bytes = create_pptx_with_settings(
            "Signed",
            &[SlideContent::new("Slide 1").add_bullet("Point 1")],
            Some(settings),
        )
        .unwrap();

        let report = crate::core::validate_package_bytes(&bytes);
        assert!(
            report.is_valid(),
            "signature package invalid: {:?}",
            report.issues
        );

        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let names: std::collections::HashSet<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains("_xmlsignatures/origin.sigs"));
        assert!(names.contains("_xmlsignatures/sig1.xml"));
        assert!(names.contains("_rels/.rels"));
    }

    #[test]
    fn test_create_pptx_with_embedded_fonts() {
        use crate::generator::slide_content::{EmbeddedFont, EmbeddedFontList, FontStyle};

        let mut fonts = EmbeddedFontList::new();
        fonts.add(EmbeddedFont::new(
            "Arial",
            FontStyle::Regular,
            vec![0u8; 20],
            "",
        ));
        let settings = PresentationSettings::new().embedded_fonts(fonts);

        let bytes = create_pptx_with_settings(
            "Font Demo",
            &[SlideContent::new("Slide 1").add_bullet("Point 1")],
            Some(settings),
        )
        .unwrap();

        let report = crate::core::validate_package_bytes(&bytes);
        assert!(
            report.is_valid(),
            "embedded font package invalid: {:?}",
            report.issues
        );

        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let names: std::collections::HashSet<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains("ppt/fonts/Arial-regular.fntdata"));

        let mut rels = String::new();
        archive
            .by_name("ppt/_rels/presentation.xml.rels")
            .unwrap()
            .read_to_string(&mut rels)
            .unwrap();
        assert!(rels.contains("relationships/font"));
        assert!(rels.contains("fonts/Arial-regular.fntdata"));

        let mut presentation = String::new();
        archive
            .by_name("ppt/presentation.xml")
            .unwrap()
            .read_to_string(&mut presentation)
            .unwrap();
        assert!(presentation.contains("<p:embeddedFontLst>"));
        assert!(presentation.contains("Arial"));
    }

    #[test]
    fn test_create_pptx_with_ink_annotations() {
        use crate::generator::slide_content::{InkAnnotations, InkPen, InkStroke};

        let mut ink = InkAnnotations::new();
        ink.add_stroke(
            InkStroke::new(InkPen::red())
                .add_point(100.0, 100.0)
                .add_point(200.0, 200.0),
        );

        let bytes = create_pptx_with_content(
            "Ink Demo",
            vec![
                SlideContent::new("Slide 1")
                    .add_bullet("Point 1")
                    .with_ink(ink),
            ],
        )
        .unwrap();

        let report = crate::core::validate_package_bytes(&bytes);
        assert!(
            report.is_valid(),
            "ink package invalid: {:?}",
            report.issues
        );

        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();
        let names: std::collections::HashSet<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().to_string())
            .collect();
        assert!(names.contains("ppt/ink/ink1.xml"));

        let mut rels = String::new();
        archive
            .by_name("ppt/slides/_rels/slide1.xml.rels")
            .unwrap()
            .read_to_string(&mut rels)
            .unwrap();
        assert!(rels.contains("relationships/ink"));
        assert!(rels.contains("../ink/ink1.xml"));

        let mut slide = String::new();
        archive
            .by_name("ppt/slides/slide1.xml")
            .unwrap()
            .read_to_string(&mut slide)
            .unwrap();
        assert!(slide.contains("mc:AlternateContent"));
        assert!(slide.contains("p:contentPart"));
    }

    #[test]
    fn test_create_pptx_with_content_to_writer() {
        let slides = vec![
            SlideContent::new("Title").add_bullet("Point 1"),
            SlideContent::new("Slide 2").add_bullet("Point 2"),
        ];

        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let result = create_pptx_with_content_to_writer(cursor, "Test", &slides, None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_pptx_lazy_to_writer() {
        let source = TestSlideSource {
            count: 10,
            with_notes: false,
        };

        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let result = create_pptx_lazy_to_writer(cursor, "Lazy Test", Box::new(source), None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_pptx_lazy_with_notes() {
        let source = TestSlideSource {
            count: 5,
            with_notes: true,
        };

        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let result =
            create_pptx_lazy_to_writer(cursor, "Lazy Test with Notes", Box::new(source), None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_lazy_slide_source() {
        let source = TestSlideSource {
            count: 3,
            with_notes: false,
        };

        assert_eq!(source.slide_count(), 3);
        assert!(!source.slide_has_notes(0));
        assert_eq!(source.slide_chart_count(0), 0);

        let slide = source.generate_slide(0);
        assert!(slide.is_some());
        assert_eq!(slide.unwrap().title, "Slide 1");

        let out_of_bounds = source.generate_slide(10);
        assert!(out_of_bounds.is_none());
    }

    #[test]
    fn test_streaming_api_compatibility() {
        // Test that the streaming API produces the same output as the in-memory API
        let slides = vec![SlideContent::new("Test").add_bullet("Item 1")];

        // In-memory version
        let in_memory = create_pptx_with_content("Test", slides.clone()).unwrap();

        // Streaming version
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let streaming = create_pptx_with_content_to_writer(cursor, "Test", &slides, None)
            .unwrap()
            .into_inner();

        // Both should produce valid ZIP files (non-empty)
        assert!(!in_memory.is_empty());
        assert!(!streaming.is_empty());
    }

    #[test]
    fn test_lazy_vs_eager_compatibility() {
        // Test that lazy and eager APIs produce compatible output
        let eager_slides = vec![
            SlideContent::new("Slide 1").add_bullet("Point 1"),
            SlideContent::new("Slide 2").add_bullet("Point 2"),
        ];

        // Eager version
        let eager = create_pptx_with_content("Test", eager_slides).unwrap();

        // Lazy version (equivalent content)
        let source = TestSlideSource {
            count: 2,
            with_notes: false,
        };
        let buffer = Vec::new();
        let cursor = Cursor::new(buffer);
        let lazy = create_pptx_lazy_to_writer(cursor, "Test", Box::new(source), None)
            .unwrap()
            .into_inner();

        // Both should produce valid ZIP files
        assert!(!eager.is_empty());
        assert!(!lazy.is_empty());
    }
}
