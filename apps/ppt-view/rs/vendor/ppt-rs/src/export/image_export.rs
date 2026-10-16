//! Image export module
//!
//! Provides functionality to export presentations and individual slides
//! to image formats (PNG, JPEG).
//!
//! **No LibreOffice dependency.** Slides are rendered to PDF with the
//! pure-Rust `pdfrs` engine (see [`crate::export::slide_render`]), then
//! rasterized to images with `pdftoppm` (poppler):
//!
//! 1. `slide_render::render_to_pdf_bytes` — all slides, one PDF page each
//! 2. `pdftoppm -png|-jpeg` — one image per page
//!
//! The only external requirement is `pdftoppm`. Each export uses a unique
//! temp working directory, so concurrent exports never collide.

use crate::api::Presentation;
use crate::exc::{PptxError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Image format for export
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ImageFormat {
    /// PNG format (lossless, good for graphics)
    #[default]
    Png,
    /// JPEG format (lossy, good for photos)
    Jpeg,
}

impl ImageFormat {
    /// Get file extension
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            ImageFormat::Jpeg => "jpg",
        }
    }

    /// Get MIME type
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
        }
    }
}

/// Options for image export
#[derive(Debug, Clone)]
pub struct ImageExportOptions {
    /// Image format (PNG or JPEG)
    pub format: ImageFormat,
    /// DPI/resolution (default 150)
    pub dpi: u32,
    /// JPEG quality (0-100, default 90)
    pub jpeg_quality: u8,
    /// Output width in pixels (0 = auto based on DPI)
    pub width: u32,
    /// Output height in pixels (0 = auto based on DPI)
    pub height: u32,
    /// Export all slides or specific slide (0 = all, 1+ = specific slide)
    pub slide_number: usize,
}

impl Default for ImageExportOptions {
    fn default() -> Self {
        Self {
            format: ImageFormat::Png,
            dpi: 150,
            jpeg_quality: 90,
            width: 0,
            height: 0,
            slide_number: 0,
        }
    }
}

impl ImageExportOptions {
    /// Create new options with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set image format
    pub fn with_format(mut self, format: ImageFormat) -> Self {
        self.format = format;
        self
    }

    /// Set DPI
    pub fn with_dpi(mut self, dpi: u32) -> Self {
        self.dpi = dpi;
        self
    }

    /// Set JPEG quality
    pub fn with_jpeg_quality(mut self, quality: u8) -> Self {
        self.jpeg_quality = quality.min(100);
        self
    }

    /// Set output dimensions
    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    /// Set specific slide to export (1-based, 0 = all)
    pub fn with_slide(mut self, slide: usize) -> Self {
        self.slide_number = slide;
        self
    }

    /// High quality preset (300 DPI, PNG)
    pub fn high_quality() -> Self {
        Self {
            format: ImageFormat::Png,
            dpi: 300,
            jpeg_quality: 95,
            width: 0,
            height: 0,
            slide_number: 0,
        }
    }

    /// Web optimized preset (96 DPI, JPEG)
    pub fn web_optimized() -> Self {
        Self {
            format: ImageFormat::Jpeg,
            dpi: 96,
            jpeg_quality: 85,
            width: 0,
            height: 0,
            slide_number: 0,
        }
    }
}

