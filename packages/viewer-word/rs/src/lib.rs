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
    #[serde(skip_serializing_if = "Option::is_none")]
    default_tab_stop_pt: Option<f32>,
    typography: TypographyDto,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TypographyDto {
    balance_single_byte_double_byte_width: bool,
    underline_trailing_spaces: bool,
    compress_punctuation: bool,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    break_type: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    header_distance_pt: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    footer_distance_pt: Option<f32>,
    page_number_start: Option<u32>,
    page_number_format: Option<String>,
    columns: Option<u16>,
    text_direction: Option<String>,
    column_gap_pt: Option<f32>,
    column_widths_pt: Vec<f32>,
    column_spaces_pt: Vec<f32>,
    column_separator: bool,
    columns_rtl: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StatsDto {
    paragraphs: usize,
    tables: usize,
    images: usize,
    runs: usize,
}

#[derive(Serialize, Default)]
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
    indent_right_pt: Option<f32>,
    indent_first_pt: Option<f32>,
    hanging_pt: Option<f32>,
    line_pct: Option<f32>,
    spacing_before_pt: Option<f32>,
    spacing_after_pt: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_break_before: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pagination: Option<PaginationDto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_spacing: Option<LineSpacingDto>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tab_stops: Vec<TabStopDto>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    shading: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bidi: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rotation_degrees: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chart: Option<ChartDto>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    column_break_offsets: Vec<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChartDto {
    kind: String,
    title: Option<String>,
    alt: Option<String>,
    categories: Vec<String>,
    series: Vec<ChartSeriesDto>,
    width_px: Option<u32>,
    height_px: Option<u32>,
    wireframe: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChartSeriesDto { name: String, values: Vec<f64>, bubble_sizes: Vec<f64> }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PaginationDto {
    keep_next: bool,
    keep_lines: bool,
    widow_control: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LineSpacingDto {
    rule: &'static str,
    value_pt: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TabStopDto {
    position_pt: f32,
    alignment: &'static str,
    leader: &'static str,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    anchor: Option<ImageAnchorDto>,
    caps: Option<bool>,
    small_caps: Option<bool>,
    rtl: Option<bool>,
    rotation_degrees: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<PageFieldDto>,
}

#[derive(Serialize)]
struct PageFieldDto { kind: String, format: Option<String> }

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageAnchorDto {
    horizontal_from: &'static str,
    vertical_from: &'static str,
    x_pt: f64,
    y_pt: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TableDto {
    grid: Vec<Vec<SlotDto>>,
    header_rows: usize,
    col_widths_pct: Vec<f32>,
    width_pct: Option<f32>,
    fixed_layout: bool,
    row_min_heights_pt: Vec<Option<f32>>,
    indent_pt: Option<f32>,
    align: Option<&'static str>,
    bidi_visual: bool,
    row_cant_split: Vec<bool>,
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
    borders: CellBordersDto,
}

#[derive(Serialize)]
struct CellBordersDto {
    top: BorderDto,
    right: BorderDto,
    bottom: BorderDto,
    left: BorderDto,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BorderDto {
    style: &'static str,
    color: String,
    width_pt: f32,
}

fn cell_border_dto(border: Option<rwml::CellBorder>) -> BorderDto {
    match border {
        Some(border) if border.style != "none" && border.width_eighths > 0 => BorderDto {
            style: border.style,
            color: border
                .color
                .as_ref()
                .map(color_hex)
                .unwrap_or_else(|| "#000000".into()),
            width_pt: border.width_eighths as f32 / 8.0,
        },
        _ => BorderDto {
            style: "none",
            color: "#000000".into(),
            width_pt: 0.0,
        },
    }
}

fn legacy_table_border(table: &Table, side: rwml::TableBorderSide) -> Option<rwml::CellBorder> {
    let style = table.border_styles.get(side).or(table.border_style)?;
    Some(rwml::CellBorder {
        style: match style {
            rwml::TableBorderStyle::Single => "solid",
            rwml::TableBorderStyle::Dotted => "dotted",
            rwml::TableBorderStyle::Dashed => "dashed",
            rwml::TableBorderStyle::Double => "double",
        },
        color: table.border_colors.get(side).or(table.border_color),
        width_eighths: table
            .border_sizes
            .get(side)
            .or(table.border_size_eighths)
            .unwrap_or(4),
    })
}

#[wasm_bindgen(js_name = toDocument)]
pub fn to_document(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let doc = Document::open(bytes).map_err(|e| JsValue::from_str(&format!("{e:#}")))?;
    let dto = convert_document(&doc);
    serde_wasm_bindgen::to_value(&dto).map_err(|e| JsValue::from_str(&format!("serialize: {e}")))
}

fn convert_document(doc: &Document) -> DocDto {
    let layout = doc.model_with_layout_hints();
    let model = &layout.model;
    let mut stats = StatsDto {
        paragraphs: 0,
        tables: 0,
        images: 0,
        runs: 0,
    };
    let mut section_index = 0;
    let blocks = model
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(index, source)| {
            let mut dto = convert_block(source, &mut stats)?;
            if let Some(hint) = layout.paragraphs.get(index) {
                apply_layout_hint(&mut dto, hint);
                if let Block::Paragraph(paragraph) = source {
                    dto.column_break_offsets = hint.column_break_offsets.iter().map(|offset| {
                        let mut consumed = 0;
                        let mut hidden = 0;
                        for run in &paragraph.runs {
                            let count = run.text.chars().count().min(offset.saturating_sub(consumed));
                            if run.props.hidden { hidden += count; }
                            consumed += count;
                            if consumed >= *offset { break; }
                        }
                        offset.saturating_sub(hidden)
                    }).collect();
                }
            }
            if let Some(section) = dto.section.as_mut() {
                if let Some(columns) = layout.section_columns.get(section_index) {
                    apply_section_columns(section, columns);
                }
                if let Some(hints) = layout.running_paragraphs.get(section_index) {
                    apply_running_layout_hints(section, hints);
                }
                if let Some(&(header, footer)) = layout.running_distances.get(section_index) {
                    section.header_distance_pt = header;
                    section.footer_distance_pt = footer;
                }
                if let Some(Some(kind)) = layout.section_break_types.get(section_index) {
                    section.break_type = Some(kind);
                }
                section_index += 1;
            }
            Some(dto)
        })
        .collect();
    let mut setup = section_dto_from_doc_setup(&model.setup);
    if let Some(columns) = layout.section_columns.get(section_index) {
        apply_section_columns(&mut setup, columns);
    }
    if let Some(hints) = layout.running_paragraphs.get(section_index) {
        apply_running_layout_hints(&mut setup, hints);
    }
    if let Some(&(header, footer)) = layout.running_distances.get(section_index) {
        setup.header_distance_pt = header;
        setup.footer_distance_pt = footer;
    }
    DocDto {
        blocks,
        setup,
        stats,
        default_tab_stop_pt: layout.default_tab_stop_pt,
        typography: TypographyDto {
            balance_single_byte_double_byte_width: layout.typography.balance_single_byte_double_byte_width,
            underline_trailing_spaces: layout.typography.underline_trailing_spaces,
            compress_punctuation: layout.typography.compress_punctuation,
        },
    }
}

fn apply_layout_hint(dto: &mut BlockDto, hint: &rwml::PreviewParagraphLayout) {
    if let Some(table) = dto.table.as_mut() {
        table.row_cant_split = hint.table_row_cant_split.clone();
        for (row, hints) in table.grid.iter_mut().zip(&hint.table_cells) {
            for (cell, hints) in row
                .iter_mut()
                .filter_map(|slot| slot.cell.as_mut())
                .zip(hints)
            {
                for (block, hint) in cell.blocks.iter_mut().zip(hints) {
                    apply_layout_hint(block, hint);
                }
            }
        }
        return;
    }
    if !matches!(dto.kind, "paragraph" | "heading" | "listItem") {
        return;
    }
    dto.pagination = Some(PaginationDto {
        keep_next: hint.keep_next,
        keep_lines: hint.keep_lines,
        widow_control: hint.widow_control,
    });
    dto.line_spacing = hint.line_spacing.and_then(|(rule, value_pt)| {
        (value_pt.is_finite() && value_pt > 0.0).then_some(LineSpacingDto { rule, value_pt })
    });
    dto.tab_stops = hint
        .tab_stops
        .iter()
        .filter_map(|stop| {
            stop.position_pt.is_finite().then_some(TabStopDto {
                position_pt: stop.position_pt,
                alignment: stop.alignment,
                leader: stop.leader,
            })
        })
        .collect();
}

fn apply_section_columns(dto: &mut SectionDto, columns: &rwml::PreviewSectionColumns) {
    dto.column_gap_pt = columns.gap_pt;
    dto.column_widths_pt = columns.widths_pt.clone();
    dto.column_spaces_pt = columns.spaces_pt.clone();
    dto.column_separator = columns.separator;
    dto.columns_rtl = columns.rtl;
}

fn apply_running_layout_hints(
    section: &mut SectionDto,
    hints: &rwml::PreviewRunningParagraphLayouts,
) {
    for (blocks, hints) in [
        (&mut section.header, &hints.header),
        (&mut section.first_header, &hints.first_header),
        (&mut section.even_header, &hints.even_header),
        (&mut section.footer, &hints.footer),
        (&mut section.first_footer, &hints.first_footer),
        (&mut section.even_footer, &hints.even_footer),
    ] {
        for (block, hint) in blocks.iter_mut().zip(hints) {
            apply_layout_hint(block, hint);
        }
    }
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
                indent_right_pt: p.props.indent.right_pt,
                indent_first_pt: p.props.indent.first_line_pt,
                hanging_pt: p.props.indent.hanging_pt,
                line_pct: p.props.spacing.line_pct,
                spacing_before_pt: p.props.spacing.before_pt,
                spacing_after_pt: p.props.spacing.after_pt,
                page_break_before: p.props.page_break_before.then_some(true),
                pagination: None,
                line_spacing: None,
                tab_stops: Vec::new(),
                list: None,
                table: None,
                src: None,
                alt: None,
                width_px: None,
                height_px: None,
                section: None,
                shading: p.props.shading.as_ref().map(color_hex),
                bidi: p.props.bidi.then_some(true),
                ..Default::default()
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
                indent_right_pt: None,
                indent_first_pt: None,
                hanging_pt: None,
                line_pct: None,
                spacing_before_pt: None,
                spacing_after_pt: None,
                page_break_before: None,
                pagination: None,
                line_spacing: None,
                tab_stops: Vec::new(),
                list: None,
                table: Some(convert_table(t, stats)),
                src: None,
                alt: None,
                width_px: None,
                height_px: None,
                section: None,
                ..Default::default()
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
                indent_right_pt: None,
                indent_first_pt: None,
                hanging_pt: None,
                line_pct: None,
                spacing_before_pt: None,
                spacing_after_pt: None,
                page_break_before: None,
                pagination: None,
                line_spacing: None,
                tab_stops: Vec::new(),
                list: None,
                table: None,
                src,
                alt,
                width_px: w,
                height_px: h,
                section: None,
                rotation_degrees: img.rotation_degrees,
                ..Default::default()
            })
        }
        Block::PageBreak => Some(BlockDto {
            kind: "pageBreak",
            level: None,
            content: None,
            align: None,
            indent_left_pt: None,
            indent_right_pt: None,
            indent_first_pt: None,
            hanging_pt: None,
            line_pct: None,
            spacing_before_pt: None,
            spacing_after_pt: None,
            page_break_before: None,
            pagination: None,
            line_spacing: None,
            tab_stops: Vec::new(),
            list: None,
            table: None,
            src: None,
            alt: None,
            width_px: None,
            height_px: None,
            section: None,
            ..Default::default()
        }),
        Block::SectionBreak(setup) => Some(BlockDto {
            kind: "sectionBreak",
            level: None,
            content: None,
            align: None,
            indent_left_pt: None,
            indent_right_pt: None,
            indent_first_pt: None,
            hanging_pt: None,
            line_pct: None,
            spacing_before_pt: None,
            spacing_after_pt: None,
            page_break_before: None,
            pagination: None,
            line_spacing: None,
            tab_stops: Vec::new(),
            list: None,
            table: None,
            src: None,
            alt: None,
            width_px: None,
            height_px: None,
            section: Some(section_dto_from_section_setup(setup)),
            ..Default::default()
        }),
        Block::Chart(c) => Some(BlockDto {
            kind: "chart",
            chart: Some(ChartDto {
                kind: lower_camel(&format!("{:?}", c.kind)),
                title: c.title.clone(), alt: c.alt.clone(), categories: c.categories.clone(),
                series: c.series.iter().map(|series| ChartSeriesDto {
                    name: series.name.clone(), values: series.values.clone(), bubble_sizes: series.bubble_sizes.clone(),
                }).collect(),
                width_px: c.width_px, height_px: c.height_px, wireframe: c.wireframe,
            }),
            ..Default::default()
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
        anchor: None,
        caps: None, small_caps: None, rtl: None, rotation_degrees: None, field: None,
    }
}

fn convert_run(run: &Run, stats: &mut StatsDto) -> Option<InlineDto> {
    if run.props.hidden { return None; }
    if let Some(img) = &run.image {
        stats.images += 1;
        let (src, alt, w, h) = image_dto(img)?;
        let mut dto = empty_inline();
        dto.kind = "image";
        dto.src = src;
        dto.alt = alt;
        dto.width_px = w;
        dto.height_px = h;
        dto.rotation_degrees = img.rotation_degrees;
        dto.anchor = img.drawing_layout.as_ref().and_then(|layout| layout.anchor).map(|anchor| ImageAnchorDto {
            horizontal_from: anchor.horizontal_from,
            vertical_from: anchor.vertical_from,
            x_pt: anchor.x_emu as f64 / 12_700.0,
            y_pt: anchor.y_emu as f64 / 12_700.0,
        });
        return Some(dto);
    }
    let mut dto = empty_inline();
    if let rwml::FieldRole::Simple { instruction } = &run.field {
        let parts: Vec<_> = instruction.split_whitespace().collect();
        let kind = parts.first().map(|part| part.to_ascii_uppercase()).unwrap_or_default();
        if matches!(kind.as_str(), "PAGE" | "NUMPAGES" | "SECTIONPAGES") {
            let format = parts.windows(2).find(|pair| pair[0] == "\\*")
                .map(|pair| pair[1].to_string()).filter(|value| value != "MERGEFORMAT");
            dto.field = Some(PageFieldDto { kind, format });
        }
    }
    if run.text.is_empty() && dto.field.is_none() {
        return None;
    }
    dto.text = Some(run.text.clone());
    let cp = &run.props;
    if cp.font.is_some() {
        dto.font = cp.font.clone();
    }
    dto.size_half_pt = cp.size_half_pt;
    dto.color = cp.color.as_ref().map(color_hex);
    dto.highlight = cp.highlight.clone();
    dto.caps = cp.caps.then_some(true);
    dto.small_caps = cp.small_caps.then_some(true);
    dto.rtl = cp.rtl.then_some(true);
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
        break_type: None,
        header_distance_pt: None,
        footer_distance_pt: None,
        page_number_start: None, page_number_format: None, columns: None, text_direction: None,
        column_gap_pt: None, column_widths_pt: Vec::new(), column_spaces_pt: Vec::new(),
        column_separator: false, columns_rtl: false,
    }
}

fn section_dto_from_doc_setup(setup: &rwml::DocSetup) -> SectionDto {
    let mut dto = section_dto(
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
    );
    dto.page_number_start = setup.page_number_start;
    dto.page_number_format = setup.page_number_format.map(|value| lower_camel(&format!("{value:?}")));
    dto.columns = setup.columns;
    dto.text_direction = setup.text_direction.map(|value| lower_camel(&format!("{value:?}")));
    dto
}

fn section_dto_from_section_setup(setup: &rwml::SectionSetup) -> SectionDto {
    let mut dto = section_dto(
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
    );
    dto.break_type = setup.section_break.map(|kind| match kind {
        rwml::SectionBreakKind::NextPage => "nextPage",
        rwml::SectionBreakKind::EvenPage => "evenPage",
        rwml::SectionBreakKind::OddPage => "oddPage",
    });
    dto.page_number_start = setup.page_number_start;
    dto.page_number_format = setup.page_number_format.map(|value| lower_camel(&format!("{value:?}")));
    dto.columns = setup.columns;
    dto.text_direction = setup.text_direction.map(|value| lower_camel(&format!("{value:?}")));
    dto
}

fn lower_camel(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map(|first| first.to_lowercase().collect::<String>() + chars.as_str()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_source_runs_are_not_exposed_to_preview() {
        let mut stats = StatsDto { paragraphs: 0, tables: 0, images: 0, runs: 0 };
        let run = Run {
            text: "hidden template instructions".into(),
            props: rwml::CharProps { hidden: true, ..Default::default() },
            ..Default::default()
        };
        assert!(convert_run(&run, &mut stats).is_none());
    }

    #[test]
    fn empty_cached_page_fields_remain_dynamic() {
        let mut stats = StatsDto { paragraphs: 0, tables: 0, images: 0, runs: 0 };
        for kind in ["PAGE", "NUMPAGES", "SECTIONPAGES"] {
            let mut run = Run {
                field: rwml::FieldRole::Simple { instruction: format!(" {kind} \\* roman ") },
                ..Default::default()
            };
            let dto = convert_run(&run, &mut stats).expect("An empty cache must retain the page field");
            assert_eq!(dto.text.as_deref(), Some(""));
            let field = dto.field.expect("The renderer needs the dynamic field identity");
            assert_eq!(field.kind, kind);
            assert_eq!(field.format.as_deref(), Some("roman"));
            run.props.hidden = true;
            assert!(convert_run(&run, &mut stats).is_none(), "Hidden page fields must remain hidden");
        }
    }

    #[test]
    fn ordinary_empty_runs_remain_filtered() {
        let mut stats = StatsDto { paragraphs: 0, tables: 0, images: 0, runs: 0 };
        assert!(convert_run(&Run::default(), &mut stats).is_none());
        let run = Run {
            field: rwml::FieldRole::Simple { instruction: "DATE".into() },
            ..Default::default()
        };
        assert!(convert_run(&run, &mut stats).is_none());
    }

    #[test]
    fn source_docx_visual_properties_survive_preview_conversion() {
        let paragraph = rwml::Paragraph {
            props: rwml::ParaProps { shading: Some(rwml::Color { r: 18, g: 52, b: 86 }), bidi: true, ..Default::default() },
            runs: vec![
                Run { text: "invisible".into(), props: rwml::CharProps { hidden: true, ..Default::default() }, ..Default::default() },
                Run { text: "visible".into(), props: rwml::CharProps { caps: true, small_caps: true, rtl: true, ..Default::default() }, ..Default::default() },
            ],
        };
        let mut model = rwml::DocModel::default();
        model.blocks = vec![Block::Paragraph(paragraph)];
        model.setup.columns = Some(2);
        model.setup.page_number_start = Some(4);
        model.setup.page_number_format = Some(rwml::PageNumberFormat::LowerRoman);
        model.setup.footer = vec![Block::Paragraph(rwml::Paragraph {
            runs: vec![Run { text: "cached".into(), field: rwml::FieldRole::Simple { instruction: "PAGE".into() }, ..Default::default() }],
            ..Default::default()
        })];
        let bytes = rwml::write_docx(&model);
        let document = Document::open(&bytes).unwrap();
        let dto = convert_document(&document);
        let paragraph = &dto.blocks[0];
        assert_eq!(paragraph.shading.as_deref(), Some("#123456"));
        assert_eq!(paragraph.bidi, Some(true));
        let runs = paragraph.content.as_ref().unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text.as_deref(), Some("visible"));
        assert_eq!((runs[0].caps, runs[0].small_caps, runs[0].rtl), (Some(true), Some(true), Some(true)));
        assert_eq!(dto.setup.columns, Some(2));
        assert_eq!(dto.setup.page_number_start, Some(4));
        assert_eq!(dto.setup.page_number_format.as_deref(), Some("lowerRoman"));
        assert!(dto.setup.footer.iter().flat_map(|block| block.content.iter().flatten())
            .any(|run| run.field.as_ref().is_some_and(|field| field.kind == "PAGE")));
    }

    fn sample_document(name: &str) -> Document {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../apps/word-view/public/samples")
            .join(name);
        Document::open(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn preview_keeps_source_pagination_and_running_distances() {
        let doc = sample_document("manual.docx");
        let dto = convert_document(&doc);
        assert!(
            dto.blocks.iter().any(|block| block.kind == "heading"
                && block
                    .pagination
                    .as_ref()
                    .is_some_and(|hint| hint.keep_next && hint.keep_lines)),
            "source heading rules must reach the browser paginator"
        );
        assert!(
            dto.blocks.iter().any(|block| block.kind == "paragraph"
                && block.content.as_ref().is_some_and(Vec::is_empty)),
            "source empty paragraphs must survive DTO conversion"
        );
        assert_eq!(dto.setup.header_distance_pt, Some(42.55));
        assert_eq!(dto.setup.footer_distance_pt, Some(49.6));

        let dto = convert_document(&sample_document("req.docx"));
        assert_eq!(dto.setup.header_distance_pt, Some(34.0));
        assert_eq!(dto.setup.footer_distance_pt, Some(34.0));
        let first_section = dto
            .blocks
            .iter()
            .find_map(|block| block.section.as_ref())
            .unwrap();
        assert_eq!(first_section.break_type, Some("oddPage"));
        assert!(dto.blocks.iter().any(|block| block
            .pagination
            .as_ref()
            .is_some_and(|hint| hint.widow_control)));
    }

    #[test]
    fn preview_passes_exact_line_height_and_paragraph_page_break() {
        let mut paragraph = rwml::Paragraph::default();
        paragraph.props.page_break_before = true;
        let mut stats = StatsDto {
            paragraphs: 0,
            tables: 0,
            images: 0,
            runs: 0,
        };
        let mut dto = convert_block(&Block::Paragraph(paragraph), &mut stats).unwrap();
        apply_layout_hint(
            &mut dto,
            &rwml::PreviewParagraphLayout {
                keep_next: true,
                keep_lines: true,
                widow_control: true,
                line_spacing: Some(("exact", 12.5)),
                ..Default::default()
            },
        );
        assert_eq!(dto.page_break_before, Some(true));
        let spacing = dto.line_spacing.unwrap();
        assert_eq!(spacing.rule, "exact");
        assert_eq!(spacing.value_pt, 12.5);
        assert!(dto.pagination.unwrap().keep_next);
    }

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
    fn preview_passes_real_table_grid_and_header_fill() {
        let dto = convert_document(&sample_document("styles-word.docx"));
        let table = dto
            .blocks
            .iter()
            .find_map(|block| block.table.as_ref())
            .unwrap();
        let header = table.grid[0][0].cell.as_ref().unwrap();
        assert_eq!(header.shading.as_deref(), Some("#ddebf7"));
        assert_eq!(header.borders.top.style, "solid");
        assert_eq!(header.borders.top.color, "#000000");
        assert_eq!(header.borders.top.width_pt, 0.5);
        let requirements = convert_document(&sample_document("req.docx"));
        let table = requirements
            .blocks
            .iter()
            .find_map(|block| block.table.as_ref())
            .unwrap();
        let cell = table.grid[0][0].cell.as_ref().unwrap();
        // Source table borders are `none`, but this cell directly supplies a line.
        assert_eq!(cell.borders.top.style, "solid");
        assert_eq!(cell.borders.top.width_pt, 0.5);
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
    let layout = img.drawing_layout.as_ref();
    Some((src, img.alt.clone(), layout.and_then(|value| value.width_px).or(img.width_px), layout.and_then(|value| value.height_px).or(img.height_px)))
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
                    borders: CellBordersDto {
                        top: cell_border_dto(cell.borders.top.or_else(|| {
                            legacy_table_border(
                                t,
                                if ri == 0 {
                                    rwml::TableBorderSide::Top
                                } else {
                                    rwml::TableBorderSide::InsideHorizontal
                                },
                            )
                        })),
                        right: cell_border_dto(cell.borders.right.or_else(|| {
                            legacy_table_border(
                                t,
                                if ci + cs >= cols {
                                    rwml::TableBorderSide::Right
                                } else {
                                    rwml::TableBorderSide::InsideVertical
                                },
                            )
                        })),
                        bottom: cell_border_dto(cell.borders.bottom.or_else(|| {
                            legacy_table_border(
                                t,
                                if ri + rs >= rows {
                                    rwml::TableBorderSide::Bottom
                                } else {
                                    rwml::TableBorderSide::InsideHorizontal
                                },
                            )
                        })),
                        left: cell_border_dto(cell.borders.left.or_else(|| {
                            legacy_table_border(
                                t,
                                if ci == 0 {
                                    rwml::TableBorderSide::Left
                                } else {
                                    rwml::TableBorderSide::InsideVertical
                                },
                            )
                        })),
                    },
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
        row_min_heights_pt: t.row_min_heights_pt.clone(),
        indent_pt: t.indent_twips.map(|v| v as f32 / 20.0),
        align: t.align.and_then(align_str),
        bidi_visual: t.bidi_visual,
        row_cant_split: Vec::new(),
    }
}
