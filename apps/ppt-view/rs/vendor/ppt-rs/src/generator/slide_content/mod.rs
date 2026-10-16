//! Slide content and layout types
//!
//! This module contains types for building slide content:
//! - `BulletStyle` - Bullet point styles (numbered, lettered, roman, custom)
//! - `BulletPoint` - Individual bullet point with formatting
//! - `BulletTextFormat` - Text formatting for bullet points
//! - `SlideLayout` - Layout types (title only, title and content, etc.)
//! - `SlideContent` - Complete slide content builder
//! - `CodeBlock` - Code block with syntax highlighting

mod bullet;
mod code_block;
pub mod comments;
mod content;
pub mod digital_signature;
pub mod embedded_fonts;
pub mod ink_annotations;
mod layout;
pub mod presentation_settings;
pub mod print_settings;
pub mod sections;
pub mod slide_show_settings;
pub mod table_merge;
pub mod transition;

pub use bullet::{BulletPoint, BulletStyle, BulletTextFormat};
pub use code_block::CodeBlock;
pub use comments::{Comment, CommentAuthor, CommentAuthorList, SlideComments};
pub use content::SlideContent;
pub use digital_signature::{DigitalSignature, HashAlgorithm, SignatureCommitment, SignerInfo};
pub use embedded_fonts::{EmbeddedFont, EmbeddedFontList, FontCharset, FontStyle};
pub use ink_annotations::{InkAnnotations, InkPen, InkPoint, InkStroke, PenTip};
pub use layout::SlideLayout;
pub use presentation_settings::PresentationSettings;
pub use print_settings::{HandoutLayout, Orientation, PrintColorMode, PrintSettings, PrintWhat};
pub use sections::{SectionManager, SlideSection};
pub use slide_show_settings::{PenColor, ShowType, SlideRange, SlideShowSettings};
pub use table_merge::{CellMergeState, MergeRegion, TableMergeMap};
pub use transition::TransitionType;
