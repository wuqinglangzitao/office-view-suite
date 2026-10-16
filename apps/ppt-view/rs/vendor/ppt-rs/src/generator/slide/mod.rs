//! Slide generation module
//!
//! Contains utilities for generating slide XML with various layouts
//! and text formatting.

pub mod formatting;

pub use formatting::{
    TextSegment, generate_rich_text_runs, generate_text_props, parse_inline_formatting,
};