/// Export presentation to images
///
/// Slides are rendered to PDF with the pure-Rust `pdfrs` engine and
/// rasterized with `pdftoppm` (poppler). Requires `pdftoppm` to be
/// installed; LibreOffice is not needed.
///
/// Images are written to `output_dir` as `Slide1.png`, `Slide2.png`, …
/// (sorted by slide number) and the returned paths are in slide order.
///
/// # Arguments
/// * `presentation` - The presentation to export
/// * `output_dir` - Directory to save images
/// * `options` - Export options
///
/// # Returns
/// Vector of paths to generated image files
pub fn export_to_images<P: AsRef<Path>>(
    presentation: &Presentation,
    output_dir: P,
    options: &ImageExportOptions,
) -> Result<Vec<PathBuf>> {
    if presentation.slide_count() == 0 {
        return Err(PptxError::InvalidState(
            crate::exc::messages::must_not_be_empty("presentation slides for image export"),
        ));
    }

    if !is_pdftoppm_available() {
        return Err(PptxError::Generic(String::from(
            "pdftoppm not found (install poppler)",
        )));
    }

    let output_dir = output_dir.as_ref();
    std::fs::create_dir_all(output_dir)?;

    // Unique working directory: prevents temp-file collisions between
    // concurrent exports and stale-output collection.
    let work_dir = std::env::temp_dir().join(format!("ppt_rs_img_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&work_dir)?;

    let result = export_to_images_in(&work_dir, output_dir, presentation, options);

    let _ = std::fs::remove_dir_all(&work_dir);
    result
}

#[cfg(feature = "pdf-native")]
fn export_to_images_in(
    work_dir: &Path,
    output_dir: &Path,
    presentation: &Presentation,
    options: &ImageExportOptions,
) -> Result<Vec<PathBuf>> {
    // Pure-Rust vector render: one PDF page per slide (no LibreOffice)
    let pdf = work_dir.join("deck.pdf");
    let bytes = crate::export::slide_render::render_to_pdf_bytes(presentation)?;
    std::fs::write(&pdf, bytes)?;

    let prefix = work_dir.join("Slide");
    let args = pdftoppm_args(options, &pdf, &prefix);
    let output = Command::new("pdftoppm")
        .args(&args)
        .output()
        .map_err(|e| PptxError::Generic(format!("Failed to execute pdftoppm: {}", e)))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(PptxError::Generic(format!(
            "pdftoppm conversion failed: {}",
            stderr.trim()
        )));
    }

    collect_output_images(work_dir, output_dir, options.format)
}

/// Export a specific slide to an image
pub fn export_slide_to_image<P: AsRef<Path>>(
    presentation: &Presentation,
    slide_number: usize,
    output_path: P,
    options: &ImageExportOptions,
) -> Result<PathBuf> {
    if slide_number == 0 || slide_number > presentation.slide_count() {
        return Err(PptxError::InvalidOperation(format!(
            "Invalid slide number: {} (presentation has {} slides)",
            slide_number,
            presentation.slide_count()
        )));
    }

    let mut slide_options = options.clone();
    slide_options.slide_number = slide_number;

    let output_dir = output_path.as_ref().parent().unwrap_or(Path::new("."));

    let paths = export_to_images(presentation, output_dir, &slide_options)?;

    let rendered = paths
        .first()
        .ok_or_else(|| PptxError::Generic(format!("Slide {} was not rendered", slide_number)))?;

    // Rename to requested output path if different
    if rendered != output_path.as_ref() {
        std::fs::rename(rendered, output_path.as_ref())?;
    }
    Ok(output_path.as_ref().to_path_buf())
}

/// Check if `pdftoppm` (poppler) is available
fn is_pdftoppm_available() -> bool {
    Command::new("pdftoppm")
        .arg("-v")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .is_ok()
}

/// Build the `pdftoppm` argument list for the given options.
///
/// Output files are named `<prefix>-<page>.<ext>` where `<page>` is
/// zero-padded to the digit width of the last page (e.g. `Slide-01.png`).
fn pdftoppm_args(options: &ImageExportOptions, pdf: &Path, prefix: &Path) -> Vec<String> {
    let mut args = Vec::new();

    match options.format {
        ImageFormat::Png => args.push(String::from("-png")),
        ImageFormat::Jpeg => args.push(String::from("-jpeg")),
    }

    if options.width > 0 || options.height > 0 {
        // Explicit pixel size; a -1 dimension preserves the aspect ratio.
        args.push(String::from("-scale-to-x"));
        if options.width > 0 {
            args.push(options.width.to_string());
        } else {
            args.push(String::from("-1"));
        }
        args.push(String::from("-scale-to-y"));
        if options.height > 0 {
            args.push(options.height.to_string());
        } else {
            args.push(String::from("-1"));
        }
    } else {
        args.push(String::from("-r"));
        args.push(options.dpi.to_string());
    }

    if options.format == ImageFormat::Jpeg {
        args.push(String::from("-jpegopt"));
        args.push(format!("quality={}", options.jpeg_quality));
    }

    if options.slide_number > 0 {
        args.push(String::from("-f"));
        args.push(options.slide_number.to_string());
        args.push(String::from("-l"));
        args.push(options.slide_number.to_string());
    }

    args.push(pdf.display().to_string());
    args.push(prefix.display().to_string());
    args
}

/// Move rasterized pages from the working directory into `output_dir`,
/// renamed to `Slide{n}.{ext}`, sorted by slide number.
fn collect_output_images(
    work_dir: &Path,
    output_dir: &Path,
    format: ImageFormat,
) -> Result<Vec<PathBuf>> {
    let ext = format.extension();
    let mut named: Vec<(usize, PathBuf)> = Vec::new();

    for entry in std::fs::read_dir(work_dir)? {
        let path = entry?.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("Slide-") {
            continue;
        }
        if Path::new(name)
            .extension()
            .and_then(|e| e.to_str())
            .is_none_or(|e| !e.eq_ignore_ascii_case(ext))
        {
            continue;
        }
        let Some(page) = parse_page_number(name) else {
            continue;
        };

        let final_path = output_dir.join(format!("Slide{}.{}", page, ext));
        std::fs::rename(&path, &final_path)?;
        named.push((page, final_path));
    }

    named.sort_by_key(|(page, _)| *page);
    Ok(named.into_iter().map(|(_, p)| p).collect())
}

/// Extract the page number from a pdftoppm output file name
/// (e.g. `Slide-01.png` → 1, `Slide3.jpg` → 3).
fn parse_page_number(file_name: &str) -> Option<usize> {
    let stem = file_name.rsplit_once('.')?.0;
    let digits: String = stem.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Render presentation thumbnail (first slide only)
pub fn render_thumbnail<P: AsRef<Path>>(
    presentation: &Presentation,
    output_path: P,
    width: u32,
) -> Result<PathBuf> {
    let options = ImageExportOptions::new()
        .with_format(ImageFormat::Png)
        .with_slide(1);

    // Calculate DPI based on desired width
    let dpi = (width as f32 / 10.0) as u32;

    let options = ImageExportOptions { dpi, ..options };

    export_slide_to_image(presentation, 1, output_path, &options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_format_extension() {
        assert_eq!(ImageFormat::Png.extension(), "png");
        assert_eq!(ImageFormat::Jpeg.extension(), "jpg");
    }

    #[test]
    fn test_image_export_options() {
        let opts = ImageExportOptions::new()
            .with_format(ImageFormat::Jpeg)
            .with_dpi(200)
            .with_jpeg_quality(85);

        assert_eq!(opts.format, ImageFormat::Jpeg);
        assert_eq!(opts.dpi, 200);
        assert_eq!(opts.jpeg_quality, 85);
    }

    #[test]
    fn test_high_quality_preset() {
        let opts = ImageExportOptions::high_quality();
        assert_eq!(opts.dpi, 300);
        assert_eq!(opts.format, ImageFormat::Png);
    }

    #[test]
    fn test_web_optimized_preset() {
        let opts = ImageExportOptions::web_optimized();
        assert_eq!(opts.dpi, 96);
        assert_eq!(opts.format, ImageFormat::Jpeg);
        assert_eq!(opts.jpeg_quality, 85);
    }

    #[test]
    fn test_parse_page_number() {
        assert_eq!(parse_page_number("Slide-1.png"), Some(1));
        assert_eq!(parse_page_number("Slide-01.png"), Some(1));
        assert_eq!(parse_page_number("Slide-44.png"), Some(44));
        assert_eq!(parse_page_number("Slide3.jpg"), Some(3));
        assert_eq!(parse_page_number("other.png"), None);
        assert_eq!(parse_page_number("Slide-.png"), None);
    }

    #[test]
    fn test_pdftoppm_args_png_dpi() {
        let options = ImageExportOptions::new().with_dpi(150);
        let args = pdftoppm_args(
            &options,
            Path::new("/tmp/deck.pdf"),
            Path::new("/tmp/Slide"),
        );
        assert_eq!(
            args,
            vec!["-png", "-r", "150", "/tmp/deck.pdf", "/tmp/Slide"]
        );
    }

    #[test]
    fn test_pdftoppm_args_jpeg_quality() {
        let options = ImageExportOptions::new()
            .with_format(ImageFormat::Jpeg)
            .with_dpi(96)
            .with_jpeg_quality(85);
        let args = pdftoppm_args(
            &options,
            Path::new("/tmp/deck.pdf"),
            Path::new("/tmp/Slide"),
        );
        assert_eq!(
            args,
            vec![
                "-jpeg",
                "-r",
                "96",
                "-jpegopt",
                "quality=85",
                "/tmp/deck.pdf",
                "/tmp/Slide"
            ]
        );
    }

    #[test]
    fn test_pdftoppm_args_dimensions_override_dpi() {
        let options = ImageExportOptions::new().with_dimensions(640, 0);
        let args = pdftoppm_args(
            &options,
            Path::new("/tmp/deck.pdf"),
            Path::new("/tmp/Slide"),
        );
        assert_eq!(
            args,
            vec![
                "-png",
                "-scale-to-x",
                "640",
                "-scale-to-y",
                "-1",
                "/tmp/deck.pdf",
                "/tmp/Slide"
            ]
        );
    }

    #[test]
    fn test_pdftoppm_args_page_selection() {
        let options = ImageExportOptions::new().with_slide(2);
        let args = pdftoppm_args(
            &options,
            Path::new("/tmp/deck.pdf"),
            Path::new("/tmp/Slide"),
        );
        assert_eq!(
            args,
            vec![
                "-png",
                "-r",
                "150",
                "-f",
                "2",
                "-l",
                "2",
                "/tmp/deck.pdf",
                "/tmp/Slide"
            ]
        );
    }

    #[test]
    fn test_collect_output_images_renames_and_sorts() {
        let work = std::env::temp_dir().join(format!("ppt_rs_collect_{}", uuid::Uuid::new_v4()));
        let out = std::env::temp_dir().join(format!("ppt_rs_collect_out_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&work).unwrap();
        std::fs::create_dir_all(&out).unwrap();

        // pdftoppm zero-pads page numbers to the digit width of the last page
        for name in [
            "Slide-3.png",
            "Slide-01.png",
            "Slide-2.png",
            "deck.pdf",
            "Slide-x.png",
        ] {
            std::fs::write(work.join(name), b"").unwrap();
        }

        let paths = collect_output_images(&work, &out, ImageFormat::Png).unwrap();
        assert_eq!(
            paths
                .iter()
                .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
                .collect::<Vec<_>>(),
            vec!["Slide1.png", "Slide2.png", "Slide3.png"]
        );
        for p in &paths {
            assert!(p.exists());
        }
        // Non-page files stay behind; work dir only holds deck.pdf now
        assert!(work.join("deck.pdf").exists());

        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_dir_all(&out);
    }
}
