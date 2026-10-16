//! Validates the in-repo public `.docx` corpus and generated legacy `.doc` benchmark
//! through rwml's public API — a license-clean gate anyone can run with `cargo test`.
//!
//! For each `.docx` it asserts the editor's core contract holds on a real/feature-rich file:
//!   1. `Document::open` succeeds;
//!   2. a no-op `open -> save` round-trips and **re-opens** (and is idempotent on a second
//!      save — the serialization is stable);
//!   3. `add_image_png` produces a package that re-opens with rwml.
//!
//! Per-*part* byte-stability (the stronger "unmodeled content preserved" claim) is covered by
//! the crate's unit tests and by `examples/validate_edit.rs` + `scripts/validate_edit_check.py`
//! against the larger private corpus; this test keeps the public gate self-contained.
#![cfg(feature = "docx")]

use std::fs;
use std::path::{Path, PathBuf};

#[cfg(feature = "render")]
use rwml::{Block, CellMargins};
use rwml::{Document, DocumentWarning};

/// A genuinely valid 2x3 RGB PNG (correct chunk CRCs + a real zlib IDAT), so
/// `add_image_png`'s CRC-checked PNG validation accepts it.
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x08, 0x02, 0x00, 0x00, 0x00, 0x36, 0x88, 0x49,
    0xD6, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0x60, 0xC0, 0x02, 0x00,
    0x00, 0x15, 0x00, 0x01, 0x39, 0xC1, 0xE0, 0x23, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44,
    0xAE, 0x42, 0x60, 0x82,
];

fn collect_docx(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_docx(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("docx") {
            out.push(p);
        }
    }
}

#[test]
fn public_corpus_opens_roundtrips_and_edits() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/public");
    let mut files = Vec::new();
    collect_docx(&root, &mut files);
    files.sort();

    assert!(
        files.len() >= 5,
        "expected the public corpus to hold >=5 .docx (found {}); is corpus/public present?",
        files.len()
    );

    for path in &files {
        let label = path.strip_prefix(&root).unwrap_or(path).display();
        let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {label}: {e}"));

        // 1. opens
        let doc = Document::open(&bytes).unwrap_or_else(|e| panic!("open {label}: {e}"));

        // 2. no-op save re-opens, and a second save is byte-identical (stable serialization)
        let saved = doc.save().unwrap_or_else(|e| panic!("save {label}: {e}"));
        let reopened = Document::open(&saved).unwrap_or_else(|e| panic!("reopen {label}: {e}"));
        let saved2 = reopened
            .save()
            .unwrap_or_else(|e| panic!("re-save {label}: {e}"));
        assert_eq!(saved, saved2, "no-op save is not idempotent for {label}");

        // 3. add_image_png yields a package that re-opens
        let mut edit = Document::open(&bytes).unwrap();
        edit.add_image_png(TINY_PNG, "rwml_public_corpus_test.png")
            .unwrap_or_else(|e| panic!("add_image_png {label}: {e}"));
        let edited = edit
            .save()
            .unwrap_or_else(|e| panic!("save edited {label}: {e}"));
        Document::open(&edited).unwrap_or_else(|e| panic!("reopen edited {label}: {e}"));
    }
}

#[derive(Debug)]
struct ExpectedReport {
    path: String,
    comments: usize,
    footnotes: usize,
    endnotes: usize,
    text_boxes: usize,
    fields: usize,
    tracked_insertions: usize,
    tracked_deletions: usize,
    tracked_moves: usize,
    tracked_property_changes: usize,
    content_controls: usize,
    hyperlinks: usize,
    nested_tables: usize,
    floating_shapes: usize,
    charts: usize,
    ole_objects: usize,
    unsupported_metafiles: usize,
    warnings: Vec<String>,
}

