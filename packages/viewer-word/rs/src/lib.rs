//! word-view-rs:rwml 文档模型 → 富样式 DTO(kind-tagged camelCase),
//! 供前端已验收的自研 DOM 渲染器直接消费。DOC/DOCX 均走 rwml 解析。

use base64::Engine as _;
use rwml::{Align, Block, Document, Run, Table, VertAlign};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocDto {
    blocks: Vec<BlockDto>,
    setup: SectionDto,
    stats: StatsDto,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PageDto {
    width_pt: f32,
    height_pt: f32,
    left_pt: f32,
    right_pt: f32,
    top_pt: f32,
    bottom_pt: f32,
    landscape: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SectionDto {
    page: PageDto,
    header: Vec<BlockDto>,
    first_header: Vec<BlockDto>,
    even_header: Vec<BlockDto>,
    footer: Vec<BlockDto>,
    first_footer: Vec<BlockDto>,
    even_footer: Vec<BlockDto>,
    title_page: bool,
    page_numbers: bool,
    line_pitch_pt: Option<f32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatsDto {
    paragraphs: usize,
    tables: usize,
    images: usize,
    runs: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BlockDto {
    kind: &'static str,
    /* heading 1-6 */
    level: Option<u8>,
    /* heading/paragraph/listItem 内容 */
    content: Option<Vec<InlineDto>>,
    /* 段落级版式 */
    align: Option<&'static str>,
    indent_left_pt: Option<f32>,
    indent_first_pt: Option<f32>,
    hanging_pt: Option<f32>,
    line_pct: Option<f32>,
    spacing_before_pt: Option<f32>,
    spacing_after_pt: Option<f32>,
    /* 列表项 */
    list: Option<ListDto>,
    /* 表格 */
    table: Option<TableDto>,
    /* 图片 */
    src: Option<String>,
    alt: Option<String>,
    width_px: Option<u32>,
    height_px: Option<u32>,
    section: Option<SectionDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListDto {
    level: u8,
    ordered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    marker_font: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InlineDto {
    kind: &'static str,
    text: Option<String>,
    /* 富字符样式 */
    font: Option<String>,
    size_half_pt: Option<u16>,
    color: Option<String>,
    highlight: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strike: Option<bool>,
    vert_align: Option<&'static str>,
    /* 链接 */
    href: Option<String>,
    /* 图片 */
    src: Option<String>,
    alt: Option<String>,
    width_px: Option<u32>,
    height_px: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TableDto {
    grid: Vec<Vec<SlotDto>>,
    header_rows: usize,
    col_widths_pct: Vec<f32>,
    width_pct: Option<f32>,
    fixed_layout: bool,
    indent_pt: Option<f32>,
    align: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SlotDto {
    kind: &'static str,
    cell: Option<CellDto>,
    origin_row: Option<usize>,
    origin_col: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CellDto {
    blocks: Vec<BlockDto>,
    col_span: u16,
    row_span: u16,
    is_header: bool,
    shading: Option<String>,
    width_pct: Option<f32>,
    valign: Option<&'static str>,
    margin_top_pt: Option<f32>,
    margin_right_pt: Option<f32>,
    margin_bottom_pt: Option<f32>,
    margin_left_pt: Option<f32>,
}

#[wasm_bindgen(js_name = toDocument)]
pub fn to_document(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let doc = Document::open(bytes).map_err(|e| JsValue::from_str(&format!("{e:#}")))?;
    let model = doc.model();
    let mut stats = StatsDto {
        paragraphs: 0,
        tables: 0,
        images: 0,
        runs: 0,
    };
    let blocks = convert_blocks(&model.blocks, &mut stats);
    let setup = section_dto_from_doc_setup(&model.setup);
    let dto = DocDto {
        blocks,
        setup,
        stats,
    };
    serde_wasm_bindgen::to_value(&dto).map_err(|e| JsValue::from_str(&format!("serialize: {e}")))
}

fn convert_blocks(blocks: &[Block], stats: &mut StatsDto) -> Vec<BlockDto> {
    blocks
        .iter()
        .filter_map(|b| convert_block(b, stats))
        .collect()
}

fn convert_block(block: &Block, stats: &mut StatsDto) -> Option<BlockDto> {
    match block {
        Block::Paragraph(p) => {
            stats.paragraphs += 1;
            stats.runs += p.runs.len();
            let mut dto = BlockDto {
                kind: "paragraph",
                level: None,
                content: None,
                align: align_str(p.props.align),
                indent_left_pt: p.props.indent.left_pt,
                indent_first_pt: p.props.indent.first_line_pt,
                hanging_pt: p.props.indent.hanging_pt,
                line_pct: p.props.spacing.line_pct,
                spacing_before_pt: p.props.spacing.before_pt,
                spacing_after_pt: p.props.spacing.after_pt,
                list: None,
                table: None,
                src: None,
                alt: None,
                width_px: None,
                height_px: None,
                section: None,
            };
            let content: Vec<InlineDto> = p
                .runs
                .iter()
                .filter_map(|run| convert_run(run, stats))
                .collect();
            if let Some(list) = &p.props.list {
                dto.kind = "listItem";
                dto.list = Some(ListDto {
                    level: list.level,
                    ordered: list.ordered,
                    label: if list.label.is_empty() {
                        None
                    } else {
                        Some(list.label.clone())
                    },
                    marker_font: list.marker_font.clone(),
                });
            } else if let Some(level) = p.props.heading_level {
                dto.kind = "heading";
                dto.level = Some(level.clamp(1, 6));
            }
            dto.content = Some(content);
            Some(dto)
        }
        Block::Table(t) => {
            stats.tables += 1;
            Some(BlockDto {
                kind: "table",
                level: None,
                content: None,
                align: None,
                indent_left_pt: None,
                indent_first_pt: None,
                hanging_pt: None,
                line_pct: None,
                spacing_before_pt: None,
                spacing_after_pt: None,
                list: None,
                table: Some(convert_table(t, stats)),
                src: None,
                alt: None,
                width_px: None,
                height_px: None,
                section: None,
            })
        }
        Block::Image(img) => {
            stats.images += 1;
            image_dto(img).map(|(src, alt, w, h)| BlockDto {
                kind: "image",
                level: None,
                content: None,
                align: None,
                indent_left_pt: None,
                indent_first_pt: None,
                hanging_pt: None,
                line_pct: None,
                spacing_before_pt: None,
                spacing_after_pt: None,
                list: None,
                table: None,
                src,
                alt,
                width_px: w,
                height_px: h,
                section: None,
            })
        }
        Block::PageBreak => Some(BlockDto {
            kind: "pageBreak",
            level: None,
            content: None,
            align: None,
            indent_left_pt: None,
            indent_first_pt: None,
            hanging_pt: None,
            line_pct: None,
            spacing_before_pt: None,
            spacing_after_pt: None,
            list: None,
            table: None,
            src: None,
            alt: None,
            width_px: None,
            height_px: None,
            section: None,
        }),
        Block::SectionBreak(setup) => Some(BlockDto {
            kind: "sectionBreak",
            level: None,
            content: None,
            align: None,
            indent_left_pt: None,
            indent_first_pt: None,
            hanging_pt: None,
            line_pct: None,
            spacing_before_pt: None,
            spacing_after_pt: None,
            list: None,
            table: None,
            src: None,
            alt: None,
            width_px: None,
            height_px: None,
            section: Some(section_dto_from_section_setup(setup)),
        }),
        Block::Chart(c) => Some(BlockDto {
            kind: "paragraph",
            level: None,
            content: Some(vec![InlineDto {
                kind: "text",
                text: Some(format!(
                    "[图表:{}]",
                    c.alt.clone().or(c.title.clone()).unwrap_or_default()
                )),
                italic: Some(true),
                ..empty_inline()
            }]),
            align: None,
            indent_left_pt: None,
            indent_first_pt: None,
            hanging_pt: None,
            line_pct: None,
            spacing_before_pt: None,
            spacing_after_pt: None,
            list: None,
            table: None,
            src: None,
            alt: None,
            width_px: None,
            height_px: None,
            section: None,
        }),
    }
}

fn empty_inline() -> InlineDto {
    InlineDto {
        kind: "text",
        text: None,
        font: None,
        size_half_pt: None,
        color: None,
        highlight: None,
        bold: None,
        italic: None,
        underline: None,
        strike: None,
        vert_align: None,
        href: None,
        src: None,
        alt: None,
        width_px: None,
        height_px: None,
    }
}

fn convert_run(run: &Run, stats: &mut StatsDto) -> Option<InlineDto> {
    if let Some(img) = &run.image {
        stats.images += 1;
        let (src, alt, w, h) = image_dto(img)?;
        let mut dto = empty_inline();
        dto.kind = "image";
        dto.src = src;
        dto.alt = alt;
        dto.width_px = w;
        dto.height_px = h;
        return Some(dto);
    }
    if run.text.is_empty() {
        return None;
    }
    let mut dto = empty_inline();
    dto.text = Some(run.text.clone());
    let cp = &run.props;
    if cp.font.is_some() {
        dto.font = cp.font.clone();
    }
    dto.size_half_pt = cp.size_half_pt;
    dto.color = cp.color.as_ref().map(color_hex);
    dto.highlight = cp.highlight.clone();
    if cp.bold {
        dto.bold = Some(true);
    }
    if cp.italic {
        dto.italic = Some(true);
    }
    if cp.underline {
        dto.underline = Some(true);
    }
    if cp.strike {
        dto.strike = Some(true);
    }
    dto.vert_align = match cp.vert_align {
        VertAlign::Baseline => None,
        VertAlign::Super => Some("super"),
        VertAlign::Sub => Some("sub"),
    };
    if let rwml::FieldRole::Hyperlink { url } = &run.field {
        dto.kind = "link";
        dto.href = Some(url.clone());
    }
    Some(dto)
}

fn page_dto(page: &rwml::PageSetup) -> PageDto {
    PageDto {
        width_pt: page.width_pt,
        height_pt: page.height_pt,
        left_pt: page.left(),
        right_pt: page.right(),
        top_pt: page.top(),
        bottom_pt: page.bottom(),
        landscape: page.landscape,
    }
}

fn running_blocks(blocks: &[Block]) -> Vec<BlockDto> {
    let mut stats = StatsDto {
        paragraphs: 0,
        tables: 0,
        images: 0,
        runs: 0,
    };
    convert_blocks(blocks, &mut stats)
        .into_iter()
        .filter(|block| block.kind != "pageBreak" && block.kind != "sectionBreak")
        .collect()
}

fn section_dto(
    page: &rwml::PageSetup,
    header: &[Block],
    first_header: &[Block],
    even_header: &[Block],
    footer: &[Block],
    first_footer: &[Block],
    even_footer: &[Block],
    title_page: bool,
    page_numbers: bool,
    doc_grid: Option<&rwml::DocGrid>,
) -> SectionDto {
    SectionDto {
        page: page_dto(page),
        header: running_blocks(header),
        first_header: running_blocks(first_header),
        even_header: running_blocks(even_header),
        footer: running_blocks(footer),
        first_footer: running_blocks(first_footer),
        even_footer: running_blocks(even_footer),
        title_page,
        page_numbers,
        line_pitch_pt: doc_grid
            .and_then(|grid| grid.line_pitch)
            .map(|twips| twips as f32 / 20.0),
    }
}

fn section_dto_from_doc_setup(setup: &rwml::DocSetup) -> SectionDto {
    section_dto(
        &setup.page,
        &setup.header,
        &setup.first_header,
        &setup.even_header,
        &setup.footer,
        &setup.first_footer,
        &setup.even_footer,
        setup.title_page,
        setup.page_numbers,
        setup.doc_grid.as_ref(),
    )
}

fn section_dto_from_section_setup(setup: &rwml::SectionSetup) -> SectionDto {
    section_dto(
        &setup.page,
        &setup.header,
        &setup.first_header,
        &setup.even_header,
        &setup.footer,
        &setup.first_footer,
        &setup.even_footer,
        setup.title_page,
        setup.page_numbers,
        setup.doc_grid.as_ref(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_page_geometry_and_asymmetric_margins() {
        let page = rwml::PageSetup {
            width_pt: 720.0,
            height_pt: 540.0,
            margin_pt: 36.0,
            margin_left_pt: Some(54.0),
            margin_right_pt: Some(45.0),
            margin_top_pt: Some(27.0),
            margin_bottom_pt: Some(63.0),
            landscape: true,
        };
        let dto = page_dto(&page);
        assert_eq!((dto.width_pt, dto.height_pt), (720.0, 540.0));
        assert_eq!(
            (dto.left_pt, dto.right_pt, dto.top_pt, dto.bottom_pt),
            (54.0, 45.0, 27.0, 63.0)
        );
        assert!(dto.landscape);
    }

    #[test]
    fn preserves_manual_line_break_runs() {
        let run = Run {
            text: "\n".into(),
            ..Default::default()
        };
        let mut stats = StatsDto {
            paragraphs: 0,
            tables: 0,
            images: 0,
            runs: 0,
        };
        let dto = convert_run(&run, &mut stats).expect("manual break should remain visible");
        assert_eq!(dto.text.as_deref(), Some("\n"));
    }
}

fn image_dto(
    img: &rwml::Image,
) -> Option<(Option<String>, Option<String>, Option<u32>, Option<u32>)> {
    let src = match (&img.bytes, &img.mime) {
        (Some(bytes), Some(mime)) => Some(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )),
        _ => None,
    };
    Some((src, img.alt.clone(), img.width_px, img.height_px))
}

fn color_hex(c: &rwml::Color) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn align_str(a: Align) -> Option<&'static str> {
    match a {
        Align::Left => None,
        Align::Center => Some("center"),
        Align::Right => Some("right"),
        Align::Justify => Some("justify"),
    }
}

/// rows/cells + spans → 规范网格:origin 槽携带单元格,covered 槽回指 origin
fn convert_table(t: &Table, stats: &mut StatsDto) -> TableDto {
    let rows = t.rows.len();
    let mut cols = 0usize;
    // 先模拟一遍确定总列数(考虑跨行占位)
    let mut occupied: Vec<Vec<bool>> = vec![vec![]; rows];
    let reserve = |occupied: &mut Vec<Vec<bool>>, r: usize, c: usize| {
        while occupied[r].len() <= c {
            occupied[r].push(false);
        }
    };
    for (ri, row) in t.rows.iter().enumerate() {
        let mut ci = 0usize;
        for cell in &row.cells {
            reserve(&mut occupied, ri, ci);
            while occupied[ri][ci] {
                ci += 1;
                reserve(&mut occupied, ri, ci);
            }
            let (rs, cs) = (cell.row_span.max(1) as usize, cell.col_span.max(1) as usize);
            for dr in 0..rs {
                for dc in 0..cs {
                    let r = ri + dr;
                    while r >= occupied.len() {
                        occupied.push(vec![]);
                    }
                    reserve(&mut occupied, r, ci + dc);
                    occupied[r][ci + dc] = true;
                }
            }
            cols = cols.max(ci + cs);
            ci += cs;
        }
    }

    let mut grid: Vec<Vec<SlotDto>> = occupied
        .iter()
        .map(|row| {
            (0..cols.max(row.len()))
                .map(|_| SlotDto {
                    kind: "covered",
                    cell: None,
                    origin_row: None,
                    origin_col: None,
                })
                .collect()
        })
        .collect();

    let mut covered: Vec<Vec<bool>> = grid.iter().map(|r| vec![false; r.len()]).collect();
    for (ri, row) in t.rows.iter().enumerate() {
        let mut ci = 0usize;
        for cell in &row.cells {
            while ci < covered[ri].len() && covered[ri][ci] {
                ci += 1;
            }
            if ci >= covered[ri].len() {
                break;
            }
            let (rs, cs) = (cell.row_span.max(1) as usize, cell.col_span.max(1) as usize);
            for dr in 0..rs {
                for dc in 0..cs {
                    if ri + dr < covered.len() && ci + dc < covered[ri + dr].len() {
                        covered[ri + dr][ci + dc] = true;
                    }
                }
            }
            grid[ri][ci] = SlotDto {
                kind: "origin",
                cell: Some(CellDto {
                    blocks: convert_blocks(&cell.blocks, stats),
                    col_span: cs as u16,
                    row_span: rs as u16,
                    is_header: cell.is_header,
                    shading: cell.shading.as_ref().map(color_hex),
                    width_pct: cell.width_pct,
                    valign: match cell.valign {
                        rwml::VCell::Top => None,
                        rwml::VCell::Center => Some("middle"),
                        rwml::VCell::Bottom => Some("bottom"),
                    },
                    margin_top_pt: cell.margins.map(|margins| margins.top as f32 / 20.0),
                    margin_right_pt: cell.margins.map(|margins| margins.right as f32 / 20.0),
                    margin_bottom_pt: cell.margins.map(|margins| margins.bottom as f32 / 20.0),
                    margin_left_pt: cell.margins.map(|margins| margins.left as f32 / 20.0),
                }),
                origin_row: None,
                origin_col: None,
            };
            ci += cs;
        }
    }

    TableDto {
        grid,
        header_rows: t.header_rows,
        col_widths_pct: t.col_widths_pct.clone(),
        width_pct: t.width_pct,
        fixed_layout: t.fixed_layout,
        indent_pt: t.indent_twips.map(|v| v as f32 / 20.0),
        align: t.align.and_then(align_str),
    }
}
