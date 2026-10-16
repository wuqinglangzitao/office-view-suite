#![cfg(feature = "docx")]

use std::io::{Read, Write};

use rwml::{Block, CellMargins, Document, Table};

fn docx_fixture(document_xml: &str) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut out);
        let mut zip = zip::ZipWriter::new(cursor);
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
            ),
            (
                "word/_rels/document.xml.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"/>"#,
            ),
            ("word/document.xml", document_xml),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    out
}

fn first_table(document_xml: &str) -> Table {
    tables(document_xml).remove(0)
}

fn tables(document_xml: &str) -> Vec<Table> {
    Document::open(&docx_fixture(document_xml))
        .expect("fixture opens")
        .model()
        .blocks
        .into_iter()
        .filter_map(|block| match block {
            Block::Table(table) => Some(table),
            _ => None,
        })
        .collect()
}

fn zip_part(bytes: &[u8], name: &str) -> Vec<u8> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("valid zip");
    let mut part = zip.by_name(name).expect("part exists");
    let mut out = Vec::new();
    part.read_to_end(&mut out).expect("part reads");
    out
}

#[test]
fn direct_table_cell_margins_cascade_per_side() {
    let table = first_table(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:tbl>
                <w:tblPr>
                    <w:tblCellMar>
                        <w:top w:w="120" w:type="dxa"/>
                        <w:start w:w="240" w:type="dxa"/>
                        <w:bottom w:w="360" w:type="dxa"/>
                        <w:end w:w="480" w:type="dxa"/>
                    </w:tblCellMar>
                </w:tblPr>
                <w:tr>
                    <w:tc><w:p><w:r><w:t>default</w:t></w:r></w:p></w:tc>
                    <w:tc>
                        <w:tcPr>
                            <w:tcMar>
                                <w:top w:w="0" w:type="dxa"/>
                                <w:end w:w="600" w:type="dxa"/>
                            </w:tcMar>
                        </w:tcPr>
                        <w:p><w:r><w:t>override</w:t></w:r></w:p>
                    </w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#,
    );

    assert_eq!(
        table.rows[0].cells[0].margins,
        Some(CellMargins {
            top: 120,
            right: 480,
            bottom: 360,
            left: 240,
        })
    );
    assert_eq!(
        table.rows[0].cells[1].margins,
        Some(CellMargins {
            top: 0,
            right: 600,
            bottom: 360,
            left: 240,
        })
    );
}

#[test]
fn row_table_cell_margin_exceptions_replace_direct_table_defaults_per_side() {
    let table = first_table(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:tbl>
                <w:tblPr><w:tblCellMar>
                    <w:top w:w="100"/><w:start w:w="200"/>
                    <w:bottom w:w="300"/><w:end w:w="400"/>
                </w:tblCellMar></w:tblPr>
                <w:tr>
                    <w:tblPrEx><w:tblCellMar>
                        <w:top w:w="500"/><w:end w:w="600"/>
                    </w:tblCellMar></w:tblPrEx>
                    <w:tc><w:p><w:r><w:t>row defaults</w:t></w:r></w:p></w:tc>
                    <w:tc><w:tcPr><w:tcMar>
                        <w:start w:w="800"/><w:bottom w:w="700"/>
                    </w:tcMar></w:tcPr><w:p><w:r><w:t>cell override</w:t></w:r></w:p></w:tc>
                </w:tr>
                <w:tr>
                    <w:tc><w:p><w:r><w:t>table defaults</w:t></w:r></w:p></w:tc>
                </w:tr>
                <w:tr>
                    <w:tblPrEx><w:tblCellMar/></w:tblPrEx>
                    <w:tc><w:p><w:r><w:t>empty row defaults</w:t></w:r></w:p></w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#,
    );

    assert_eq!(
        table.rows[0].cells[0].margins,
        Some(CellMargins {
            top: 500,
            right: 600,
            bottom: 0,
            left: 115,
        })
    );
    assert_eq!(
        table.rows[0].cells[1].margins,
        Some(CellMargins {
            top: 500,
            right: 600,
            bottom: 700,
            left: 800,
        })
    );
    assert_eq!(
        table.rows[1].cells[0].margins,
        Some(CellMargins {
            top: 100,
            right: 400,
            bottom: 300,
            left: 200,
        })
    );
    assert_eq!(
        table.rows[2].cells[0].margins,
        Some(CellMargins {
            top: 0,
            right: 115,
            bottom: 0,
            left: 115,
        })
    );
}