fn parse_expected_reports(manifest: &str) -> Vec<ExpectedReport> {
    manifest
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("path\t")
        })
        .map(|line| {
            let cols: Vec<_> = line.split('\t').collect();
            assert_eq!(cols.len(), 18, "bad manifest row: {line}");
            let parse = |i: usize| {
                cols[i]
                    .parse::<usize>()
                    .unwrap_or_else(|e| panic!("bad numeric column {i} in {line}: {e}"))
            };
            let warnings = if cols[17] == "-" {
                Vec::new()
            } else {
                cols[17].split('|').map(str::to_owned).collect()
            };
            ExpectedReport {
                path: cols[0].to_owned(),
                comments: parse(1),
                footnotes: parse(2),
                endnotes: parse(3),
                text_boxes: parse(4),
                fields: parse(5),
                tracked_insertions: parse(6),
                tracked_deletions: parse(7),
                tracked_moves: parse(8),
                tracked_property_changes: parse(9),
                content_controls: parse(10),
                hyperlinks: parse(11),
                nested_tables: parse(12),
                floating_shapes: parse(13),
                charts: parse(14),
                ole_objects: parse(15),
                unsupported_metafiles: parse(16),
                warnings,
            }
        })
        .collect()
}

#[test]
fn public_report_manifest_accounts_for_every_docx_fixture() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/public");
    let manifest = fs::read_to_string(root.join("MANIFEST.tsv")).expect("read corpus manifest");
    let expected = parse_expected_reports(&manifest);

    let mut manifested: Vec<_> = expected.iter().map(|row| root.join(&row.path)).collect();
    manifested.sort();
    manifested.dedup();

    let mut discovered = Vec::new();
    collect_docx(&root, &mut discovered);
    discovered.sort();

    assert_eq!(
        manifested.len(),
        expected.len(),
        "manifest paths must be unique"
    );
    assert_eq!(
        manifested, discovered,
        "MANIFEST.tsv must account for every committed public .docx fixture"
    );
}

fn warning_name(warning: &DocumentWarning) -> &'static str {
    match warning {
        DocumentWarning::UnsupportedFieldEvaluation { .. } => "UnsupportedFieldEvaluation",
        DocumentWarning::TrackedChangesPresent { .. } => "TrackedChangesPresent",
        DocumentWarning::IncompleteRevisionView { .. } => "IncompleteRevisionView",
        DocumentWarning::FloatingShapePlaceholderOnly { .. } => "FloatingShapePlaceholderOnly",
        DocumentWarning::ChartsPreservedButNotModeled { .. } => "ChartsPreservedButNotModeled",
        DocumentWarning::OleObjectsPreservedButNotModeled { .. } => {
            "OleObjectsPreservedButNotModeled"
        }
        DocumentWarning::UnsupportedMetafileImages { .. } => "UnsupportedMetafileImages",
        DocumentWarning::LegacyDocFlattenedSubdocuments { .. } => "LegacyDocFlattenedSubdocuments",
        DocumentWarning::PackageReadOnly { .. } => "PackageReadOnly",
    }
}

#[test]
fn public_corpus_report_matches_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/public");
    let manifest = fs::read_to_string(root.join("MANIFEST.tsv")).expect("read corpus manifest");
    let expected = parse_expected_reports(&manifest);
    assert!(
        expected.len() >= 5,
        "expected the report manifest to describe the synthetic feature corpus"
    );

    for row in expected {
        let path = root.join(&row.path);
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", row.path));
        let doc = Document::open(&bytes).unwrap_or_else(|e| panic!("open {}: {e}", row.path));
        let report = doc.report();
        let features = report.features;

        assert_eq!(features.comments, row.comments, "comments in {}", row.path);
        assert_eq!(
            features.footnotes, row.footnotes,
            "footnotes in {}",
            row.path
        );
        assert_eq!(features.endnotes, row.endnotes, "endnotes in {}", row.path);
        assert_eq!(
            features.text_boxes, row.text_boxes,
            "text boxes in {}",
            row.path
        );
        assert_eq!(features.fields, row.fields, "fields in {}", row.path);
        assert_eq!(
            features.tracked_insertions, row.tracked_insertions,
            "insertions in {}",
            row.path
        );
        assert_eq!(
            features.tracked_deletions, row.tracked_deletions,
            "deletions in {}",
            row.path
        );
        assert_eq!(
            features.tracked_moves, row.tracked_moves,
            "moves in {}",
            row.path
        );
        assert_eq!(
            features.tracked_property_changes, row.tracked_property_changes,
            "property changes in {}",
            row.path
        );
        assert_eq!(
            features.content_controls, row.content_controls,
            "content controls in {}",
            row.path
        );
        assert_eq!(
            features.hyperlinks, row.hyperlinks,
            "hyperlinks in {}",
            row.path
        );
        assert_eq!(
            features.nested_tables, row.nested_tables,
            "nested tables in {}",
            row.path
        );
        assert_eq!(
            features.floating_shapes, row.floating_shapes,
            "floating shapes in {}",
            row.path
        );
        assert_eq!(features.charts, row.charts, "charts in {}", row.path);
        assert_eq!(
            features.ole_objects, row.ole_objects,
            "OLE objects in {}",
            row.path
        );
        assert_eq!(
            features.unsupported_metafiles, row.unsupported_metafiles,
            "unsupported metafiles in {}",
            row.path
        );

        let mut warnings: Vec<_> = report
            .warnings
            .iter()
            .map(|warning| warning_name(warning).to_owned())
            .collect();
        warnings.sort();
        let mut expected_warnings = row.warnings;
        expected_warnings.sort();
        assert_eq!(warnings, expected_warnings, "warnings in {}", row.path);
    }
}

