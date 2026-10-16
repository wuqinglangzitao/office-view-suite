//! PPTX file generator - creates proper ZIP-based PPTX files

// Layout constants (shared positioning and sizing values)
pub mod constants;

// Core XML generation modules
pub mod package_xml;
pub mod presentation_theme;
pub mod props_xml;
pub mod slide_content;
pub mod slide_xml;
pub mod theme_xml;

pub mod layout_parts;
pub mod template;

// Modular layout system
pub mod layouts;

// Builder and content modules
pub mod builder;
pub mod media_registry;
pub mod memory_profile;
pub mod package_cache;

// Text module (modularized)
pub mod text;

pub mod shapes;
pub mod shapes_xml;

// Table module (modularized)
pub mod table;

pub mod image_effects;
pub mod images;
pub mod images_xml;

// Charts module (modularized)
#[path = "charts/mod.rs"]
pub mod charts;

// Notes module for speaker notes
pub mod notes_xml;

// Slide utilities (formatting, etc.)
pub mod slide;

// New element modules
pub mod connectors;
pub mod gradients;
pub mod hyperlinks;
pub mod media;

pub use builder::{
    LazySlideSource, create_pptx, create_pptx_lazy_to_writer, create_pptx_to_writer,
    create_pptx_with_content, create_pptx_with_content_to_writer, create_pptx_with_settings,
    create_pptx_with_template,
};
pub use charts::{
    Chart, ChartBuilder, ChartSeries, ChartType, generate_chart_part_xml, generate_chart_ref_xml,
};
pub use images::{Image, ImageBuilder, ImageSource};
pub use images_xml::{
    generate_image_content_type, generate_image_relationship, generate_image_xml,
};
pub use layout_parts::STANDARD_LAYOUT_COUNT;
pub use notes_xml::{
    create_notes_master_rels_xml, create_notes_master_xml, create_notes_rels_xml, create_notes_xml,
};
pub use presentation_theme::{PresentationTheme, ThemeColorScheme, ThemeFonts};
pub use shapes::{
    FillType, GradientDirection as ShapeGradientDirection, GradientFill as ShapeGradientFill,
    GradientStop as ShapeGradientStop, Shape, ShapeFill, ShapeLine, ShapeType, cm_to_emu,
    emu_to_inches, inches_to_emu,
};
pub use shapes_xml::{generate_connector_xml, generate_shape_xml, generate_shapes_xml};
pub use slide_content::{
    BulletPoint, BulletStyle, BulletTextFormat, CellMergeState, CodeBlock, Comment, CommentAuthor,
    CommentAuthorList, DigitalSignature, EmbeddedFont, EmbeddedFontList, FontCharset, FontStyle,
    HandoutLayout, HashAlgorithm, InkAnnotations, InkPen, InkPoint, InkStroke, MergeRegion,
    Orientation, PenColor, PenTip, PresentationSettings, PrintColorMode, PrintSettings, PrintWhat,
    SectionManager, ShowType, SignatureCommitment, SignerInfo, SlideComments, SlideRange,
    SlideSection, SlideShowSettings, TableMergeMap, TransitionType,
};
pub use slide_content::{SlideContent, SlideLayout};
pub use table::{
    CellAlign, CellVAlign, Table, TableBuilder, TableCell, TableRow,
    generate_table_xml as generate_table_xml_new,
};
pub use template::PptxTemplate;
pub use text::{
    FormattedText, Paragraph, RtlLanguage, RtlTextProps, Run, TextAlign, TextAnchor, TextDirection,
    TextFormat, TextFrame,
};

// New element exports
pub use connectors::{
    ArrowSize, ArrowType, ConnectionSite, Connector, ConnectorLine, ConnectorType, LineDash,
    generate_connector_xml as generate_cxn_xml,
};
pub use gradients::{
    GradientDirection, GradientFill, GradientStop, GradientType, PresetGradients,
    generate_gradient_fill_xml,
};
pub use hyperlinks::{
    Hyperlink, HyperlinkAction, generate_hyperlink_relationship_xml, generate_shape_hyperlink_xml,
    generate_text_hyperlink_xml,
};
pub use media::{
    Audio, AudioFormat, AudioOptions, MediaRelIds, Video, VideoFormat, VideoOptions,
    generate_audio_xml, generate_video_xml,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slide_content_builder() {
        let slide = SlideContent::new("Title")
            .add_bullet("Point 1")
            .add_bullet("Point 2");

        assert_eq!(slide.title, "Title");
        assert_eq!(slide.content.len(), 2);
        assert_eq!(slide.content[0], "Point 1");
    }

    #[test]
    fn test_bullet_styles() {
        let bullet = BulletStyle::Bullet;
        assert!(bullet.to_xml().contains("buChar"));

        let number = BulletStyle::Number;
        assert!(number.to_xml().contains("arabicPeriod"));

        let letter = BulletStyle::LetterLower;
        assert!(letter.to_xml().contains("alphaLcPeriod"));

        let roman = BulletStyle::RomanUpper;
        assert!(roman.to_xml().contains("romanUcPeriod"));

        let custom = BulletStyle::Custom('★');
        assert!(custom.to_xml().contains("★"));

        let none = BulletStyle::None;
        assert!(none.to_xml().contains("buNone"));
    }

    #[test]
    fn test_numbered_slide() {
        let slide = SlideContent::new("Steps")
            .with_bullet_style(BulletStyle::Number)
            .add_bullet("First step")
            .add_bullet("Second step")
            .add_bullet("Third step");

        assert_eq!(slide.bullets.len(), 3);
        assert_eq!(slide.bullets[0].style, BulletStyle::Number);
    }

    #[test]
    fn test_mixed_bullet_styles() {
        let slide = SlideContent::new("Mixed")
            .add_numbered("Step 1")
            .add_numbered("Step 2")
            .add_lettered("Option a")
            .add_lettered("Option b");

        assert_eq!(slide.bullets.len(), 4);
        assert_eq!(slide.bullets[0].style, BulletStyle::Number);
        assert_eq!(slide.bullets[2].style, BulletStyle::LetterLower);
    }

    #[test]
    fn test_sub_bullets() {
        let slide = SlideContent::new("Hierarchy")
            .add_bullet("Main point")
            .add_sub_bullet("Sub point 1")
            .add_sub_bullet("Sub point 2")
            .add_bullet("Another main point");

        assert_eq!(slide.bullets.len(), 4);
        assert_eq!(slide.bullets[0].level, 0);
        assert_eq!(slide.bullets[1].level, 1);
        assert_eq!(slide.bullets[2].level, 1);
        assert_eq!(slide.bullets[3].level, 0);
    }
}
