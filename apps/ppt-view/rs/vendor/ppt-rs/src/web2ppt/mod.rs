//! Web2PPT - Convert webpages to PowerPoint presentations
//!
//! This module provides functionality to fetch webpages, extract content,
//! and generate PowerPoint presentations from the extracted content.

#[cfg(feature = "web2ppt")]
mod config;
#[cfg(feature = "web2ppt")]
mod converter;
#[cfg(feature = "web2ppt")]
mod fetcher;
#[cfg(feature = "web2ppt")]
mod parser;
#[cfg(feature = "web2md")]
mod web2md_bridge;

#[cfg(feature = "web2ppt")]
pub use config::Web2PptConfig;
#[cfg(feature = "web2ppt")]
pub use converter::{
    ConversionOptions, Web2Ppt, html_to_pptx, html_to_pptx_with_options, url_to_pptx,
    url_to_pptx_with_options,
};
#[cfg(feature = "web2ppt")]
pub use fetcher::WebFetcher;
#[cfg(feature = "web2ppt")]
pub use parser::{ContentBlock, ContentType, WebContent, WebParser};
#[cfg(feature = "web2md")]
pub use web2md_bridge::{html_to_pptx_via_web2md, url_to_pptx_via_web2md};

/// Error types for web2ppt operations
#[derive(Debug, thiserror::Error)]
pub enum Web2PptError {
    #[error("Failed to fetch URL: {0}")]
    FetchError(String),

    #[error("Failed to parse HTML: {0}")]
    ParseError(String),

    #[error("Failed to generate presentation: {0}")]
    GenerationError(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("No content found on page")]
    NoContent,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Web2PptError>;