#[derive(Debug)]
struct ExpectedLegacyReport {
    path: String,
    paragraphs: u32,
    tables: u16,
    figures: u16,
    text_chars: usize,
    warnings: Vec<String>,
}

fn parse_expected_legacy_reports(manifest: &str) -> Vec<ExpectedLegacyReport> {
    manifest
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("path\t")
        })
        .map(|line| {
            let cols: Vec<_> = line.split('\t').collect();
            assert_eq!(cols.len(), 6, "bad legacy manifest row: {line}");
            let parse = |i: usize| {
                cols[i]
                    .parse::<usize>()
                    .unwrap_or_else(|e| panic!("bad numeric column {i} in {line}: {e}"))
            };
            let warnings = if cols[5] == "-" {
                Vec::new()
            } else {
                cols[5].split('|').map(str::to_owned).collect()
            };
            ExpectedLegacyReport {
                path: cols[0].to_owned(),
                paragraphs: parse(1).try_into().expect("paragraph count fits u32"),
                tables: parse(2).try_into().expect("table count fits u16"),
                figures: parse(3).try_into().expect("figure count fits u16"),
                text_chars: parse(4),
                warnings,
            }
        })
        .collect()
}

#[test]
fn public_legacy_corpus_matches_manifest_and_has_mature_goldens() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/public/benchmark");
    let manifest =
        fs::read_to_string(root.join("LEGACY_MANIFEST.tsv")).expect("read legacy manifest");
    let expected = parse_expected_legacy_reports(&manifest);
    assert!(
        expected.len() >= 3,
        "expected at least three generated legacy fixtures"
    );

    let mut manifested = Vec::new();
    for row in expected {
        let path = root.join(&row.path);
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", row.path));
        let doc = Document::open(&bytes).unwrap_or_else(|e| panic!("open {}: {e}", row.path));
        let report = doc.report();
        assert_eq!(
            report.format,
            rwml::DocumentFormat::Doc,
            "format in {}",
            row.path
        );
        assert_eq!(
            report.stats.paragraphs, row.paragraphs,
            "paragraphs in {}",
            row.path
        );
        assert_eq!(report.stats.tables, row.tables, "tables in {}", row.path);
        assert_eq!(report.stats.figures, row.figures, "figures in {}", row.path);
        assert_eq!(
            report.stats.text_chars, row.text_chars,
            "text chars in {}",
            row.path
        );

        let mut warnings: Vec<_> = report
            .warnings
            .iter()
            .map(|warning| warning_name(warning).to_owned())
            .collect();
        warnings.sort();
        let mut expected_warnings = row.warnings;
        expected_warnings.sort();
        assert_eq!(warnings, expected_warnings, "warnings in {}", row.path);

        let stem = path
            .file_stem()
            .and_then(|name| name.to_str())
            .expect("ASCII fixture stem");
        for golden in [
            root.join("sample-poi").join(format!("{stem}.poi.txt")),
            root.join("sample-lo").join(format!("{stem}.txt")),
        ] {
            let text = fs::read_to_string(&golden)
                .unwrap_or_else(|e| panic!("read mature golden {}: {e}", golden.display()));
            assert!(
                !text.trim().is_empty(),
                "empty mature golden {}",
                golden.display()
            );
        }
        manifested.push(path);
    }

    let mut discovered = Vec::new();
    for entry in fs::read_dir(root.join("sample")).expect("read legacy sample directory") {
        let path = entry.expect("read legacy sample entry").path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("doc") {
            discovered.push(path);
        }
    }
    manifested.sort();
    discovered.sort();
    assert_eq!(
        discovered, manifested,
        "legacy manifest must account for every .doc"
    );
}

