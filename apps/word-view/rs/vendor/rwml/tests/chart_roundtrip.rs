#![cfg(feature = "docx")]

use std::collections::BTreeMap;
use std::io::{Read, Write};

use rwml::{
    Block, Cell, Chart, ChartKind, ChartSeries, ChartShape, DocModel, DocSetup, Document, Row,
    Table,
};

fn package_parts(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("valid DOCX zip");
    let mut parts = BTreeMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("read DOCX part");
        if entry.is_dir() {
            continue;
        }
        let mut payload = Vec::new();
        entry.read_to_end(&mut payload).expect("read DOCX payload");
        parts.insert(entry.name().to_string(), payload);
    }
    parts
}

fn write_package(parts: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut bytes));
        let options = zip::write::SimpleFileOptions::default();
        for (name, payload) in parts {
            zip.start_file(name, options).expect("start DOCX part");
            zip.write_all(payload).expect("write DOCX part");
        }
        zip.finish().expect("finish DOCX zip");
    }
    bytes
}

fn chart(kind: ChartKind, name: &str) -> Chart {
    Chart {
        kind,
        title: Some(format!("{name} title")),
        categories: vec!["Q1".to_string(), "Q2".to_string()],
        series: vec![ChartSeries {
            name: format!("{name} series"),
            values: vec![12.5, 27.0],
            bubble_sizes: if matches!(kind, ChartKind::Bubble | ChartKind::Bubble3D) {
                vec![3.0, 5.0]
            } else {
                Vec::new()
            },
        }],
        width_px: Some(320),
        height_px: Some(180),
        alt: Some(format!("{name} alt")),
        wireframe: matches!(kind, ChartKind::Surface | ChartKind::Surface3D),
        shape: if matches!(
            kind,
            ChartKind::Bar3D
                | ChartKind::StackedBar3D
                | ChartKind::PercentStackedBar3D
                | ChartKind::Column3D
                | ChartKind::StackedColumn3D
                | ChartKind::PercentStackedColumn3D
        ) {
            ChartShape::Pyramid
        } else {
            ChartShape::Box
        },
    }
}

