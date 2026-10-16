//! Slide layout XML generators
//!
//! Each layout is a separate module with atomic responsibility.

mod blank;
mod centered_title;
mod common;
mod title_content;
mod title_only;
mod two_column;

pub use blank::BlankLayout;
pub use centered_title::CenteredTitleLayout;
pub use common::{
    ExtendedTextProps, SlideXmlBuilder, generate_text_props, generate_text_props_extended,
};
pub use title_content::{TitleBigContentLayout, TitleContentLayout};
pub use title_only::TitleOnlyLayout;
pub use two_column::TwoColumnLayout;

use super::slide_content::{SlideContent, SlideLayout};

/// Generate slide XML based on layout type
pub fn create_slide_xml_for_layout(content: &SlideContent) -> String {
    match content.layout {
        SlideLayout::Blank => BlankLayout::generate(),
        SlideLayout::TitleOnly => TitleOnlyLayout::generate(content),
        SlideLayout::CenteredTitle => CenteredTitleLayout::generate(content),
        SlideLayout::TitleAndBigContent => TitleBigContentLayout::generate(content),
        SlideLayout::TwoColumn => TwoColumnLayout::generate(content),
        SlideLayout::SectionHeader => CenteredTitleLayout::generate(content),
        SlideLayout::TitleAndContent => TitleContentLayout::generate(content),
    }
}