#[cfg(feature = "render")]
#[derive(Debug)]
struct ExpectedRenderReport {
    path: String,
    pages: usize,
    warnings: Vec<String>,
}

#[cfg(feature = "render")]
fn parse_expected_render_reports(manifest: &str) -> Vec<ExpectedRenderReport> {
    manifest
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("path\t")
        })
        .map(|line| {
            let cols: Vec<_> = line.split('\t').collect();
            assert_eq!(cols.len(), 3, "bad render manifest row: {line}");
            let warnings = if cols[2] == "-" {
                Vec::new()
            } else {
                cols[2].split('|').map(str::to_owned).collect()
            };
            ExpectedRenderReport {
                path: cols[0].to_owned(),
                pages: cols[1]
                    .parse()
                    .unwrap_or_else(|e| panic!("bad page count in {line}: {e}")),
                warnings,
            }
        })
        .collect()
}

#[cfg(feature = "render")]
fn render_warning_name(warning: &rwml::RenderWarning) -> &'static str {
    match warning {
        rwml::RenderWarning::UnsupportedFieldEvaluation { .. } => "UnsupportedFieldEvaluation",
        rwml::RenderWarning::FloatingShapePlaceholderOnly { .. } => "FloatingShapePlaceholderOnly",
        rwml::RenderWarning::ChartsPreservedButNotModeled { .. } => "ChartsPreservedButNotModeled",
        rwml::RenderWarning::OleObjectsPreservedButNotModeled { .. } => {
            "OleObjectsPreservedButNotModeled"
        }
        rwml::RenderWarning::UnsupportedMetafileImages { .. } => "UnsupportedMetafileImages",
        rwml::RenderWarning::MissingImageBytes { .. } => "MissingImageBytes",
        rwml::RenderWarning::UndecodableRasterImages { .. } => "UndecodableRasterImages",
    }
}

#[cfg(feature = "render")]
fn expected_render_unsupported(mut features: rwml::FeatureInventory) -> rwml::FeatureInventory {
    features.field_kinds = features.unsupported_field_kinds.clone();
    features.fields = features.field_kinds.iter().map(|item| item.count).sum();
    features
}

#[cfg(feature = "render")]
fn replace_cell_list_labels(blocks: &mut [Block], next: &mut usize) {
    for block in blocks {
        match block {
            Block::Paragraph(paragraph) => {
                if let Some(list) = &mut paragraph.props.list {
                    *next += 1;
                    list.label = format!("changed-cell-marker-{next}");
                }
            }
            Block::Table(table) => {
                for row in &mut table.rows {
                    for cell in &mut row.cells {
                        replace_cell_list_labels(&mut cell.blocks, next);
                    }
                }
            }
            Block::Image(_) | Block::Chart(_) | Block::PageBreak | Block::SectionBreak(_) => {}
        }
    }
}

#[cfg(feature = "render")]
#[test]
fn public_corpus_render_report_matches_manifest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/public");
    let manifest =
        fs::read_to_string(root.join("RENDER_MANIFEST.tsv")).expect("read render manifest");
    let expected = parse_expected_render_reports(&manifest);
    assert!(
        expected.len() >= 5,
        "expected the render manifest to describe the synthetic feature corpus"
    );

    for row in expected {
        let path = root.join(&row.path);
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", row.path));
        let doc = Document::open(&bytes).unwrap_or_else(|e| panic!("open {}: {e}", row.path));
        let features = doc.report().features;
        let rendered = doc
            .try_to_pdf_with_report()
            .unwrap_or_else(|e| panic!("render {}: {e}", row.path));

        assert!(
            rendered.pdf.starts_with(b"%PDF") && rendered.pdf.len() > 800,
            "invalid or tiny PDF for {}",
            row.path
        );
        assert_eq!(rendered.report.pages, row.pages, "pages in {}", row.path);
        assert_eq!(
            rendered.report.unsupported,
            expected_render_unsupported(features),
            "render unsupported inventory in {}",
            row.path
        );

        let mut warnings: Vec<_> = rendered
            .report
            .warnings
            .iter()
            .map(|warning| render_warning_name(warning).to_owned())
            .collect();
        warnings.sort();
        let mut expected_warnings = row.warnings;
        expected_warnings.sort();
        assert_eq!(
            warnings, expected_warnings,
            "render warnings in {}",
            row.path
        );
    }
}

