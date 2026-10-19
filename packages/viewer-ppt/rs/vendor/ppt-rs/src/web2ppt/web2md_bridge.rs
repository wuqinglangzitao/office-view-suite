//! Bridge from web content to PowerPoint via the `web2md` crate.
//!
//! Pipeline: URL → HTML (WebFetcher) → Markdown (web2md::PageToMarkdown)
//!           → Slides (cli::markdown::parser) → PPTX bytes.

use super::converter::ConversionOptions;
use super::fetcher::WebFetcher;
use super::{Result, Web2PptConfig, Web2PptError};

/// Convert a URL to PPTX bytes using the web2md pipeline.
///
/// Fetches the page with the existing blocking `WebFetcher`, converts HTML to
/// Markdown via `web2md::PageToMarkdown::convert`, then parses the Markdown into
/// slides with the built-in markdown parser.
pub fn url_to_pptx_via_web2md(
    url: &str,
    config: Web2PptConfig,
    options: &ConversionOptions,
) -> Result<Vec<u8>> {
    let fetcher = WebFetcher::with_config(config.clone())?;
    let html = fetcher.fetch(url)?;
    html_to_pptx_via_web2md(&html, url, config, options)
}

/// Convert an HTML string to PPTX bytes using the web2md pipeline.
pub fn html_to_pptx_via_web2md(
    html: &str,
    url: &str,
    config: Web2PptConfig,
    options: &ConversionOptions,
) -> Result<Vec<u8>> {
    // HTML → Markdown via web2md
    let exclude: Vec<String> = Vec::new();
    let markdown = web2md::PageToMarkdown::convert(
        html,
        config.include_images,
        true, // keep_header
        true, // main_content — extract <article>/<main>
        &exclude,
    )
    .map_err(|e| Web2PptError::ParseError(format!("web2md conversion failed: {e}")))?;

    if markdown.trim().is_empty() {
        return Err(Web2PptError::NoContent);
    }

    // Markdown → Slides via built-in parser
    let mut slides = crate::cli::markdown::parse(&markdown)
        .map_err(|e| Web2PptError::ParseError(format!("Markdown parse failed: {e}")))?;

    // Apply slide limit from config
    if slides.len() > config.max_slides {
        slides.truncate(config.max_slides);
    }

    // Override title if requested
    let title = options.title.clone().unwrap_or_else(|| {
        extract_title_from_markdown(&markdown).unwrap_or_else(|| url.to_string())
    });

    crate::create_pptx_with_content(&title, slides)
        .map_err(|e| Web2PptError::GenerationError(e.to_string()))
}

/// Extract the first H1 heading text from a Markdown string.
fn extract_title_from_markdown(md: &str) -> Option<String> {
    for line in md.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("# ") {
            let title = rest.trim().to_string();
            if !title.is_empty() {
                return Some(title);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_title_from_markdown() {
        let md = "# My Title\n\nSome content";
        assert_eq!(
            extract_title_from_markdown(md),
            Some("My Title".to_string())
        );
    }

    #[test]
    fn test_extract_title_missing() {
        let md = "No heading here";
        assert_eq!(extract_title_from_markdown(md), None);
    }

    #[test]
    fn test_html_to_pptx_via_web2md() {
        let html = r#"<!DOCTYPE html>
<html>
<head><title>Test Page</title></head>
<body>
<article>
<h1>Main Title</h1>
<p>This is a paragraph with enough text to be included in the presentation.</p>
<h2>Section 1</h2>
<p>Section 1 content with enough text to be included in the presentation.</p>
<ul>
<li>Item 1</li>
<li>Item 2</li>
</ul>
</article>
</body>
</html>"#;

        let config = Web2PptConfig::default();
        let options = ConversionOptions::default();
        let result = html_to_pptx_via_web2md(html, "https://example.com", config, &options);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
        let pptx = result.unwrap();
        assert!(!pptx.is_empty());
    }

    #[test]
    fn test_html_to_pptx_via_web2md_empty() {
        let html = "<html><body></body></html>";
        let config = Web2PptConfig::default();
        let options = ConversionOptions::default();
        let result = html_to_pptx_via_web2md(html, "https://example.com", config, &options);
        assert!(result.is_err());
    }

    #[test]
    fn test_html_to_pptx_via_web2md_custom_title() {
        let html = r#"<html><body><article>
<h1>Original Title</h1>
<p>Some content here that is long enough to be included.</p>
</article></body></html>"#;

        let config = Web2PptConfig::default();
        let options = ConversionOptions::new().title("Custom Title");
        let result = html_to_pptx_via_web2md(html, "https://example.com", config, &options);
        assert!(result.is_ok());
    }
}
