//! excel-view-rs:umya-spreadsheet 读 xlsx(值+样式+列宽行高)→ DTO
use serde::Serialize;
use std::{collections::HashMap, io::Cursor};
use umya_spreadsheet::reader::xlsx::{cell_images::read as read_cell_images, read_reader};
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Doc {
    sheets: Vec<SheetDto>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SheetDto {
    name: String,
    highest_col: u32,
    highest_row: u32,
    merges: Vec<String>,
    col_widths: Vec<ColWidth>,
    row_heights: Vec<RowHeight>,
    cells: Vec<CellDto>,
    images: Vec<ImageDto>,
    shapes: Vec<ShapeDto>,
    frozen_columns: u32,
    frozen_rows: u32,
}

/// 列宽(Excel 字符单位,前端换算 px)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ColWidth {
    col: u32,
    width: f64,
}

/// 行高(pt)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RowHeight {
    row: u32,
    height: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CellDto {
    col: u32,
    row: u32,
    value: String,
    formula: Option<String>,
    font_name: Option<String>,
    font_size: Option<f64>,
    bold: bool,
    italic: bool,
    underline: bool,
    font_color: Option<String>,
    fill: Option<String>,
    h_align: Option<String>,
    v_align: Option<String>,
    wrap_text: bool,
    border_left: Option<String>,
    border_right: Option<String>,
    border_top: Option<String>,
    border_bottom: Option<String>,
    numfmt: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    rich_text: Vec<RichTextRunDto>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct RichTextRunDto {
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    font_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bold: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    underline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    font_color: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImageDto {
    col: u32,
    row: u32,
    to_col: Option<u32>,
    to_row: Option<u32>,
    width_px: Option<u32>,
    height_px: Option<u32>,
    data: String,
    name: String,
    fit_cell: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ShapeDto {
    col: u32,
    row: u32,
    to_col: Option<u32>,
    to_row: Option<u32>,
    width_px: Option<u32>,
    height_px: Option<u32>,
    offset_x_px: i32,
    offset_y_px: i32,
    name: String,
    geometry: String,
    fill: Option<String>,
    line_color: Option<String>,
    line_width_px: Option<f64>,
    paragraphs: Vec<ShapeTextParagraphDto>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ShapeTextParagraphDto {
    runs: Vec<RichTextRunDto>,
}

/// ARGB("00FFFF00") → "#FFFF00"(去 alpha 取后 6 位)
fn hex6(argb: &str) -> String {
    let s = argb.trim_start_matches('#');
    let tail = if s.len() >= 6 { &s[s.len() - 6..] } else { s };
    format!("#{tail}")
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen(js_name = "toDocument")]
pub fn to_document(bytes: &[u8]) -> Result<JsValue, JsValue> {
    // with_sheet_read=true:立即反序列化,否则访问 sheet 会 panic(umya 3.x 懒加载语义)
    let book = read_reader(Cursor::new(bytes), true)
        .map_err(|e| JsValue::from_str(&format!("read: {e}")))?;
    let wps_images = read_cell_images(Cursor::new(bytes))
        .map_err(|e| JsValue::from_str(&format!("read cell images: {e}")))?;
    let wps_images: HashMap<_, _> = wps_images
        .into_iter()
        .map(|image| (image.id.clone(), image))
        .collect();

    let theme = book.get_theme();
    let mut sheets = Vec::new();
    for sheet in book.get_sheet_collection().iter() {
        // 行/列级样式索引:Excel 继承顺序 cell > row > col
        let row_styles: std::collections::HashMap<u32, &umya_spreadsheet::structs::Style> = sheet
            .get_row_dimensions()
            .into_iter()
            .map(|r| (r.get_row_num(), r.style()))
            .collect();
        let col_styles: std::collections::HashMap<u32, &umya_spreadsheet::structs::Style> = sheet
            .get_column_dimensions()
            .iter()
            .map(|c| (c.get_col_num(), c.style()))
            .collect();

        let mut cells = Vec::new();
        let mut images = Vec::new();
        for cell in sheet.get_cells() {
            let st = cell.get_style();
            let (cn, rn) = {
                let coord = cell.get_coordinate();
                (coord.get_col_num(), coord.get_row_num())
            };
            let row_st = row_styles.get(&rn);
            let col_st = col_styles.get(&cn);
            let (font_name, font_size, bold, italic, underline, font_color) = st
                .get_font()
                .or_else(|| row_st.and_then(|s| s.get_font()))
                .or_else(|| col_st.and_then(|s| s.get_font()))
                .map(|f| {
                    (
                        f.get_name().clone(),
                        f.get_size(),
                        f.get_bold(),
                        f.get_italic(),
                        !matches!(f.get_underline(), "" | "none"),
                        f.get_color().get_argb_with_theme(theme).to_string(),
                    )
                })
                .unwrap_or(("", 0.0, false, false, false, String::new()));
            // Excel solid fill 的显示色在 fgColor;bgColor 仅部分模式有意义,作回退
            let fill = st
                .get_fill()
                .or_else(|| row_st.and_then(|s| s.get_fill()))
                .or_else(|| col_st.and_then(|s| s.get_fill()))
                .and_then(|fl| fl.get_pattern_fill())
                .and_then(|p| {
                    p.get_foreground_color()
                        .or_else(|| p.get_background_color())
                })
                .map(|c| c.get_argb_with_theme(theme).to_string())
                .filter(|s| !s.is_empty() && s != "00000000");
            let (h_align, v_align, wrap_text) = st
                .get_alignment()
                .map(|a| {
                    (
                        format!("{:?}", a.get_horizontal()),
                        format!("{:?}", a.get_vertical()),
                        a.get_wrap_text(),
                    )
                })
                .map(|(h, v, w)| {
                    (
                        (h != "General").then_some(h),
                        (v != "Bottom").then_some(v),
                        w,
                    )
                })
                .unwrap_or((None, None, false));
            let style_of = |x: &umya_spreadsheet::structs::Border| {
                let s = x.get_border_style();
                (s != "none").then(|| s.to_string())
            };
            let (bl, br, bt, bb) = st
                .get_borders()
                .as_ref()
                .map(|b| {
                    (
                        style_of(b.get_left()),
                        style_of(b.get_right()),
                        style_of(b.get_top()),
                        style_of(b.get_bottom()),
                    )
                })
                .unwrap_or((None, None, None, None));
            let formula = (!cell.formula().is_empty()).then(|| cell.formula().to_string());
            let display_image = formula
                .as_deref()
                .and_then(extract_dispimg_id)
                .and_then(|id| wps_images.get(id));
            if let Some(image) = display_image {
                images.push(ImageDto {
                    col: cn,
                    row: rn,
                    to_col: None,
                    to_row: None,
                    width_px: Some(image.width_px),
                    height_px: Some(image.height_px),
                    data: data_uri(&image.image_name, &image.image_data),
                    name: image.image_name.clone(),
                    fit_cell: true,
                });
            }
            let rich_text = if display_image.is_some() {
                Vec::new()
            } else {
                rich_text_runs(cell, theme)
            };
            cells.push(CellDto {
                col: cell.get_coordinate().get_col_num(),
                row: cell.get_coordinate().get_row_num(),
                value: if display_image.is_some() {
                    String::new()
                } else {
                    cell.value().to_string()
                },
                formula,
                font_name: (!font_name.is_empty()).then(|| font_name.to_string()),
                font_size: (font_size > 0.0).then_some(font_size),
                bold,
                italic,
                underline,
                font_color: (!font_color.is_empty() && font_color != "00000000")
                    .then(|| hex6(&font_color)),
                fill: fill.as_deref().map(hex6),
                h_align,
                v_align,
                wrap_text,
                border_left: bl,
                border_right: br,
                border_top: bt,
                border_bottom: bb,
                numfmt: st.get_number_format().and_then(|n| {
                    (n.get_format_code() != "General").then(|| n.get_format_code().to_string())
                }),
                rich_text,
            });
        }
        for image in sheet.image_collection() {
            if !image.has_image() {
                continue;
            }
            let from = image.from_marker_type();
            let to = image.to_marker_type();
            let dimensions = image.one_cell_anchor().map(|anchor| {
                (
                    (anchor.extent().cx().max(0) as u64 / 9_525) as u32,
                    (anchor.extent().cy().max(0) as u64 / 9_525) as u32,
                )
            });
            images.push(ImageDto {
                col: from.col() + 1,
                row: from.row() + 1,
                to_col: to.map(|marker| marker.col() + 1),
                to_row: to.map(|marker| marker.row() + 1),
                width_px: dimensions.map(|value| value.0),
                height_px: dimensions.map(|value| value.1),
                data: format!(
                    "data:{};base64,{}",
                    mime_type(image.image_name()),
                    image.image_data_base64()
                ),
                name: image.image_name().to_string(),
                fit_cell: false,
            });
        }
        let shapes = shape_dtos(sheet);
        let shape_highest_col = shapes
            .iter()
            .map(|shape| shape.to_col.unwrap_or(shape.col))
            .max()
            .unwrap_or(0);
        let shape_highest_row = shapes
            .iter()
            .map(|shape| shape.to_row.unwrap_or(shape.row))
            .max()
            .unwrap_or(0);
        let pane = sheet
            .sheets_views()
            .sheet_view_list()
            .first()
            .and_then(|view| view.pane());
        sheets.push(SheetDto {
            name: sheet.get_name().to_string(),
            highest_col: sheet.get_highest_column().max(shape_highest_col),
            highest_row: sheet.get_highest_row().max(shape_highest_row),
            merges: sheet
                .get_merge_cells()
                .iter()
                .map(|m| m.get_range())
                .collect(),
            col_widths: sheet
                .get_column_dimensions()
                .iter()
                .map(|c| ColWidth {
                    col: c.get_col_num(),
                    width: c.get_width(),
                })
                .collect(),
            row_heights: sheet
                .get_row_dimensions()
                .into_iter()
                .map(|r| RowHeight {
                    row: r.get_row_num(),
                    height: r.get_height(),
                })
                .collect(),
            cells,
            images,
            shapes,
            frozen_columns: pane
                .map(|pane| pane.horizontal_split().max(0.0) as u32)
                .unwrap_or(0),
            frozen_rows: pane
                .map(|pane| pane.vertical_split().max(0.0) as u32)
                .unwrap_or(0),
        });
    }
    serde_wasm_bindgen::to_value(&Doc { sheets })
        .map_err(|e| JsValue::from_str(&format!("serialize: {e}")))
}

fn rich_text_runs(
    cell: &umya_spreadsheet::structs::Cell,
    theme: &umya_spreadsheet::structs::drawing::Theme,
) -> Vec<RichTextRunDto> {
    let Some(rich_text) = cell.rich_text() else {
        return Vec::new();
    };

    rich_text
        .rich_text_elements()
        .iter()
        .map(|element| {
            let font = element.font();
            let font_color = font
                .map(|value| value.color().argb_with_theme(theme).to_string())
                .filter(|value| !value.is_empty() && value != "00000000")
                .map(|value| hex6(&value));
            RichTextRunDto {
                text: element.text().to_string(),
                font_name: font
                    .map(|value| value.name())
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned),
                font_size: font.map(|value| value.size()).filter(|value| *value > 0.0),
                bold: font.map(|value| value.bold()),
                italic: font.map(|value| value.italic()),
                underline: font.map(|value| !matches!(value.underline(), "" | "none")),
                font_color,
            }
        })
        .collect()
}

fn shape_dtos(sheet: &umya_spreadsheet::structs::Worksheet) -> Vec<ShapeDto> {
    let mut shapes = Vec::new();
    let drawing = sheet.worksheet_drawing();

    for anchor in drawing.one_cell_anchor_collection() {
        if let Some(shape) = anchor.shape() {
            let from = anchor.from_marker();
            push_shape(
                &mut shapes,
                shape,
                from,
                None,
                Some((anchor.extent().cx(), anchor.extent().cy())),
            );
        }
    }
    for anchor in drawing.two_cell_anchor_collection() {
        if let Some(shape) = anchor.shape() {
            push_shape(
                &mut shapes,
                shape,
                anchor.from_marker(),
                Some(anchor.to_marker()),
                None,
            );
        }
    }
    shapes
}

fn push_shape(
    output: &mut Vec<ShapeDto>,
    shape: &umya_spreadsheet::drawing::spreadsheet::Shape,
    from: &umya_spreadsheet::drawing::spreadsheet::MarkerType,
    to: Option<&umya_spreadsheet::drawing::spreadsheet::MarkerType>,
    extent: Option<(i64, i64)>,
) {
    let non_visual = shape
        .non_visual_shape_properties()
        .non_visual_drawing_properties();
    if non_visual.hidden() {
        return;
    }
    let props = shape.shape_properties();
    let outline = props.outline();
    output.push(ShapeDto {
        col: from.col() + 1,
        row: from.row() + 1,
        to_col: to.map(|marker| marker.col() + 1),
        to_row: to.map(|marker| marker.row() + 1),
        width_px: extent.map(|value| emu_to_px(value.0)),
        height_px: extent.map(|value| emu_to_px(value.1)),
        offset_x_px: emu_to_px_signed(from.col_off()),
        offset_y_px: emu_to_px_signed(from.row_off()),
        name: non_visual.name().to_string(),
        geometry: props.geometry().geometry().to_string(),
        fill: props.solid_fill().and_then(drawing_fill_hex),
        line_color: outline
            .and_then(|value| value.solid_fill())
            .and_then(drawing_fill_hex),
        line_width_px: outline
            .map(|value| value.width())
            .filter(|value| *value > 0)
            .map(|value| value as f64 / 9_525.0),
        paragraphs: shape_text_paragraphs(shape),
    });
}

fn emu_to_px(value: i64) -> u32 {
    (value.max(0) as u64 / 9_525).min(u32::MAX as u64) as u32
}

fn emu_to_px_signed(value: i32) -> i32 {
    value / 9_525
}

fn drawing_fill_hex(fill: &umya_spreadsheet::structs::drawing::SolidFill) -> Option<String> {
    fill.rgb_color_model_hex()
        .map(|color| color.val())
        .filter(|value| !value.is_empty())
        .map(hex6)
}

fn shape_text_paragraphs(
    shape: &umya_spreadsheet::drawing::spreadsheet::Shape,
) -> Vec<ShapeTextParagraphDto> {
    let Some(body) = shape.text_body() else {
        return Vec::new();
    };
    body.paragraph()
        .iter()
        .map(|paragraph| ShapeTextParagraphDto {
            runs: paragraph
                .run()
                .iter()
                .map(|run| {
                    let props = run.run_properties();
                    let font_name = props
                        .latin_font()
                        .or_else(|| props.east_asian_font())
                        .or_else(|| props.complex_script_font())
                        .map(|font| font.typeface())
                        .filter(|value| !value.is_empty())
                        .map(str::to_owned);
                    RichTextRunDto {
                        text: run.text().to_string(),
                        font_name,
                        font_size: props
                            .sz()
                            .parse::<f64>()
                            .ok()
                            .map(|value| value / 100.0)
                            .filter(|value| *value > 0.0),
                        bold: drawing_bool(props.bold()),
                        italic: drawing_bool(props.italic()),
                        underline: None,
                        font_color: props.solid_fill().and_then(drawing_fill_hex),
                    }
                })
                .collect(),
        })
        .collect()
}

fn drawing_bool(value: &str) -> Option<bool> {
    if value.is_empty() {
        None
    } else {
        Some(!matches!(value, "0" | "false" | "off"))
    }
}

fn extract_dispimg_id(formula: &str) -> Option<&str> {
    let upper = formula.to_ascii_uppercase();
    let start = upper.find("DISPIMG(")? + "DISPIMG(".len();
    let rest = formula[start..].trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let value = &rest[quote.len_utf8()..];
    let end = value.find(quote)?;
    Some(&value[..end])
}

fn mime_type(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

fn data_uri(name: &str, bytes: &[u8]) -> String {
    format!("data:{};base64,{}", mime_type(name), base64(bytes))
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        result.push(ALPHABET[(a >> 2) as usize] as char);
        result.push(ALPHABET[(((a & 3) << 4) | (b >> 4)) as usize] as char);
        result.push(if chunk.len() > 1 {
            ALPHABET[(((b & 15) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            ALPHABET[(c & 63) as usize] as char
        } else {
            '='
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use umya_spreadsheet::drawing::spreadsheet::{Shape, TextBody};
    use umya_spreadsheet::structs::drawing::{Paragraph, Run, RunProperties, TextFontType};
    use umya_spreadsheet::structs::{Cell, Font, RichText, TextElement};

    #[test]
    fn extracts_wps_display_image_id() {
        assert_eq!(
            extract_dispimg_id(r#"_xlfn.DISPIMG("ID_6351",1)"#),
            Some("ID_6351")
        );
        assert_eq!(extract_dispimg_id("SUM(A1:A2)"), None);
    }

    #[test]
    fn preserves_rich_text_run_fonts_and_colors() {
        let mut rich = RichText::default();
        let mut plain = TextElement::default();
        plain.set_text("上线前:");
        rich.add_rich_text_elements(plain);

        let mut icon_font = Font::default();
        icon_font
            .set_name("Wingdings")
            .set_size(14.0)
            .set_bold(true);
        icon_font.color_mut().set_argb_str("FFFF0000");
        let mut icon = TextElement::default();
        icon.set_text("\u{f0d8}").set_font(icon_font);
        rich.add_rich_text_elements(icon);

        let mut cell = Cell::default();
        cell.set_rich_text(rich);
        let book = umya_spreadsheet::new_file();
        let runs = rich_text_runs(&cell, book.theme());

        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].text, "上线前:");
        assert_eq!(runs[0].font_name, None);
        assert_eq!(runs[1].text, "\u{f0d8}");
        assert_eq!(runs[1].font_name.as_deref(), Some("Wingdings"));
        assert_eq!(runs[1].font_size, Some(14.0));
        assert_eq!(runs[1].bold, Some(true));
        assert_eq!(runs[1].font_color.as_deref(), Some("#FF0000"));
    }

    #[test]
    fn preserves_shape_text_run_symbol_font() {
        let mut font = TextFontType::default();
        font.set_typeface("Wingdings");
        let mut props = RunProperties::default();
        props.set_latin_font(font);
        props.set_sz("1400");
        props.set_bold("1");
        let mut run = Run::default();
        run.set_text("\u{f0d8}");
        run.set_run_properties(props);
        let mut paragraph = Paragraph::default();
        paragraph.add_run(run);
        let mut body = TextBody::default();
        body.add_paragraph(paragraph);
        let mut shape = Shape::default();
        shape.set_text_body(body);

        let paragraphs = shape_text_paragraphs(&shape);

        assert_eq!(paragraphs.len(), 1);
        assert_eq!(paragraphs[0].runs[0].text, "\u{f0d8}");
        assert_eq!(
            paragraphs[0].runs[0].font_name.as_deref(),
            Some("Wingdings")
        );
        assert_eq!(paragraphs[0].runs[0].font_size, Some(14.0));
        assert_eq!(paragraphs[0].runs[0].bold, Some(true));
    }
}