#[cfg(feature = "render")]
#[test]
fn render_activation_fixtures_preserve_opened_document_semantics() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/public/synthetic");
    let open = |name: &str| {
        let path = root.join(name);
        let bytes = fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        Document::open(&bytes).unwrap_or_else(|e| panic!("open {}: {e}", path.display()))
    };

    let style = open("style-hidden-tabs-table.docx");
    let source_text = style.text();
    let before = source_text.find("VISIBLE BEFORE").expect("visible prefix");
    let hidden = source_text
        .find("SECRET_TOKEN")
        .expect("hidden source text");
    let after = source_text.find("VISIBLE AFTER").expect("visible suffix");
    assert!(
        before < hidden && hidden < after,
        "source order must remain stable"
    );

    let style_model = style.model();
    let hidden_runs: Vec<_> = style_model
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Paragraph(paragraph) => Some(&paragraph.runs),
            _ => None,
        })
        .flatten()
        .filter(|run| run.props.hidden)
        .collect();
    assert_eq!(hidden_runs.len(), 1);
    assert_eq!(hidden_runs[0].text, "SECRET_TOKEN");
    let table = style_model
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("style fixture table");
    assert!(table.bidi_visual);
    assert_eq!(table.rows[0].cells[0].text(), "LOGICAL LEFT");
    assert_eq!(table.rows[0].cells[1].text(), "LOGICAL RIGHT");
    assert!(table.rows[0].cells.iter().all(|cell| {
        cell.margins
            == Some(CellMargins {
                top: 180,
                right: 360,
                bottom: 180,
                left: 360,
            })
    }));

    let fonts = vec![
        rwml_fonts::noto_sans_kr_subset_with_hanja().to_vec(),
        rwml_fonts::noto_sans_arabic_subset().to_vec(),
        rwml_fonts::noto_sans_hebrew_subset().to_vec(),
    ];
    let tabbed_pdf = style
        .try_to_pdf_with_fonts(&fonts)
        .expect("opened-document tab render");
    assert!(!tabbed_pdf.is_empty());
    assert_eq!(
        tabbed_pdf,
        style
            .try_to_pdf_with_fonts(&fonts)
            .expect("repeat opened-document tab render")
    );
    assert_ne!(
        tabbed_pdf,
        rwml::try_render_pdf_with_fonts(&style_model, &fonts)
            .expect("model-only default-tab render"),
        "opened DOCX tab stops must affect rendering beyond the public DocModel"
    );

    let pagination_doc = open("pagination-keep.docx");
    let pagination = pagination_doc
        .layout_pages_with_fonts(&fonts)
        .expect("fixed-font pagination layout");
    assert_eq!(pagination.pages, 3);
    assert_eq!(pagination.block_pages.len(), 8);
    assert!(pagination.block_pages.iter().all(Option::is_some));
    assert_eq!(pagination.block_pages[0], Some(1));
    assert_eq!(pagination.block_pages[1], Some(1));
    assert!(pagination
        .block_pages
        .windows(2)
        .all(|pages| pages[0] <= pages[1]));
    assert_eq!(
        pagination,
        pagination_doc
            .layout_pages_with_fonts(&fonts)
            .expect("repeat fixed-font pagination layout")
    );

    let columns_doc = open("two-columns.docx");
    let columns = columns_doc
        .layout_pages_with_fonts(&fonts)
        .expect("fixed-font column layout");
    assert_eq!(columns.pages, 2);
    assert_eq!(columns.block_pages.first(), Some(&Some(1)));
    assert_eq!(columns.block_pages.last(), Some(&Some(2)));
    assert!(columns.block_pages.contains(&Some(1)));
    assert!(columns.block_pages.contains(&Some(2)));
    assert!(columns
        .block_pages
        .windows(2)
        .all(|pages| pages[0] <= pages[1]));
    assert_eq!(
        columns,
        columns_doc
            .layout_pages_with_fonts(&fonts)
            .expect("repeat fixed-font column layout")
    );

    let rtl = open("rtl-table.docx");
    let rtl_model = rtl.model();
    let rtl_table = rtl_model
        .blocks
        .iter()
        .find_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .expect("RTL fixture table");
    assert!(rtl_table.bidi_visual);
    assert_eq!(rtl_table.rows[0].cells[0].text(), "أولى");
    assert_eq!(rtl_table.rows[0].cells[1].text(), "תא שני");
    assert!(rtl_table.rows[0].cells.iter().all(|cell| {
        cell.margins
            == Some(CellMargins {
                top: 0,
                right: 300,
                bottom: 0,
                left: 300,
            })
    }));
    let rtl_pdf = rtl
        .try_to_pdf_with_fonts(&fonts)
        .expect("fixed-font RTL render");
    assert!(!rtl_pdf.is_empty());
    assert_eq!(
        rtl_pdf,
        rtl.try_to_pdf_with_fonts(&fonts)
            .expect("repeat fixed-font RTL render")
    );

    let lists = open("table-cell-lists.docx");
    let list_model = lists.model();
    let Block::Paragraph(before) = &list_model.blocks[0] else {
        panic!("list fixture leading body paragraph");
    };
    let Block::Table(table) = &list_model.blocks[1] else {
        panic!("list fixture table");
    };
    let Block::Paragraph(after) = &list_model.blocks[2] else {
        panic!("list fixture trailing body paragraph");
    };
    let Block::Paragraph(direct) = &table.rows[0].cells[0].blocks[0] else {
        panic!("direct cell list paragraph");
    };
    let Block::Paragraph(bullet) = &table.rows[0].cells[0].blocks[1] else {
        panic!("cell bullet paragraph");
    };
    let Block::Table(nested) = &table.rows[0].cells[1].blocks[0] else {
        panic!("nested list table");
    };
    let Block::Paragraph(nested_item) = &nested.rows[0].cells[0].blocks[0] else {
        panic!("nested cell list paragraph");
    };
    assert!(table.bidi_visual);
    assert_eq!(
        before.props.list.as_ref().map(|list| list.label.as_str()),
        Some("4.")
    );
    assert_eq!(
        direct.props.list.as_ref().map(|list| list.label.as_str()),
        Some("5.")
    );
    assert_eq!(
        bullet
            .props
            .list
            .as_ref()
            .map(|list| (list.ordered, list.level, list.label.as_str())),
        // The fixture's numbering declares this bullet glyph, so it is captured
        // rather than left for the renderer's depth-based fallback.
        Some((false, 1, "\u{25CB}"))
    );
    assert_eq!(
        nested_item
            .props
            .list
            .as_ref()
            .map(|list| list.label.as_str()),
        Some("6.")
    );
    assert_eq!(
        after.props.list.as_ref().map(|list| list.label.as_str()),
        Some("7.")
    );

    let list_pdf =
        rwml::try_render_pdf_with_fonts(&list_model, &fonts).expect("table-cell list render");
    assert_eq!(
        list_pdf,
        rwml::try_render_pdf_with_fonts(&list_model, &fonts)
            .expect("repeat table-cell list render")
    );
    let mut changed_labels = list_model.clone();
    let mut changed = 0;
    let Block::Table(changed_table) = &mut changed_labels.blocks[1] else {
        panic!("changed list fixture table");
    };
    for row in &mut changed_table.rows {
        for cell in &mut row.cells {
            replace_cell_list_labels(&mut cell.blocks, &mut changed);
        }
    }
    assert_eq!(changed, 3);
    assert_ne!(
        list_pdf,
        rwml::try_render_pdf_with_fonts(&changed_labels, &fonts)
            .expect("changed table-cell marker render"),
        "changing only table-cell labels must change PDF output"
    );

    let wrap = open("wrap-top-bottom.docx");
    let report = wrap.report();
    assert_eq!(report.features.text_boxes, 1);
    assert_eq!(report.features.floating_shapes, 1);
    assert_eq!(wrap.floating_shapes().len(), 1);
}