#[test]
fn row_margin_exceptions_follow_current_mce_scope_and_rtl_defaults() {
    let tables = tables(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tblPr><w:tblCellMar>
                    <w:top w:w="10"/><w:start w:w="20"/>
                    <w:bottom w:w="30"/><w:end w:w="40"/>
                </w:tblCellMar></w:tblPr>
                <w:tr>
                    <w:tblPrEx>
                        <w:tblPrExChange><w:tblPrEx><w:tblCellMar>
                            <w:top w:w="800"/><w:start w:w="800"/>
                        </w:tblCellMar></w:tblPrEx></w:tblPrExChange>
                        <w:unknown><w:tblCellMar><w:top w:w="900"/></w:tblCellMar></w:unknown>
                        <mc:AlternateContent>
                            <mc:Choice Requires="w14"><w:tblCellMar>
                                <w:top w:w="120"/><w:start w:w="140"/>
                            </w:tblCellMar></mc:Choice>
                            <mc:Fallback><w:tblCellMar>
                                <w:top w:w="900"/><w:start w:w="900"/>
                            </w:tblCellMar></mc:Fallback>
                        </mc:AlternateContent>
                        <w:tblCellMar><w:bottom w:w="160"/></w:tblCellMar>
                    </w:tblPrEx>
                    <w:tc><w:tcPr><w:tcMar><w:end w:w="180"/></w:tcMar></w:tcPr>
                        <w:p><w:r><w:t>current row</w:t></w:r></w:p>
                    </w:tc>
                </w:tr>
                <w:tr><mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:tblPrEx><w:tblCellMar><w:top w:w="220"/></w:tblCellMar></w:tblPrEx>
                        <w:tc><w:p><w:r><w:t>selected row</w:t></w:r></w:p></w:tc>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:tblPrEx><w:tblCellMar><w:top w:w="900"/></w:tblCellMar></w:tblPrEx>
                        <w:tc><w:p><w:r><w:t>fallback row</w:t></w:r></w:p></w:tc>
                    </mc:Fallback>
                </mc:AlternateContent></w:tr>
                <w:tr>
                    <w:tblPrEx><mc:AlternateContent>
                        <mc:Choice Requires="w14"/>
                        <mc:Fallback><w:tblCellMar><w:top w:w="900"/></w:tblCellMar></mc:Fallback>
                    </mc:AlternateContent></w:tblPrEx>
                    <w:tc><w:p><w:r><w:t>empty selected row properties</w:t></w:r></w:p></w:tc>
                </w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:bidiVisual/></w:tblPr>
                <w:tr>
                    <w:tblPrEx><w:tblCellMar><w:start w:w="210"/></w:tblCellMar></w:tblPrEx>
                    <w:tc><w:tcPr><w:tcMar><w:end w:w="220"/></w:tcMar></w:tcPr>
                        <w:p><w:r><w:t>RTL row</w:t></w:r></w:p>
                    </w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#,
    );

    assert_eq!(
        tables[0].rows[0].cells[0].margins,
        Some(CellMargins {
            top: 120,
            right: 180,
            bottom: 160,
            left: 140,
        })
    );
    assert_eq!(tables[0].rows[1].cells[0].text(), "selected row");
    assert_eq!(
        tables[0].rows[1].cells[0].margins,
        Some(CellMargins {
            top: 220,
            right: 115,
            bottom: 0,
            left: 115,
        })
    );
    assert_eq!(
        tables[0].rows[2].cells[0].margins,
        Some(CellMargins {
            top: 10,
            right: 40,
            bottom: 30,
            left: 20,
        })
    );
    assert_eq!(
        tables[1].rows[0].cells[0].margins,
        Some(CellMargins {
            top: 0,
            right: 210,
            bottom: 0,
            left: 220,
        })
    );
}

