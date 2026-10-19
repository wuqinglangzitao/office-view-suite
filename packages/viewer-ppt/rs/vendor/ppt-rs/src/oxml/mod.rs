//! OXML (Office XML) element handling

pub mod comments;
pub mod editor;
pub mod ns;
pub mod presentation;
pub mod repair;
pub mod slide;
pub mod text_editor;
pub mod xmlchemy;

pub use editor::PresentationEditor;
pub use ns::Namespace;
pub use presentation::{PresentationInfo, PresentationReader, extract_slide_texts_from_bytes};
pub use repair::{PptxRepair, RepairIssue, RepairResult};
pub use slide::{ParsedSlide, SlideParser};
pub use text_editor::ParagraphEdit;
pub use xmlchemy::{XmlElement, XmlParser};
