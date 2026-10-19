//! Semantic comparison of presentations for review and CI.
//!
//! Compares slide-level content (title, bullets, images, charts, tables, media)
//! without requiring byte-identical OOXML.

use crate::api::Presentation;
use crate::exc::Result;
use crate::generator::SlideContent;

/// Semantic snapshot of one slide's review-relevant content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideSemantic {
    pub title: String,
    pub bullets: Vec<String>,
    pub image_count: usize,
    pub image_alt_texts: Vec<String>,
    pub chart_summaries: Vec<String>,
    pub table_rows: usize,
    pub table_cols: usize,
    pub video_count: usize,
    pub audio_count: usize,
}

/// A single field-level change on a slide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldChange {
    Title {
        before: String,
        after: String,
    },
    Bullets {
        before: Vec<String>,
        after: Vec<String>,
    },
    Images {
        before_count: usize,
        after_count: usize,
        before_alt_texts: Vec<String>,
        after_alt_texts: Vec<String>,
    },
    Charts {
        before: Vec<String>,
        after: Vec<String>,
    },
    Table {
        before_rows: usize,
        before_cols: usize,
        after_rows: usize,
        after_cols: usize,
    },
    Media {
        before_videos: usize,
        before_audios: usize,
        after_videos: usize,
        after_audios: usize,
    },
}

/// Slide-level addition, removal, or modification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlideChange {
    Added {
        index: usize,
        slide: SlideSemantic,
    },
    Removed {
        index: usize,
        slide: SlideSemantic,
    },
    Modified {
        index: usize,
        before: SlideSemantic,
        after: SlideSemantic,
        fields: Vec<FieldChange>,
    },
}

/// Semantic diff between two presentations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresentationDiff {
    pub title_changed: Option<(String, String)>,
    pub slide_changes: Vec<SlideChange>,
}

impl PresentationDiff {
    /// True when titles and every slide match semantically.
    pub fn is_equal(&self) -> bool {
        self.title_changed.is_none() && self.slide_changes.is_empty()
    }

    /// One-line human summary for CLI output.
    pub fn summary(&self) -> String {
        if self.is_equal() {
            return "No semantic differences".to_string();
        }
        let mut parts = Vec::new();
        if self.title_changed.is_some() {
            parts.push("title changed".to_string());
        }
        let added = self
            .slide_changes
            .iter()
            .filter(|c| matches!(c, SlideChange::Added { .. }))
            .count();
        let removed = self
            .slide_changes
            .iter()
            .filter(|c| matches!(c, SlideChange::Removed { .. }))
            .count();
        let modified = self
            .slide_changes
            .iter()
            .filter(|c| matches!(c, SlideChange::Modified { .. }))
            .count();
        if added > 0 {
            parts.push(format!("{added} slide(s) added"));
        }
        if removed > 0 {
            parts.push(format!("{removed} slide(s) removed"));
        }
        if modified > 0 {
            parts.push(format!("{modified} slide(s) modified"));
        }
        parts.join(", ")
    }
}

/// Build a semantic snapshot from generator slide content.
pub fn slide_semantic(slide: &SlideContent) -> SlideSemantic {
    let mut image_alt_texts: Vec<String> = slide
        .images
        .iter()
        .filter_map(|img| img.alt_text.as_deref().map(str::to_string))
        .collect();
    image_alt_texts.sort();

    let mut chart_summaries: Vec<String> = slide
        .charts
        .iter()
        .map(|c| {
            format!(
                "{:?}:{} ({} categories, {} series)",
                c.chart_type,
                c.title,
                c.categories.len(),
                c.series.len()
            )
        })
        .collect();
    chart_summaries.sort();

    let (table_rows, table_cols) = slide
        .table
        .as_ref()
        .map(|t| {
            (
                t.rows.len(),
                t.rows.first().map(|r| r.cells.len()).unwrap_or(0),
            )
        })
        .unwrap_or((0, 0));

    SlideSemantic {
        title: slide.title.clone(),
        bullets: slide.content.clone(),
        image_count: slide.images.len(),
        image_alt_texts,
        chart_summaries,
        table_rows,
        table_cols,
        video_count: slide.videos.len(),
        audio_count: slide.audios.len(),
    }
}

/// Compare two in-memory presentations.
pub fn compare_presentations(a: &Presentation, b: &Presentation) -> PresentationDiff {
    let title_changed = if a.get_title() == b.get_title() {
        None
    } else {
        Some((a.get_title().to_string(), b.get_title().to_string()))
    };

    let slides_a: Vec<SlideSemantic> = a.slides().iter().map(slide_semantic).collect();
    let slides_b: Vec<SlideSemantic> = b.slides().iter().map(slide_semantic).collect();

    let max_len = slides_a.len().max(slides_b.len());
    let mut slide_changes = Vec::new();

    for i in 0..max_len {
        match (slides_a.get(i), slides_b.get(i)) {
            (Some(before), Some(after)) => {
                if before != after {
                    slide_changes.push(SlideChange::Modified {
                        index: i,
                        before: before.clone(),
                        after: after.clone(),
                        fields: diff_slide_fields(before, after),
                    });
                }
            }
            (Some(before), None) => {
                slide_changes.push(SlideChange::Removed {
                    index: i,
                    slide: before.clone(),
                });
            }
            (None, Some(after)) => {
                slide_changes.push(SlideChange::Added {
                    index: i,
                    slide: after.clone(),
                });
            }
            (None, None) => {}
        }
    }

    PresentationDiff {
        title_changed,
        slide_changes,
    }
}

/// Load two PPTX files and compare them semantically.
pub fn compare_pptx_paths(old_path: &str, new_path: &str) -> Result<PresentationDiff> {
    let a = Presentation::from_path(old_path)?;
    let b = Presentation::from_path(new_path)?;
    Ok(compare_presentations(&a, &b))
}