#[test]
fn row_markup_compatibility_empty_choice_does_not_leak_fallback_content() {
    let table = first_table(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tblPr><w:tblCellMar>
                    <w:top w:w="10"/><w:start w:w="20"/>
                    <w:bottom w:w="30"/><w:end w:w="40"/>
                </w:tblCellMar></w:tblPr>
                <w:tr>
                    <mc:AlternateContent>
                        <mc:Choice Requires="w14"/>
                        <mc:Fallback>
                            <w:tblPrEx><w:tblCellMar><w:top w:w="900"/></w:tblCellMar></w:tblPrEx>
                            <w:tc><w:p><w:r><w:t>fallback cell</w:t></w:r></w:p></w:tc>
                        </mc:Fallback>
                    </mc:AlternateContent>
                    <w:tc><w:p><w:r><w:t>current cell</w:t></w:r></w:p></w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#,
    );

    assert_eq!(table.rows[0].cells.len(), 1);
    assert_eq!(table.rows[0].cells[0].text(), "current cell");
    assert_eq!(
        table.rows[0].cells[0].margins,
        Some(CellMargins {
            top: 10,
            right: 40,
            bottom: 30,
            left: 20,
        })
    );
}

#[test]
fn one_cell_margin_activates_schema_defaults_for_untouched_siblings() {
    let table = first_table(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:tbl>
                <w:tr>
                    <w:tc><w:tcPr><w:tcMar><w:top w:w="240"/></w:tcMar></w:tcPr>
                        <w:p><w:r><w:t>participating</w:t></w:r></w:p>
                    </w:tc>
                    <w:tc><w:p><w:r><w:t>untouched</w:t></w:r></w:p></w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#,
    );

    assert_eq!(
        table.rows[0].cells[0].margins,
        Some(CellMargins {
            top: 240,
            right: 115,
            bottom: 0,
            left: 115,
        })
    );
    assert_eq!(
        table.rows[0].cells[1].margins,
        Some(CellMargins {
            top: 0,
            right: 115,
            bottom: 0,
            left: 115,
        })
    );
}

#[test]
fn logical_margin_aliases_follow_table_direction_and_use_schema_defaults() {
    let tables = tables(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:tbl>
                <w:tblPr><w:tblCellMar>
                    <w:start w:w="100"/><w:end w:w="200"/>
                </w:tblCellMar></w:tblPr>
                <w:tr><w:tc><w:tcPr><w:tcMar><w:top w:w="300"/></w:tcMar></w:tcPr>
                    <w:p><w:r><w:t>LTR</w:t></w:r></w:p>
                </w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:bidiVisual/><w:tblCellMar>
                    <w:left w:w="110"/><w:right w:w="220"/>
                </w:tblCellMar></w:tblPr>
                <w:tr><w:tc><w:tcPr><w:tcMar><w:bottom w:type="nil"/></w:tcMar></w:tcPr>
                    <w:p><w:r><w:t>RTL</w:t></w:r></w:p>
                </w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#,
    );

    assert_eq!(
        tables[0].rows[0].cells[0].margins,
        Some(CellMargins {
            top: 300,
            right: 200,
            bottom: 0,
            left: 100,
        })
    );
    assert_eq!(
        tables[1].rows[0].cells[0].margins,
        Some(CellMargins {
            top: 0,
            right: 110,
            bottom: 0,
            left: 220,
        })
    );
}

#[test]
fn table_margin_defaults_require_direct_table_property_scope() {
    let tables = tables(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tblPr>
                    <w:tblStyle w:val="Nested"><w:tblCellMar>
                        <w:top w:w="900"/><w:start w:w="900"/>
                    </w:tblCellMar></w:tblStyle>
                    <w:tblCellMar><w:bottom w:w="120"/></w:tblCellMar>
                </w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>direct scope</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:tblStyle w:val="Nested"><w:tblCellMar>
                            <w:top w:w="900"/><w:start w:w="900"/>
                        </w:tblCellMar></w:tblStyle>
                        <w:tblCellMar><w:bottom w:w="130"/></w:tblCellMar>
                    </mc:Choice>
                    <mc:Fallback><w:tblCellMar><w:top w:w="800"/></w:tblCellMar></mc:Fallback>
                </mc:AlternateContent></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>MCE scope</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#,
    );

    for (table, bottom) in tables.iter().zip([120, 130]) {
        assert_eq!(
            table.rows[0].cells[0].margins,
            Some(CellMargins {
                top: 0,
                right: 115,
                bottom,
                left: 115,
            })
        );
    }
}