#[test]
fn fresh_core_and_chart_ex_payloads_reopen_as_modeled_charts() {
    let core = chart(ChartKind::StackedColumn3D, "Core");
    let chart_ex = chart(ChartKind::Waterfall, "Extended");
    let model = DocModel {
        blocks: vec![Block::Chart(core.clone()), Block::Chart(chart_ex.clone())],
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let reopened = Document::open(&bytes).expect("fresh chart document reopens");
    assert_eq!(
        reopened.model().blocks,
        vec![Block::Chart(core), Block::Chart(chart_ex)]
    );
    assert_eq!(reopened.report().features.charts, 2);
    assert_eq!(reopened.report().features.unsupported_charts, 0);
}

#[test]
fn every_authored_chart_kind_survives_fresh_native_reopen() {
    let kinds = [
        ChartKind::Bar,
        ChartKind::StackedBar,
        ChartKind::PercentStackedBar,
        ChartKind::Bar3D,
        ChartKind::StackedBar3D,
        ChartKind::PercentStackedBar3D,
        ChartKind::Column,
        ChartKind::StackedColumn,
        ChartKind::PercentStackedColumn,
        ChartKind::Column3D,
        ChartKind::StackedColumn3D,
        ChartKind::PercentStackedColumn3D,
        ChartKind::Line,
        ChartKind::LineNoMarkers,
        ChartKind::SmoothLine,
        ChartKind::StackedLine,
        ChartKind::PercentStackedLine,
        ChartKind::Line3D,
        ChartKind::Area,
        ChartKind::StackedArea,
        ChartKind::PercentStackedArea,
        ChartKind::Area3D,
        ChartKind::StackedArea3D,
        ChartKind::PercentStackedArea3D,
        ChartKind::Radar,
        ChartKind::RadarWithMarkers,
        ChartKind::FilledRadar,
        ChartKind::Scatter,
        ChartKind::ScatterMarkers,
        ChartKind::ScatterLines,
        ChartKind::ScatterSmooth,
        ChartKind::ScatterSmoothNoMarkers,
        ChartKind::Bubble,
        ChartKind::Bubble3D,
        ChartKind::Pie,
        ChartKind::ExplodedPie,
        ChartKind::Pie3D,
        ChartKind::ExplodedPie3D,
        ChartKind::PieOfPie,
        ChartKind::BarOfPie,
        ChartKind::Doughnut,
        ChartKind::ExplodedDoughnut,
        ChartKind::Surface,
        ChartKind::Surface3D,
        ChartKind::StockHighLowClose,
        ChartKind::Stock,
        ChartKind::Waterfall,
        ChartKind::Treemap,
        ChartKind::Sunburst,
        ChartKind::Histogram,
        ChartKind::BoxWhisker,
        ChartKind::Funnel,
    ];
    let model = DocModel {
        blocks: kinds
            .iter()
            .enumerate()
            .map(|(index, kind)| Block::Chart(chart(*kind, &format!("Chart {index}"))))
            .collect(),
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let reopened = Document::open(&bytes).expect("all generated chart kinds reopen");
    let reopened_model = reopened.model();
    let reopened_charts = reopened_model
        .blocks
        .iter()
        .filter_map(|block| match block {
            Block::Chart(chart) => Some(chart),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(reopened_charts.len(), kinds.len());
    for (index, reopened_chart) in reopened_charts.into_iter().enumerate() {
        let mut expected = chart(kinds[index], &format!("Chart {index}"));
        if matches!(
            expected.kind,
            ChartKind::Scatter
                | ChartKind::ScatterMarkers
                | ChartKind::ScatterLines
                | ChartKind::ScatterSmooth
                | ChartKind::ScatterSmoothNoMarkers
                | ChartKind::Bubble
                | ChartKind::Bubble3D
        ) {
            expected.categories.clear();
        }
        assert_eq!(reopened_chart, &expected, "chart kind {:?}", kinds[index]);
    }
    assert_eq!(reopened.report().features.charts, kinds.len());
    assert_eq!(reopened.report().features.unsupported_charts, 0);
}

#[test]
fn authored_chart_reopens_inside_a_table_cell() {
    let expected = chart(ChartKind::Column, "Nested");
    let model = DocModel {
        blocks: vec![Block::Table(Table {
            rows: vec![Row {
                cells: vec![Cell {
                    blocks: vec![Block::Chart(expected.clone())],
                    ..Cell::default()
                }],
            }],
            ..Table::default()
        })],
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let reopened = Document::open(&bytes).expect("nested chart document reopens");
    let Block::Table(table) = &reopened.model().blocks[0] else {
        panic!("expected table block");
    };
    assert_eq!(table.rows[0].cells[0].blocks, [Block::Chart(expected)]);
    assert_eq!(reopened.report().features.charts, 1);
    assert_eq!(reopened.report().features.unsupported_charts, 0);
}

#[test]
fn non_authored_chart_payload_stays_explicitly_unsupported() {
    let bytes = rwml::write_docx(&DocModel {
        blocks: vec![Block::Chart(chart(ChartKind::Column, "Unsupported"))],
        ..DocModel::default()
    });
    let mut parts = package_parts(&bytes);
    let payload = String::from_utf8(
        parts
            .remove("word/charts/chart1.xml")
            .expect("generated chart part"),
    )
    .expect("chart XML is UTF-8")
    .replace(
        "http://schemas.openxmlformats.org/drawingml/2006/chart",
        "urn:not-an-authored-chart",
    );
    parts.insert("word/charts/chart1.xml".to_string(), payload.into_bytes());

    let reopened = Document::open(&write_package(&parts)).expect("mutated package reopens");
    assert!(reopened
        .model()
        .blocks
        .iter()
        .all(|block| !matches!(block, Block::Chart(_))));
    assert_eq!(reopened.report().features.charts, 1);
    assert_eq!(reopened.report().features.unsupported_charts, 1);
}

fn running_chart_document() -> (DocModel, [Chart; 6]) {
    let expected = [
        chart(ChartKind::Bar, "Default header"),
        chart(ChartKind::Waterfall, "First header"),
        chart(ChartKind::Column3D, "Even header"),
        chart(ChartKind::Funnel, "Default footer"),
        chart(ChartKind::Line, "First footer"),
        chart(ChartKind::Treemap, "Even footer"),
    ];
    let model = DocModel {
        setup: DocSetup {
            header: vec![Block::Chart(expected[0].clone())],
            first_header: vec![Block::Chart(expected[1].clone())],
            even_header: vec![Block::Chart(expected[2].clone())],
            footer: vec![Block::Chart(expected[3].clone())],
            first_footer: vec![Block::Chart(expected[4].clone())],
            even_footer: vec![Block::Chart(expected[5].clone())],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };
    (model, expected)
}

#[test]
fn authored_charts_use_local_relationships_in_every_running_variant() {
    let (model, expected) = running_chart_document();
    let bytes = rwml::write_docx(&model);
    assert_eq!(bytes, rwml::write_docx(&model));
    let parts = package_parts(&bytes);

    for (part_path, rels_path, chart_tag, chart_target, alt) in [
        (
            "word/header1.xml",
            "word/_rels/header1.xml.rels",
            "c:chart",
            "charts/chart1.xml",
            "Default header alt",
        ),
        (
            "word/header2.xml",
            "word/_rels/header2.xml.rels",
            "cx:chart",
            "charts/chartEx2.xml",
            "First header alt",
        ),
        (
            "word/header3.xml",
            "word/_rels/header3.xml.rels",
            "c:chart",
            "charts/chart3.xml",
            "Even header alt",
        ),
        (
            "word/footer1.xml",
            "word/_rels/footer1.xml.rels",
            "cx:chart",
            "charts/chartEx4.xml",
            "Default footer alt",
        ),
        (
            "word/footer2.xml",
            "word/_rels/footer2.xml.rels",
            "c:chart",
            "charts/chart5.xml",
            "First footer alt",
        ),
        (
            "word/footer3.xml",
            "word/_rels/footer3.xml.rels",
            "cx:chart",
            "charts/chartEx6.xml",
            "Even footer alt",
        ),
    ] {
        let xml = String::from_utf8(parts[part_path].clone()).expect("running XML is UTF-8");
        let rels = String::from_utf8(parts[rels_path].clone()).expect("rels XML is UTF-8");
        assert!(
            xml.contains(&format!("<{chart_tag} r:id=\"rId1\"/>"))
                && xml.contains(&format!("descr=\"{alt}\""))
                && !xml.contains("rwml chart placeholder"),
            "running chart drawing missing from {part_path}: {xml}"
        );
        let namespace = if chart_tag == "c:chart" {
            "xmlns:c=\"http://schemas.openxmlformats.org/drawingml/2006/chart\""
        } else {
            "xmlns:cx=\"http://schemas.microsoft.com/office/drawing/2014/chartex\""
        };
        assert!(xml.contains(namespace), "missing {namespace}: {xml}");
        let relationship_type = if chart_tag == "c:chart" {
            "relationships/chart"
        } else {
            "relationships/chartEx"
        };
        assert!(
            rels.contains(r#"Id="rId1""#)
                && rels.contains(relationship_type)
                && rels.contains(chart_target),
            "part-local chart relationship missing from {rels_path}: {rels}"
        );
    }

    let document_rels = String::from_utf8(parts["word/_rels/document.xml.rels"].clone())
        .expect("document rels are UTF-8");
    assert!(
        !document_rels.contains("relationships/chart"),
        "{document_rels}"
    );
    assert_eq!(
        parts
            .keys()
            .filter(|path| { path.starts_with("word/charts/chart") && path.ends_with(".xml") })
            .count(),
        6
    );
    for chart_id in [1, 3, 5] {
        assert!(parts.contains_key(&format!("word/charts/chart{chart_id}.xml")));
        assert!(parts.contains_key(&format!("word/charts/_rels/chart{chart_id}.xml.rels")));
        assert!(parts.contains_key(&format!(
            "word/embeddings/Microsoft_Excel_Worksheet{chart_id}.xlsx"
        )));
    }

    let reopened = Document::open(&bytes).expect("running chart document reopens");
    let reopened_model = reopened.model();
    for (blocks, chart) in [
        (&reopened_model.setup.header, &expected[0]),
        (&reopened_model.setup.first_header, &expected[1]),
        (&reopened_model.setup.even_header, &expected[2]),
        (&reopened_model.setup.footer, &expected[3]),
        (&reopened_model.setup.first_footer, &expected[4]),
        (&reopened_model.setup.even_footer, &expected[5]),
    ] {
        assert_eq!(blocks, &[Block::Chart(chart.clone())]);
    }
    assert_eq!(reopened.report().features.charts, 6);
    assert_eq!(reopened.report().features.unsupported_charts, 0);
}

#[test]
fn authored_running_chart_reopens_through_nested_table_cells() {
    let expected = chart(ChartKind::Sunburst, "Nested running");
    let nested = Table {
        rows: vec![Row {
            cells: vec![Cell {
                blocks: vec![Block::Table(Table {
                    rows: vec![Row {
                        cells: vec![Cell {
                            blocks: vec![Block::Chart(expected.clone())],
                            ..Cell::default()
                        }],
                    }],
                    ..Table::default()
                })],
                ..Cell::default()
            }],
        }],
        ..Table::default()
    };
    let model = DocModel {
        setup: DocSetup {
            header: vec![Block::Table(nested)],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = package_parts(&bytes);
    assert!(parts.contains_key("word/charts/chartEx1.xml"));
    let reopened = Document::open(&bytes).expect("nested running chart reopens");
    let Block::Table(outer) = &reopened.model().setup.header[0] else {
        panic!("expected outer running table");
    };
    let Block::Table(inner) = &outer.rows[0].cells[0].blocks[0] else {
        panic!("expected nested running table");
    };
    assert_eq!(inner.rows[0].cells[0].blocks, [Block::Chart(expected)]);
}

#[test]
fn zero_series_running_chart_keeps_visible_fallback_without_package_parts() {
    let mut empty = chart(ChartKind::Waterfall, "Empty running");
    empty.series.clear();
    let model = DocModel {
        setup: DocSetup {
            header: vec![Block::Chart(empty)],
            ..DocSetup::default()
        },
        ..DocModel::default()
    };

    let bytes = rwml::write_docx(&model);
    let parts = package_parts(&bytes);
    let header = String::from_utf8(parts["word/header1.xml"].clone()).expect("header is UTF-8");
    assert!(header.contains("[rwml chart placeholder: Empty running alt]"));
    assert!(!header.contains("<w:drawing>"));
    assert!(!parts.contains_key("word/_rels/header1.xml.rels"));
    assert!(!parts
        .keys()
        .any(|path| path.starts_with("word/charts/") || path.starts_with("word/embeddings/")));

    let reopened = Document::open(&bytes).expect("empty running fallback reopens");
    assert!(reopened.text().contains("Empty running alt"));
}

#[cfg(feature = "render")]
#[test]
fn reopened_nested_chart_renders_deterministically() {
    let model = DocModel {
        blocks: vec![Block::Table(Table {
            rows: vec![Row {
                cells: vec![Cell {
                    blocks: vec![Block::Chart(chart(ChartKind::Bar, "Rendered"))],
                    ..Cell::default()
                }],
            }],
            ..Table::default()
        })],
        ..DocModel::default()
    };
    let reopened = Document::open(&rwml::write_docx(&model)).expect("render chart reopens");

    let rendered = reopened.to_pdf_with_report();
    let repeated = reopened.to_pdf_with_report();
    let blank = rwml::render_pdf_with_report(&DocModel::default());
    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.pdf, repeated.pdf);
    assert_ne!(rendered.pdf, blank.pdf);
    assert_eq!(rendered.report.unsupported.charts, 0);
}

#[cfg(feature = "render")]
#[test]
fn reopened_running_charts_render_deterministically() {
    let (model, _) = running_chart_document();
    let reopened = Document::open(&rwml::write_docx(&model)).expect("running charts reopen");
    let rendered = reopened.to_pdf_with_report();
    let blank = Document::open(&rwml::write_docx(&DocModel::default()))
        .expect("blank document reopens")
        .to_pdf_with_report();

    assert!(rendered.pdf.starts_with(b"%PDF"));
    assert_eq!(rendered.pdf, reopened.to_pdf_with_report().pdf);
    assert_ne!(rendered.pdf, blank.pdf);
    assert_eq!(rendered.report.unsupported.charts, 0);
}