fn diff_slide_fields(before: &SlideSemantic, after: &SlideSemantic) -> Vec<FieldChange> {
    let mut fields = Vec::new();
    if before.title != after.title {
        fields.push(FieldChange::Title {
            before: before.title.clone(),
            after: after.title.clone(),
        });
    }
    if before.bullets != after.bullets {
        fields.push(FieldChange::Bullets {
            before: before.bullets.clone(),
            after: after.bullets.clone(),
        });
    }
    if before.image_count != after.image_count || before.image_alt_texts != after.image_alt_texts {
        fields.push(FieldChange::Images {
            before_count: before.image_count,
            after_count: after.image_count,
            before_alt_texts: before.image_alt_texts.clone(),
            after_alt_texts: after.image_alt_texts.clone(),
        });
    }
    if before.chart_summaries != after.chart_summaries {
        fields.push(FieldChange::Charts {
            before: before.chart_summaries.clone(),
            after: after.chart_summaries.clone(),
        });
    }
    if before.table_rows != after.table_rows || before.table_cols != after.table_cols {
        fields.push(FieldChange::Table {
            before_rows: before.table_rows,
            before_cols: before.table_cols,
            after_rows: after.table_rows,
            after_cols: after.table_cols,
        });
    }
    if before.video_count != after.video_count || before.audio_count != after.audio_count {
        fields.push(FieldChange::Media {
            before_videos: before.video_count,
            before_audios: before.audio_count,
            after_videos: after.video_count,
            after_audios: after.audio_count,
        });
    }
    fields
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::SlideContent;
    use crate::generator::charts::{Chart, ChartSeries, ChartType};
    use crate::generator::images::Image;
    use crate::helpers::tables::simple_table;

    fn deck(title: &str, slides: Vec<SlideContent>) -> Presentation {
        let mut pres = Presentation::with_title(title);
        for slide in slides {
            pres = pres.add_slide(slide);
        }
        pres
    }

    #[test]
    fn identical_presentations_have_no_diff() {
        let a = deck(
            "Same",
            vec![
                SlideContent::new("A").add_bullet("one"),
                SlideContent::new("B").add_bullet("two"),
            ],
        );
        let b = deck(
            "Same",
            vec![
                SlideContent::new("A").add_bullet("one"),
                SlideContent::new("B").add_bullet("two"),
            ],
        );
        assert!(compare_presentations(&a, &b).is_equal());
    }

    #[test]
    fn detects_title_and_bullet_changes() {
        let a = deck(
            "Old Title",
            vec![SlideContent::new("Slide").add_bullet("alpha")],
        );
        let b = deck(
            "New Title",
            vec![SlideContent::new("Slide").add_bullet("beta")],
        );
        let diff = compare_presentations(&a, &b);
        assert!(!diff.is_equal());
        assert_eq!(
            diff.title_changed,
            Some(("Old Title".into(), "New Title".into()))
        );
        assert_eq!(diff.slide_changes.len(), 1);
        assert!(matches!(
            &diff.slide_changes[0],
            SlideChange::Modified { index: 0, fields, .. }
            if fields.iter().any(|f| matches!(f, FieldChange::Bullets { .. }))
        ));
    }

    #[test]
    fn detects_added_and_removed_slides() {
        let a = deck("T", vec![SlideContent::new("Only")]);
        let b = deck(
            "T",
            vec![SlideContent::new("First"), SlideContent::new("Second")],
        );
        let diff = compare_presentations(&a, &b);
        assert!(
            diff.slide_changes
                .iter()
                .any(|c| matches!(c, SlideChange::Modified { index: 0, .. }))
        );
        assert!(
            diff.slide_changes
                .iter()
                .any(|c| matches!(c, SlideChange::Added { index: 1, .. }))
        );
    }

    #[test]
    fn detects_chart_and_image_differences() {
        let chart = Chart::new("Sales", ChartType::Bar, vec!["Q1".into()], 0, 0, 1, 1)
            .add_series(ChartSeries::new("Revenue", vec![1.0]));
        let a = deck(
            "T",
            vec![SlideContent::new("Charts").add_chart(chart.clone())],
        );
        let img = Image::new("logo.png", 100, 100, "PNG").with_alt_text("logo");
        let b = deck(
            "T",
            vec![SlideContent::new("Charts").add_image(img).add_chart(chart)],
        );
        let diff = compare_presentations(&a, &b);
        let modified = diff
            .slide_changes
            .iter()
            .find_map(|c| match c {
                SlideChange::Modified { fields, .. } => Some(fields),
                _ => None,
            })
            .expect("slide modified");
        assert!(
            modified
                .iter()
                .any(|f| matches!(f, FieldChange::Images { .. }))
        );
    }

    #[test]
    fn detects_table_shape_change() {
        let table = simple_table(2, 1).build();
        let a = deck("T", vec![SlideContent::new("Table")]);
        let b = deck("T", vec![SlideContent::new("Table").table(table)]);
        let diff = compare_presentations(&a, &b);
        let fields = match &diff.slide_changes[0] {
            SlideChange::Modified { fields, .. } => fields,
            other => panic!("expected modified, got {other:?}"),
        };
        assert!(
            fields
                .iter()
                .any(|f| matches!(f, FieldChange::Table { .. }))
        );
    }

    #[test]
    fn compare_pptx_paths_round_trip() {
        let path = "test_semantic_diff_roundtrip.pptx";
        deck(
            "Export",
            vec![SlideContent::new("Slide 1").add_bullet("Point")],
        )
        .save(path)
        .unwrap();
        let diff = compare_pptx_paths(path, path).unwrap();
        assert!(diff.is_equal());
        let _ = std::fs::remove_file(path);
    }
}