#[test]
fn canonical_logical_sides_suppress_legacy_aliases_regardless_of_order() {
    let table = first_table(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:tbl>
                <w:tblPr><w:tblCellMar>
                    <w:start w:w="200"/><w:left w:w="900"/>
                    <w:right w:w="800"/><w:end w:w="300"/>
                    <w:top w:w="100"/><w:bottom w:w="400"/>
                </w:tblCellMar></w:tblPr>
                <w:tr>
                    <w:tc><w:tcPr><w:tcMar>
                        <w:left w:w="700"/><w:start w:w="500"/>
                        <w:end w:w="600"/><w:right w:w="700"/>
                    </w:tcMar></w:tcPr><w:p><w:r><w:t>valid canonical</w:t></w:r></w:p></w:tc>
                    <w:tc><w:tcPr><w:tcMar>
                        <w:left w:w="700"/><w:start w:w="bad"/>
                        <w:right w:w="700"/><w:end w:w="900" w:type="auto"/>
                    </w:tcMar></w:tcPr><w:p><w:r><w:t>invalid canonical</w:t></w:r></w:p></w:tc>
                    <w:tc><w:tcPr><w:tcMar>
                        <w:start w:w="450"/><w:start w:w="500"/><w:start w:w="bad"/>
                        <w:end w:w="550"/><w:end w:w="600"/><w:end w:type="auto" w:w="900"/>
                    </w:tcMar></w:tcPr><w:p><w:r><w:t>valid before invalid</w:t></w:r></w:p></w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#,
    );

    assert_eq!(
        table.rows[0].cells[0].margins,
        Some(CellMargins {
            top: 100,
            right: 600,
            bottom: 400,
            left: 500,
        })
    );
    assert_eq!(
        table.rows[0].cells[1].margins,
        Some(CellMargins {
            top: 100,
            right: 300,
            bottom: 400,
            left: 200,
        })
    );
    assert_eq!(
        table.rows[0].cells[2].margins,
        Some(CellMargins {
            top: 100,
            right: 600,
            bottom: 400,
            left: 500,
        })
    );
}

#[test]
fn invalid_nested_and_historical_margin_values_do_not_override_current_values() {
    let table = first_table(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:tbl>
                <w:tblPr>
                    <w:tblCellMar>
                        <w:top w:w="120"/><w:start w:w="240"/>
                        <w:bottom w:w="360"/><w:end w:w="480"/>
                        <w:unknown><w:top w:w="999"/><w:start w:w="999"/></w:unknown>
                    </w:tblCellMar>
                    <w:tblPrChange><w:tblPr><w:tblCellMar>
                        <w:top w:w="777"/><w:start w:w="777"/>
                    </w:tblCellMar></w:tblPr></w:tblPrChange>
                </w:tblPr>
                <w:tr>
                    <w:tc>
                        <w:tcPr>
                            <w:tcMar>
                                <w:top w:w="999" w:type="pct"/>
                                <w:top w:w="998" w:type=""/>
                                <w:start w:w="999" w:type="auto"/>
                                <w:bottom w:type="dxa"/>
                                <w:end w:w="-1" w:type="dxa"/>
                                <w:unknown><w:end w:w="999"/></w:unknown>
                            </w:tcMar>
                            <w:tcPrChange><w:tcPr><w:tcMar>
                                <w:top w:w="888"/><w:end w:w="888"/>
                            </w:tcMar></w:tcPr></w:tcPrChange>
                        </w:tcPr>
                        <w:p><w:r><w:t>inherit</w:t></w:r></w:p>
                    </w:tc>
                    <w:tc>
                        <w:tcPr><w:tcMar>
                            <w:top w:type="nil"/><w:left w:type="nil"/>
                            <w:bottom w:type="nil"/><w:right w:type="nil"/>
                        </w:tcMar></w:tcPr>
                        <w:p><w:r><w:t>nil</w:t></w:r></w:p>
                    </w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#,
    );

    assert_eq!(
        table.rows[0].cells[0].margins,
        Some(CellMargins {
            top: 120,
            right: 480,
            bottom: 360,
            left: 240,
        })
    );
    assert_eq!(table.rows[0].cells[1].margins, Some(CellMargins::default()));
}

#[test]
fn table_and_cell_margin_markup_compatibility_select_one_branch() {
    let tables = tables(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tblPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:tblCellMar>
                        <w:start w:w="120"/>
                    </w:tblCellMar></mc:Choice>
                    <mc:Fallback><w:tblCellMar>
                        <w:start w:w="900"/>
                    </w:tblCellMar></mc:Fallback>
                </mc:AlternateContent></w:tblPr>
                <w:tr><w:tc><w:tcPr><w:tcMar><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:end w:w="240"/></mc:Choice>
                    <mc:Fallback><w:end w:w="900"/></mc:Fallback>
                </mc:AlternateContent></w:tcMar></w:tcPr>
                    <w:p><w:r><w:t>choice</w:t></w:r></w:p>
                </w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"/>
                    <mc:Fallback><w:tblCellMar><w:top w:w="900"/></w:tblCellMar></mc:Fallback>
                </mc:AlternateContent></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>empty choice</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr><w:tblCellMar>
                    <w:top w:w="130"/><w:start w:w="140"/>
                    <w:bottom w:w="150"/><w:end w:w="160"/>
                </w:tblCellMar></w:tblPr>
                <w:tr><w:tc><w:tcPr><w:tcMar><mc:AlternateContent>
                    <mc:Choice Requires="w14"/>
                    <mc:Fallback><w:top w:w="900"/></mc:Fallback>
                </mc:AlternateContent></w:tcMar></w:tcPr>
                    <w:p><w:r><w:t>cell empty choice</w:t></w:r></w:p>
                </w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#,
    );

    assert_eq!(
        tables[0].rows[0].cells[0].margins,
        Some(CellMargins {
            top: 0,
            right: 240,
            bottom: 0,
            left: 120,
        })
    );
    assert_eq!(tables[1].rows[0].cells[0].margins, None);
    assert_eq!(
        tables[2].rows[0].cells[0].margins,
        Some(CellMargins {
            top: 130,
            right: 160,
            bottom: 150,
            left: 140,
        })
    );
}

#[test]
fn markup_compatibility_can_wrap_table_and_cell_property_containers() {
    let tables = tables(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:tblPr>
                        <w:tblPrChange><w:tblPr><w:tblCellMar>
                            <w:start w:w="800"/>
                        </w:tblCellMar></w:tblPr></w:tblPrChange>
                        <w:tblCellMar>
                            <w:top w:w="120"></w:top><w:start w:w="140"/>
                        </w:tblCellMar>
                    </w:tblPr></mc:Choice>
                    <mc:Fallback><w:tblPr><w:tblCellMar>
                        <w:top w:w="900"/><w:start w:w="900"/>
                    </w:tblCellMar></w:tblPr></mc:Fallback>
                </mc:AlternateContent>
                <w:tr><w:tc>
                    <mc:AlternateContent>
                        <mc:Choice Requires="w14"><w:tcPr>
                            <w:tcPrChange><w:tcPr><w:tcMar>
                                <w:end w:w="800"/>
                            </w:tcMar></w:tcPr></w:tcPrChange>
                            <w:tcMar>
                                <w:bottom w:w="240"></w:bottom><w:end w:w="260"/>
                            </w:tcMar>
                        </w:tcPr></mc:Choice>
                        <mc:Fallback><w:tcPr><w:tcMar>
                            <w:bottom w:w="900"/><w:end w:w="900"/>
                        </w:tcMar></w:tcPr></mc:Fallback>
                    </mc:AlternateContent>
                    <w:p><w:r><w:t>selected properties</w:t></w:r></w:p>
                </w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14"/>
                    <mc:Fallback>
                        <w:tblPr><w:tblCellMar><w:top w:w="900"/></w:tblCellMar></w:tblPr>
                        <w:tr><w:tc><w:p><w:r><w:t>fallback row</w:t></w:r></w:p></w:tc></w:tr>
                    </mc:Fallback>
                </mc:AlternateContent>
                <w:tr><w:tc>
                    <mc:AlternateContent>
                        <mc:Choice Requires="w14"/>
                        <mc:Fallback>
                            <w:tcPr><w:tcMar><w:top w:w="900"/></w:tcMar></w:tcPr>
                            <w:p><w:r><w:t>fallback block</w:t></w:r></w:p>
                        </mc:Fallback>
                    </mc:AlternateContent>
                    <w:p><w:r><w:t>outside block</w:t></w:r></w:p>
                </w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#,
    );

    assert_eq!(
        tables[0].rows[0].cells[0].margins,
        Some(CellMargins {
            top: 120,
            right: 260,
            bottom: 240,
            left: 140,
        })
    );
    assert_eq!(tables[1].rows.len(), 1);
    assert_eq!(tables[1].rows[0].cells[0].text(), "outside block");
    assert_eq!(tables[1].rows[0].cells[0].margins, None);
}

#[test]
fn cell_markup_compatibility_preserves_selected_content_control_metadata() {
    let table = first_table(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl><w:tr><w:tc>
                <mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:sdt>
                            <w:sdtPr>
                                <w:alias w:val=" Selected cell control "/>
                                <w:tag w:val=" selected-cell "/>
                            </w:sdtPr>
                            <w:sdtContent>
                                <w:p><w:r><w:t>selected content</w:t></w:r></w:p>
                            </w:sdtContent>
                        </w:sdt>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:p><w:r><w:t>fallback content</w:t></w:r></w:p>
                    </mc:Fallback>
                </mc:AlternateContent>
            </w:tc></w:tr></w:tbl>
        </w:body></w:document>"#,
    );

    let Block::Paragraph(paragraph) = &table.rows[0].cells[0].blocks[0] else {
        panic!("selected content is a paragraph");
    };
    assert_eq!(paragraph.text(), "selected content");
    let control = paragraph.runs[0]
        .content_control
        .as_ref()
        .expect("selected content-control metadata");
    assert_eq!(control.alias.as_deref(), Some("Selected cell control"));
    assert_eq!(control.tag.as_deref(), Some("selected-cell"));
}

fn nested_alternate_content(mut inner: String, depth: usize) -> String {
    for _ in 0..depth {
        inner = format!(
            r#"<mc:AlternateContent><mc:Choice Requires="w14">{inner}</mc:Choice><mc:Fallback><w:top w:w="900"/></mc:Fallback></mc:AlternateContent>"#
        );
    }
    inner
}

#[test]
fn deeply_nested_margin_markup_compatibility_is_bounded_and_recovers() {
    let nested_margin = nested_alternate_content(r#"<w:top w:w="999"/>"#.to_string(), 140);
    let nested_table_props = nested_alternate_content(
        r#"<w:tblCellMar><w:top w:w="999"/></w:tblCellMar>"#.to_string(),
        140,
    );
    let nested_cell_props =
        nested_alternate_content(r#"<w:tcMar><w:top w:w="999"/></w:tcMar>"#.to_string(), 140);
    let nested_row_properties = nested_alternate_content(
        r#"<w:tblCellMar><w:top w:w="999"/></w:tblCellMar>"#.to_string(),
        140,
    );
    let over_budget_row_property = nested_alternate_content(
        r#"<w:tblCellMar><w:top w:w="999"/></w:tblCellMar>"#.to_string(),
        4,
    );
    let nested_row_scope = nested_alternate_content(
        format!(r#"<w:tblPrEx>{over_budget_row_property}</w:tblPrEx>"#),
        63,
    );
    let xml = format!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
            xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><w:body>
            <w:tbl>
                <w:tblPr><w:tblCellMar>{nested_margin}<w:bottom w:w="321"/></w:tblCellMar></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>margin depth</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tblPr>{nested_table_props}<w:tblCellMar><w:bottom w:w="322"/></w:tblCellMar></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>table depth</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tr><w:tc><w:tcPr>{nested_cell_props}<w:tcMar><w:bottom w:w="323"/></w:tcMar></w:tcPr>
                    <w:p><w:r><w:t>cell depth</w:t></w:r></w:p>
                </w:tc></w:tr>
            </w:tbl>
            <w:tbl>
                <w:tr><w:tblPrEx>{nested_row_properties}<w:tblCellMar><w:bottom w:w="324"/></w:tblCellMar></w:tblPrEx>
                    <w:tc><w:p><w:r><w:t>row depth</w:t></w:r></w:p></w:tc>
                </w:tr>
            </w:tbl>
            <w:tbl>
                <w:tr>{nested_row_scope}
                    <w:tblPrEx><w:tblCellMar><w:bottom w:w="325"/></w:tblCellMar></w:tblPrEx>
                    <w:tc><w:p><w:r><w:t>shared depth recovery</w:t></w:r></w:p></w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#
    );
    let tables = tables(&xml);

    for (table, bottom) in tables.iter().zip([321, 322, 323, 324, 325]) {
        assert_eq!(
            table.rows[0].cells[0].margins,
            Some(CellMargins {
                top: 0,
                right: 115,
                bottom,
                left: 115,
            })
        );
    }
}

#[test]
fn table_margin_resolution_uses_restart_cell_and_reaches_nested_tables() {
    let table = first_table(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:tbl>
                <w:tblPr><w:tblCellMar>
                    <w:top w:w="100"/><w:start w:w="200"/>
                    <w:bottom w:w="300"/><w:end w:w="400"/>
                </w:tblCellMar></w:tblPr>
                <w:tr>
                    <w:tblPrEx><w:tblCellMar><w:top w:w="500"/></w:tblCellMar></w:tblPrEx>
                    <w:tc>
                    <w:tcPr>
                        <w:vMerge w:val="restart"/>
                        <w:tcMar><w:start w:w="550"/></w:tcMar>
                    </w:tcPr>
                    <w:tbl>
                        <w:tr>
                            <w:tblPrEx><w:tblCellMar>
                                <w:top w:w="50"/><w:start w:w="60"/>
                                <w:bottom w:w="70"/><w:end w:w="80"/>
                            </w:tblCellMar></w:tblPrEx>
                            <w:tc><w:p><w:r><w:t>nested</w:t></w:r></w:p></w:tc>
                        </w:tr>
                    </w:tbl>
                    <w:p/>
                </w:tc></w:tr>
                <w:tr>
                    <w:tblPrEx><w:tblCellMar><w:top w:w="900"/></w:tblCellMar></w:tblPrEx>
                    <w:tc>
                    <w:tcPr>
                        <w:vMerge/>
                        <w:tcMar><w:start w:w="950"/><w:end w:w="960"/></w:tcMar>
                    </w:tcPr>
                    <w:p/>
                </w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#,
    );

    let cell = &table.rows[0].cells[0];
    assert_eq!(cell.row_span, 2);
    assert_eq!(
        cell.margins,
        Some(CellMargins {
            top: 500,
            right: 115,
            bottom: 0,
            left: 550,
        })
    );
    let Block::Table(nested) = &cell.blocks[0] else {
        panic!("first cell block is the nested table");
    };
    assert_eq!(
        nested.rows[0].cells[0].margins,
        Some(CellMargins {
            top: 50,
            right: 80,
            bottom: 70,
            left: 60,
        })
    );
}

#[test]
fn fresh_conversion_preserves_physical_rtl_margins_and_save_preserves_source_part() {
    let document_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:tbl>
                <w:tblPr><w:bidiVisual/><w:tblCellMar>
                    <w:top w:w="30"/><w:start w:w="100"/>
                    <w:bottom w:w="40"/><w:end w:w="200"/>
                </w:tblCellMar></w:tblPr>
                <w:tr><w:tc><w:p><w:r><w:t>RTL margins</w:t></w:r></w:p></w:tc></w:tr>
            </w:tbl>
        </w:body></w:document>"#;
    let source = docx_fixture(document_xml);
    let document = Document::open(&source).expect("fixture opens");
    let Block::Table(table) = &document.model().blocks[0] else {
        panic!("first block is a table");
    };
    let expected = CellMargins {
        top: 30,
        right: 100,
        bottom: 40,
        left: 200,
    };
    assert_eq!(table.rows[0].cells[0].margins, Some(expected));

    let saved = document.save().expect("package-preserving save succeeds");
    assert_eq!(
        zip_part(&saved, "word/document.xml"),
        document_xml.as_bytes()
    );

    let converted = document.to_docx();
    let converted_xml = String::from_utf8(zip_part(&converted, "word/document.xml"))
        .expect("document XML is UTF-8");
    assert!(converted_xml.contains(concat!(
        r#"<w:tcMar><w:top w:w="30" w:type="dxa"/>"#,
        r#"<w:left w:w="100" w:type="dxa"/>"#,
        r#"<w:bottom w:w="40" w:type="dxa"/>"#,
        r#"<w:right w:w="200" w:type="dxa"/></w:tcMar>"#,
    )));
    assert!(!converted_xml.contains("<w:tblCellMar>"));
    let reopened = Document::open(&converted).expect("fresh conversion reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("reopened first block is a table");
    };
    assert_eq!(reopened_table.rows[0].cells[0].margins, Some(expected));
}

#[test]
fn fresh_conversion_canonicalizes_row_margin_exceptions_per_cell() {
    let document_xml = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:tbl>
                <w:tblPr><w:tblCellMar><w:bottom w:w="30"/><w:end w:w="40"/></w:tblCellMar></w:tblPr>
                <w:tr>
                    <w:tblPrEx><w:tblCellMar><w:top w:w="10"/><w:start w:w="20"/></w:tblCellMar></w:tblPrEx>
                    <w:tc><w:p><w:r><w:t>row margins</w:t></w:r></w:p></w:tc>
                </w:tr>
            </w:tbl>
        </w:body></w:document>"#;
    let source = docx_fixture(document_xml);
    let document = Document::open(&source).expect("fixture opens");
    let expected = CellMargins {
        top: 10,
        right: 115,
        bottom: 0,
        left: 20,
    };
    let Block::Table(table) = &document.model().blocks[0] else {
        panic!("first block is a table");
    };
    assert_eq!(table.rows[0].cells[0].margins, Some(expected));

    let saved = document.save().expect("package-preserving save succeeds");
    assert_eq!(
        zip_part(&saved, "word/document.xml"),
        document_xml.as_bytes()
    );

    let converted = document.to_docx();
    let converted_xml = String::from_utf8(zip_part(&converted, "word/document.xml"))
        .expect("document XML is UTF-8");
    assert!(converted_xml.contains(concat!(
        r#"<w:tcMar><w:top w:w="10" w:type="dxa"/>"#,
        r#"<w:left w:w="20" w:type="dxa"/>"#,
        r#"<w:bottom w:w="0" w:type="dxa"/>"#,
        r#"<w:right w:w="115" w:type="dxa"/></w:tcMar>"#,
    )));
    assert!(!converted_xml.contains("<w:tblPrEx>"));
    let reopened = Document::open(&converted).expect("fresh conversion reopens");
    let Block::Table(reopened_table) = &reopened.model().blocks[0] else {
        panic!("reopened first block is a table");
    };
    assert_eq!(reopened_table.rows[0].cells[0].margins, Some(expected));
}

#[cfg(feature = "render")]
#[test]
fn opened_table_margin_defaults_change_preview_layout_deterministically() {
    let source = |table_properties: &str, row_properties: &str| {
        docx_fixture(&format!(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
                <w:tbl>
                    <w:tblPr>{table_properties}</w:tblPr>
                    <w:tr>{row_properties}<w:tc><w:p><w:r>
                        <w:t>line one</w:t><w:br/>
                        <w:t>line two</w:t><w:br/>
                        <w:t>line three</w:t>
                    </w:r></w:p></w:tc></w:tr>
                </w:tbl>
                <w:sectPr>
                    <w:pgSz w:w="4400" w:h="1800"/>
                    <w:pgMar w:top="200" w:right="200" w:bottom="200" w:left="200"/>
                </w:sectPr>
            </w:body></w:document>"#
        ))
    };
    let baseline = Document::open(&source("", "")).expect("baseline opens");
    let padded = Document::open(&source(
        r#"<w:tblCellMar>
            <w:top w:w="400"/><w:start w:w="100"/>
            <w:bottom w:w="400"/><w:end w:w="100"/>
        </w:tblCellMar>"#,
        "",
    ))
    .expect("padded fixture opens");
    let row_padded = Document::open(&source(
        "",
        r#"<w:tblPrEx><w:tblCellMar>
            <w:top w:w="400"/><w:start w:w="100"/>
            <w:bottom w:w="400"/><w:end w:w="100"/>
        </w:tblCellMar></w:tblPrEx>"#,
    ))
    .expect("row-padded fixture opens");
    let fonts = vec![rwml_fonts::noto_sans_kr_subset().to_vec()];

    let baseline_layout = baseline
        .layout_pages_with_fonts(&fonts)
        .expect("baseline layout");
    let padded_layout = padded
        .layout_pages_with_fonts(&fonts)
        .expect("padded layout");
    let row_padded_layout = row_padded
        .layout_pages_with_fonts(&fonts)
        .expect("row-padded layout");
    assert_eq!((baseline_layout.pages, padded_layout.pages), (1, 2));
    assert_eq!(row_padded_layout, padded_layout);
    assert_eq!(
        padded_layout,
        padded
            .layout_pages_with_fonts(&fonts)
            .expect("repeat padded layout")
    );

    let baseline_pdf = baseline.to_pdf_with_fonts(&fonts);
    let padded_pdf = padded.to_pdf_with_fonts(&fonts);
    let row_padded_pdf = row_padded.to_pdf_with_fonts(&fonts);
    assert!(padded_pdf.starts_with(b"%PDF-"));
    assert_ne!(padded_pdf, baseline_pdf);
    assert_eq!(row_padded_pdf, padded_pdf);
    assert_eq!(padded_pdf, padded.to_pdf_with_fonts(&fonts));
}
