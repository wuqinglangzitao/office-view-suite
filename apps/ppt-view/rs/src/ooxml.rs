use ppt_rs::oxml::{XmlElement, XmlParser};
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Cursor, Read};

const DEFAULT_SLIDE_WIDTH: u64 = 9_144_000;
const DEFAULT_SLIDE_HEIGHT: u64 = 6_858_000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Doc {
    pub slide_width_emu: u64,
    pub slide_height_emu: u64,
    pub slides: Vec<Slide>,
    pub warnings: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Slide {
    pub hidden: bool,
    pub shapes: Vec<Shape>,
    pub tables: Vec<Table>,
    pub background: Option<String>,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Shape {
    pub kind: String,
    pub name: String,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    pub rotation: f32,
    pub flip_h: bool,
    pub flip_v: bool,
    pub shape_type: Option<String>,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width_emu: Option<i64>,
    pub dash: Option<String>,
    pub shadow: Option<Shadow>,
    pub head_end: Option<String>,
    pub tail_end: Option<String>,
    pub adjustments: Vec<i64>,
    pub custom_paths: Vec<CustomPath>,
    pub image: Option<String>,
    pub paragraphs: Vec<Para>,
    pub align: Option<String>,
    pub vertical_align: Option<String>,
    pub margin_left_emu: i64,
    pub margin_right_emu: i64,
    pub margin_top_emu: i64,
    pub margin_bottom_emu: i64,
    pub auto_fit: Option<String>,
    pub wrap_text: bool,
    pub font_scale: f32,
    pub line_spacing_reduction: f32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Shadow {
    pub color: String,
    pub blur_radius_emu: i64,
    pub distance_emu: i64,
    pub direction_deg: f32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CustomPath {
    pub width: i64,
    pub height: i64,
    pub data: String,
    pub fill: bool,
    pub stroke: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Para {
    pub level: u32,
    pub align: Option<String>,
    pub margin_left_emu: Option<i64>,
    pub margin_right_emu: Option<i64>,
    pub indent_emu: Option<i64>,
    pub line_spacing: Option<f32>,
    pub line_spacing_pt: Option<f32>,
    pub space_before: Option<f32>,
    pub space_before_pt: Option<f32>,
    pub space_after: Option<f32>,
    pub space_after_pt: Option<f32>,
    pub bullet: Option<String>,
    pub bullet_font: Option<String>,
    pub runs: Vec<Run>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub font: Option<String>,
    pub size_pt: Option<f32>,
    pub color: Option<String>,
    pub shadow: Option<Shadow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    pub columns: Vec<i64>,
    pub rows: Vec<TableRow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableRow {
    pub height_emu: i64,
    pub cells: Vec<TableCell>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableCell {
    pub text: String,
    pub col_span: u32,
    pub row_span: u32,
    pub fill: Option<String>,
    pub color: Option<String>,
    pub font_size_pt: Option<f32>,
    pub align: Option<String>,
    pub margin_left_emu: i64,
    pub margin_right_emu: i64,
    pub margin_top_emu: i64,
    pub margin_bottom_emu: i64,
}

#[derive(Clone, Copy, Default, Debug)]
struct BoxTransform {
    x: i64,
    y: i64,
    w: i64,
    h: i64,
    rotation: f32,
    flip_h: bool,
    flip_v: bool,
}

#[derive(Clone, Copy)]
struct CoordinateSpace {
    out_x: f64,
    out_y: f64,
    child_x: f64,
    child_y: f64,
    scale_x: f64,
    scale_y: f64,
}

impl Default for CoordinateSpace {
    fn default() -> Self {
        Self {
            out_x: 0.0,
            out_y: 0.0,
            child_x: 0.0,
            child_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
        }
    }
}

impl CoordinateSpace {
    fn map(&self, value: BoxTransform) -> BoxTransform {
        BoxTransform {
            x: (self.out_x + (value.x as f64 - self.child_x) * self.scale_x).round() as i64,
            y: (self.out_y + (value.y as f64 - self.child_y) * self.scale_y).round() as i64,
            w: (value.w as f64 * self.scale_x).round() as i64,
            h: (value.h as f64 * self.scale_y).round() as i64,
            ..value
        }
    }
}

#[derive(Clone, Default)]
struct Theme {
    colors: HashMap<String, String>,
    color_map: HashMap<String, String>,
    major_ea: Option<String>,
    minor_ea: Option<String>,
    effect_shadows: Vec<Option<Shadow>>,
}

#[derive(Clone, Default)]
struct TextDefaults {
    font: Option<String>,
    color: Option<String>,
}

#[derive(Clone)]
struct Rel {
    target: String,
    rel_type: String,
}

pub fn parse_document(bytes: &[u8]) -> Result<Doc, String> {
    let archive = Archive::open(bytes)?;
    let presentation = archive.xml("ppt/presentation.xml")?;
    let presentation_rels = archive.relationships("ppt/presentation.xml")?;
    let (slide_width_emu, slide_height_emu) = slide_size(&presentation);
    let slide_paths = slide_paths(&presentation, &presentation_rels, &archive);
    let mut slides = Vec::with_capacity(slide_paths.len());
    let mut warnings = Vec::new();

    for slide_path in slide_paths {
        let slide_root = archive.xml(&slide_path)?;
        let slide_rels = archive.relationships(&slide_path)?;
        let layout_path = relationship_target(&slide_rels, "slideLayout");
        let layout_root = layout_path.as_deref().and_then(|p| archive.xml(p).ok());
        let layout_rels = layout_path
            .as_deref()
            .and_then(|p| archive.relationships(p).ok())
            .unwrap_or_default();
        let master_path = relationship_target(&layout_rels, "slideMaster");
        let master_root = master_path.as_deref().and_then(|p| archive.xml(p).ok());
        let master_rels = master_path
            .as_deref()
            .and_then(|p| archive.relationships(p).ok())
            .unwrap_or_default();
        let theme_path = relationship_target(&master_rels, "theme")
            .or_else(|| archive.first_path("ppt/theme/theme"));
        let mut theme = theme_path
            .as_deref()
            .and_then(|p| archive.xml(p).ok())
            .map(|root| parse_theme(&root))
            .unwrap_or_default();
        if let Some(master) = master_root.as_ref() {
            theme.color_map = parse_color_map(master);
        }

        let mut placeholder_boxes = HashMap::new();
        if let Some(master) = master_root.as_ref() {
            collect_placeholder_boxes(master, &mut placeholder_boxes);
        }
        if let Some(layout) = layout_root.as_ref() {
            collect_placeholder_boxes(layout, &mut placeholder_boxes);
        }

        let background = background_fill(&slide_root, &theme)
            .or_else(|| {
                layout_root
                    .as_ref()
                    .and_then(|r| background_fill(r, &theme))
            })
            .or_else(|| {
                master_root
                    .as_ref()
                    .and_then(|r| background_fill(r, &theme))
            });

        let mut shapes = Vec::new();
        let mut tables = Vec::new();
        if let (Some(master), Some(path)) = (master_root.as_ref(), master_path.as_deref()) {
            parse_visual_tree(
                master,
                path,
                &master_rels,
                &archive,
                &theme,
                true,
                &placeholder_boxes,
                background.as_deref(),
                &mut shapes,
                &mut tables,
                &mut warnings,
            );
        }
        if let (Some(layout), Some(path)) = (layout_root.as_ref(), layout_path.as_deref()) {
            parse_visual_tree(
                layout,
                path,
                &layout_rels,
                &archive,
                &theme,
                true,
                &placeholder_boxes,
                background.as_deref(),
                &mut shapes,
                &mut tables,
                &mut warnings,
            );
        }
        parse_visual_tree(
            &slide_root,
            &slide_path,
            &slide_rels,
            &archive,
            &theme,
            false,
            &placeholder_boxes,
            background.as_deref(),
            &mut shapes,
            &mut tables,
            &mut warnings,
        );
        slides.push(Slide {
            hidden: is_hidden_slide(&slide_root),
            shapes,
            tables,
            background,
        });
    }

    warnings.sort();
    warnings.dedup();
    Ok(Doc {
        slide_width_emu,
        slide_height_emu,
        slides,
        warnings,
    })
}

struct Archive {
    parts: HashMap<String, Vec<u8>>,
}

impl Archive {
    fn open(bytes: &[u8]) -> Result<Self, String> {
        let mut zip =
            zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("open zip: {e}"))?;
        let mut parts = HashMap::new();
        for index in 0..zip.len() {
            let mut part = zip
                .by_index(index)
                .map_err(|e| format!("read zip entry: {e}"))?;
            if part.is_dir() {
                continue;
            }
            let mut data = Vec::new();
            part.read_to_end(&mut data)
                .map_err(|e| format!("read {}: {e}", part.name()))?;
            parts.insert(part.name().to_string(), data);
        }
        Ok(Self { parts })
    }

    fn xml(&self, path: &str) -> Result<XmlElement, String> {
        let bytes = self
            .parts
            .get(path)
            .ok_or_else(|| format!("missing part: {path}"))?;
        XmlParser::parse(bytes.as_slice()).map_err(|e| format!("parse {path}: {e}"))
    }

    fn relationships(&self, source_path: &str) -> Result<HashMap<String, Rel>, String> {
        let rels_path = relationships_path(source_path);
        let Some(bytes) = self.parts.get(&rels_path) else {
            return Ok(HashMap::new());
        };
        let root =
            XmlParser::parse(bytes.as_slice()).map_err(|e| format!("parse {rels_path}: {e}"))?;
        let mut result = HashMap::new();
        for rel in root.find_all("Relationship") {
            let (Some(id), Some(target)) = (rel.attr("Id"), rel.attr("Target")) else {
                continue;
            };
            result.insert(
                id.to_string(),
                Rel {
                    target: normalize_target(source_path, target),
                    rel_type: rel.attr("Type").unwrap_or_default().to_string(),
                },
            );
        }
        Ok(result)
    }

    fn first_path(&self, prefix: &str) -> Option<String> {
        let mut paths: Vec<_> = self
            .parts
            .keys()
            .filter(|p| p.starts_with(prefix) && p.ends_with(".xml"))
            .cloned()
            .collect();
        paths.sort();
        paths.into_iter().next()
    }
}

fn slide_size(root: &XmlElement) -> (u64, u64) {
    root.find_descendant("sldSz")
        .map(|s| {
            (
                attr_u64(s, "cx", DEFAULT_SLIDE_WIDTH),
                attr_u64(s, "cy", DEFAULT_SLIDE_HEIGHT),
            )
        })
        .unwrap_or((DEFAULT_SLIDE_WIDTH, DEFAULT_SLIDE_HEIGHT))
}

fn slide_paths(root: &XmlElement, rels: &HashMap<String, Rel>, archive: &Archive) -> Vec<String> {
    let mut result = Vec::new();
    if let Some(list) = root.find_descendant("sldIdLst") {
        for id in list.find_all("sldId") {
            if let Some(rel_id) = id.attr("r:id").or_else(|| id.attr("id")) {
                if let Some(rel) = rels.get(rel_id) {
                    result.push(rel.target.clone());
                }
            }
        }
    }
    if result.is_empty() {
        result = archive
            .parts
            .keys()
            .filter(|p| p.starts_with("ppt/slides/slide") && p.ends_with(".xml"))
            .cloned()
            .collect();
        result.sort_by_key(|p| numeric_suffix(p));
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn parse_visual_tree(
    root: &XmlElement,
    source_path: &str,
    rels: &HashMap<String, Rel>,
    archive: &Archive,
    theme: &Theme,
    skip_placeholders: bool,
    placeholder_boxes: &HashMap<String, BoxTransform>,
    background_fill: Option<&str>,
    shapes: &mut Vec<Shape>,
    tables: &mut Vec<Table>,
    warnings: &mut Vec<String>,
) {
    let Some(tree) = root.find_descendant("spTree") else {
        return;
    };
    parse_children(
        tree,
        source_path,
        rels,
        archive,
        theme,
        skip_placeholders,
        placeholder_boxes,
        CoordinateSpace::default(),
        None,
        background_fill,
        shapes,
        tables,
        warnings,
    );
}

#[allow(clippy::too_many_arguments)]
fn parse_children(
    parent: &XmlElement,
    source_path: &str,
    rels: &HashMap<String, Rel>,
    archive: &Archive,
    theme: &Theme,
    skip_placeholders: bool,
    placeholder_boxes: &HashMap<String, BoxTransform>,
    space: CoordinateSpace,
    inherited_fill: Option<String>,
    background_fill: Option<&str>,
    shapes: &mut Vec<Shape>,
    tables: &mut Vec<Table>,
    warnings: &mut Vec<String>,
) {
    for child in &parent.children {
        let placeholder = placeholder_key(child);
        if skip_placeholders && placeholder.is_some() {
            continue;
        }
        match child.local_name.as_str() {
            "sp" => shapes.push(parse_shape(
                child,
                theme,
                placeholder
                    .as_deref()
                    .and_then(|k| placeholder_boxes.get(k)),
                space,
                inherited_fill.as_deref(),
                background_fill,
            )),
            "pic" => {
                if let Some(shape) = parse_picture(child, rels, archive, theme, space, warnings) {
                    shapes.push(shape);
                }
            }
            "cxnSp" => shapes.push(parse_connector(child, theme, space)),
            "grpSp" => {
                let group_space = group_space(child, space);
                let group_fill = child
                    .find("grpSpPr")
                    .and_then(|properties| fill_css(properties, theme))
                    .or_else(|| inherited_fill.clone());
                parse_children(
                    child,
                    source_path,
                    rels,
                    archive,
                    theme,
                    skip_placeholders,
                    placeholder_boxes,
                    group_space,
                    group_fill,
                    background_fill,
                    shapes,
                    tables,
                    warnings,
                );
            }
            "graphicFrame" => {
                if let Some(table) = parse_table(child, theme, space) {
                    tables.push(table);
                }
            }
            _ => {}
        }
    }
}

fn parse_shape(
    element: &XmlElement,
    theme: &Theme,
    fallback: Option<&BoxTransform>,
    space: CoordinateSpace,
    inherited_fill: Option<&str>,
    background_fill: Option<&str>,
) -> Shape {
    let properties = element.find("spPr");
    let mut transform = properties
        .and_then(|p| p.find("xfrm"))
        .map(parse_transform)
        .or_else(|| fallback.copied())
        .unwrap_or_default();
    transform = space.map(transform);
    let body = element.find("txBody");
    let body_pr = body.and_then(|b| b.find("bodyPr"));
    let (auto_fit, font_scale, line_spacing_reduction) = text_fit(body_pr);
    let (mut fill, stroke, stroke_width_emu, dash) =
        shape_style(properties, element.find("style"), theme);
    if properties.is_some_and(|properties| properties.find("grpFill").is_some()) {
        fill = inherited_fill.map(str::to_string);
    }
    if is_true(element.attr("useBgFill")) {
        fill = Some(background_fill.unwrap_or("#FFFFFF").to_string());
    }
    let (head_end, tail_end) = line_ends(properties);
    let style = element.find("style");
    let shadow = resolved_shape_shadow(properties, style, theme);
    let adjustments = geometry_adjustments(properties);
    let custom_paths = parse_custom_paths(properties);
    let text_defaults = shape_text_defaults(style, theme);
    Shape {
        kind: "shape".into(),
        name: element_name(element),
        x: transform.x,
        y: transform.y,
        w: transform.w,
        h: transform.h,
        rotation: transform.rotation,
        flip_h: transform.flip_h,
        flip_v: transform.flip_v,
        shape_type: properties
            .and_then(|p| p.find("prstGeom"))
            .and_then(|g| g.attr("prst"))
            .map(str::to_string)
            .or_else(|| (!custom_paths.is_empty()).then(|| "custom".into())),
        fill,
        stroke,
        stroke_width_emu,
        dash,
        shadow,
        head_end,
        tail_end,
        adjustments,
        custom_paths,
        image: None,
        paragraphs: body
            .map(|b| parse_paragraphs(b, theme, Some(&text_defaults)))
            .unwrap_or_default(),
        align: None,
        vertical_align: body_pr.and_then(|p| p.attr("anchor")).map(vertical_align),
        margin_left_emu: body_pr
            .map(|p| attr_i64(p, "lIns", 91_440))
            .unwrap_or(91_440),
        margin_right_emu: body_pr
            .map(|p| attr_i64(p, "rIns", 91_440))
            .unwrap_or(91_440),
        margin_top_emu: body_pr
            .map(|p| attr_i64(p, "tIns", 45_720))
            .unwrap_or(45_720),
        margin_bottom_emu: body_pr
            .map(|p| attr_i64(p, "bIns", 45_720))
            .unwrap_or(45_720),
        auto_fit,
        wrap_text: body_pr.and_then(|p| p.attr("wrap")) != Some("none"),
        font_scale,
        line_spacing_reduction,
    }
}

fn parse_picture(
    element: &XmlElement,
    rels: &HashMap<String, Rel>,
    archive: &Archive,
    theme: &Theme,
    space: CoordinateSpace,
    warnings: &mut Vec<String>,
) -> Option<Shape> {
    let properties = element.find("spPr");
    let transform = space.map(
        properties
            .and_then(|p| p.find("xfrm"))
            .map(parse_transform)
            .unwrap_or_default(),
    );
    let rel_id = element
        .find_descendant("blip")?
        .attr("r:embed")
        .or_else(|| element.find_descendant("blip")?.attr("embed"))?;
    let rel = rels.get(rel_id)?;
    let bytes = archive.parts.get(&rel.target)?;
    let mime = mime_type(&rel.target);
    if mime.is_none() {
        warnings.push(format!("暂不支持图片格式: {}", extension(&rel.target)));
        return None;
    }
    let (fill, stroke, stroke_width_emu, dash) =
        shape_style(properties, element.find("style"), theme);
    Some(Shape {
        kind: "picture".into(),
        name: element_name(element),
        x: transform.x,
        y: transform.y,
        w: transform.w,
        h: transform.h,
        rotation: transform.rotation,
        flip_h: transform.flip_h,
        flip_v: transform.flip_v,
        shape_type: Some("rect".into()),
        fill,
        stroke,
        stroke_width_emu,
        dash,
        shadow: resolved_shape_shadow(properties, element.find("style"), theme),
        head_end: None,
        tail_end: None,
        adjustments: Vec::new(),
        custom_paths: Vec::new(),
        image: Some(format!("data:{};base64,{}", mime.unwrap(), base64(bytes))),
        paragraphs: Vec::new(),
        align: None,
        vertical_align: None,
        margin_left_emu: 0,
        margin_right_emu: 0,
        margin_top_emu: 0,
        margin_bottom_emu: 0,
        auto_fit: None,
        wrap_text: true,
        font_scale: 1.0,
        line_spacing_reduction: 0.0,
    })
}

fn parse_connector(element: &XmlElement, theme: &Theme, space: CoordinateSpace) -> Shape {
    let properties = element.find("spPr");
    let transform = space.map(
        properties
            .and_then(|p| p.find("xfrm"))
            .map(parse_transform)
            .unwrap_or_default(),
    );
    let (_, stroke, stroke_width_emu, dash) = shape_style(properties, element.find("style"), theme);
    let (head_end, tail_end) = line_ends(properties);
    Shape {
        kind: "line".into(),
        name: element_name(element),
        x: transform.x,
        y: transform.y,
        w: transform.w,
        h: transform.h,
        rotation: transform.rotation,
        flip_h: transform.flip_h,
        flip_v: transform.flip_v,
        shape_type: properties
            .and_then(|p| p.find("prstGeom"))
            .and_then(|g| g.attr("prst"))
            .map(str::to_string),
        fill: None,
        stroke: stroke.or_else(|| Some("#000000".into())),
        stroke_width_emu,
        dash,
        shadow: resolved_shape_shadow(properties, element.find("style"), theme),
        head_end,
        tail_end,
        adjustments: Vec::new(),
        custom_paths: Vec::new(),
        image: None,
        paragraphs: Vec::new(),
        align: None,
        vertical_align: None,
        margin_left_emu: 0,
        margin_right_emu: 0,
        margin_top_emu: 0,
        margin_bottom_emu: 0,
        auto_fit: None,
        wrap_text: true,
        font_scale: 1.0,
        line_spacing_reduction: 0.0,
    }
}

fn is_hidden_slide(root: &XmlElement) -> bool {
    matches!(root.attr("show"), Some("0" | "false"))
}

fn text_fit(body_pr: Option<&XmlElement>) -> (Option<String>, f32, f32) {
    let Some(body_pr) = body_pr else {
        return (None, 1.0, 0.0);
    };
    if body_pr.find("spAutoFit").is_some() {
        return (Some("resize".into()), 1.0, 0.0);
    }
    if let Some(auto_fit) = body_pr.find("normAutofit") {
        let font_scale = attr_i64(auto_fit, "fontScale", 100_000) as f32 / 100_000.0;
        let line_spacing_reduction = attr_i64(auto_fit, "lnSpcReduction", 0) as f32 / 100_000.0;
        return (
            Some("shrink".into()),
            font_scale.clamp(0.1, 1.0),
            line_spacing_reduction.clamp(0.0, 0.9),
        );
    }
    (None, 1.0, 0.0)
}

fn parse_table(element: &XmlElement, theme: &Theme, space: CoordinateSpace) -> Option<Table> {
    let table = element.find_descendant("tbl")?;
    let transform = space.map(
        element
            .find("xfrm")
            .map(parse_transform)
            .unwrap_or_default(),
    );
    let columns = table
        .find("tblGrid")
        .map(|grid| {
            grid.find_all("gridCol")
                .into_iter()
                .map(|c| attr_i64(c, "w", 0))
                .collect()
        })
        .unwrap_or_default();
    let mut rows = Vec::new();
    for row in table.find_all("tr") {
        let mut cells = Vec::new();
        for cell in row.find_all("tc") {
            if is_true(cell.attr("hMerge")) || is_true(cell.attr("vMerge")) {
                continue;
            }
            let paragraphs = cell
                .find("txBody")
                .map(|b| parse_paragraphs(b, theme, None))
                .unwrap_or_default();
            let text = paragraphs
                .iter()
                .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n");
            let first_run = paragraphs.first().and_then(|p| p.runs.first());
            let cell_pr = cell.find("tcPr");
            cells.push(TableCell {
                text,
                col_span: attr_u32(cell, "gridSpan", 1),
                row_span: attr_u32(cell, "rowSpan", 1),
                fill: cell_pr.and_then(|p| fill_css(p, theme)),
                color: first_run.and_then(|r| r.color.clone()),
                font_size_pt: first_run.and_then(|r| r.size_pt),
                align: paragraphs.first().and_then(|p| p.align.clone()),
                margin_left_emu: cell_pr
                    .map(|p| attr_i64(p, "marL", 91_440))
                    .unwrap_or(91_440),
                margin_right_emu: cell_pr
                    .map(|p| attr_i64(p, "marR", 91_440))
                    .unwrap_or(91_440),
                margin_top_emu: cell_pr
                    .map(|p| attr_i64(p, "marT", 45_720))
                    .unwrap_or(45_720),
                margin_bottom_emu: cell_pr
                    .map(|p| attr_i64(p, "marB", 45_720))
                    .unwrap_or(45_720),
            });
        }
        rows.push(TableRow {
            height_emu: attr_i64(row, "h", 0),
            cells,
        });
    }
    Some(Table {
        x: transform.x,
        y: transform.y,
        w: transform.w,
        h: transform.h,
        columns,
        rows,
    })
}

fn parse_paragraphs(
    body: &XmlElement,
    theme: &Theme,
    text_defaults: Option<&TextDefaults>,
) -> Vec<Para> {
    let mut result = Vec::new();
    let list_style = body.find("lstStyle");
    for p in body.find_all("p") {
        let p_pr = p.find("pPr");
        let level = p_pr.map(|v| attr_u32(v, "lvl", 0)).unwrap_or(0);
        let level_name = format!("lvl{}pPr", level + 1);
        let level_pr = list_style.and_then(|style| style.find(&level_name));
        let default_pr = list_style.and_then(|style| style.find("defPPr"));
        let mut fallbacks = Vec::new();
        if let Some(value) = p_pr.and_then(|value| value.find("defRPr")) {
            fallbacks.push(value);
        }
        if let Some(value) = level_pr.and_then(|value| value.find("defRPr")) {
            fallbacks.push(value);
        }
        if let Some(value) = default_pr.and_then(|value| value.find("defRPr")) {
            fallbacks.push(value);
        }
        if let Some(value) = p.find("endParaRPr") {
            fallbacks.push(value);
        }
        let mut runs = Vec::new();
        for child in &p.children {
            match child.local_name.as_str() {
                "r" | "fld" => {
                    let text = child
                        .find("t")
                        .map(|t| t.text_content())
                        .unwrap_or_else(|| child.text_content());
                    if text.is_empty() {
                        continue;
                    }
                    runs.push(parse_run(
                        &text,
                        child.find("rPr").or_else(|| child.find("defRPr")),
                        &fallbacks,
                        theme,
                        text_defaults,
                    ));
                }
                "br" => runs.push(parse_run(
                    "\n",
                    child.find("rPr"),
                    &fallbacks,
                    theme,
                    text_defaults,
                )),
                _ => {}
            }
        }
        if !runs.is_empty() {
            let (line_spacing, line_spacing_pt) =
                inherited_paragraph_spacing(p_pr, level_pr, default_pr, "lnSpc");
            let (space_before, space_before_pt) =
                inherited_paragraph_spacing(p_pr, level_pr, default_pr, "spcBef");
            let (space_after, space_after_pt) =
                inherited_paragraph_spacing(p_pr, level_pr, default_pr, "spcAft");
            result.push(Para {
                level,
                align: inherited_paragraph_attr(p_pr, level_pr, default_pr, "algn")
                    .map(paragraph_align),
                margin_left_emu: inherited_paragraph_attr(p_pr, level_pr, default_pr, "marL")
                    .and_then(|value| value.parse().ok()),
                margin_right_emu: inherited_paragraph_attr(p_pr, level_pr, default_pr, "marR")
                    .and_then(|value| value.parse().ok()),
                indent_emu: inherited_paragraph_attr(p_pr, level_pr, default_pr, "indent")
                    .and_then(|value| value.parse().ok()),
                line_spacing,
                line_spacing_pt,
                space_before,
                space_before_pt,
                space_after,
                space_after_pt,
                bullet: paragraph_bullet(p_pr, level_pr, default_pr),
                bullet_font: paragraph_bullet_font(p_pr, level_pr, default_pr, theme),
                runs,
            });
        }
    }
    result
}

fn parse_run(
    text: &str,
    explicit: Option<&XmlElement>,
    fallbacks: &[&XmlElement],
    theme: &Theme,
    text_defaults: Option<&TextDefaults>,
) -> Run {
    let east_asian = inherited_font(explicit, fallbacks, "ea");
    let latin = inherited_font(explicit, fallbacks, "latin");
    let font = if contains_east_asian(text) {
        east_asian.or(latin)
    } else {
        latin.or(east_asian)
    }
    .map(|name| theme_font(name, theme))
    .or_else(|| text_defaults.and_then(|defaults| defaults.font.clone()));
    Run {
        text: text.to_string(),
        bold: inherited_attr(explicit, fallbacks, "b")
            .map(is_true_value)
            .unwrap_or(false),
        italic: inherited_attr(explicit, fallbacks, "i")
            .map(is_true_value)
            .unwrap_or(false),
        underline: inherited_attr(explicit, fallbacks, "u")
            .map(|v| v != "none")
            .unwrap_or(false),
        font,
        size_pt: inherited_attr(explicit, fallbacks, "sz")
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| v / 100.0),
        color: explicit
            .and_then(|p| fill_css(p, theme))
            .or_else(|| fallbacks.iter().find_map(|p| fill_css(p, theme)))
            .or_else(|| text_defaults.and_then(|defaults| defaults.color.clone())),
        shadow: explicit
            .and_then(|p| parse_shadow(Some(p), theme))
            .or_else(|| fallbacks.iter().find_map(|p| parse_shadow(Some(p), theme))),
    }
}

fn inherited_attr<'a>(
    explicit: Option<&'a XmlElement>,
    fallbacks: &[&'a XmlElement],
    name: &str,
) -> Option<&'a str> {
    explicit
        .and_then(|value| value.attr(name))
        .or_else(|| fallbacks.iter().find_map(|value| value.attr(name)))
}

fn inherited_font<'a>(
    explicit: Option<&'a XmlElement>,
    fallbacks: &[&'a XmlElement],
    kind: &str,
) -> Option<&'a str> {
    explicit
        .and_then(|value| value.find(kind))
        .and_then(|value| value.attr("typeface"))
        .or_else(|| {
            fallbacks
                .iter()
                .find_map(|value| value.find(kind).and_then(|font| font.attr("typeface")))
        })
}

fn contains_east_asian(text: &str) -> bool {
    text.chars().any(|character| {
        matches!(
            character as u32,
            0x2E80..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF
        )
    })
}

fn paragraph_spacing(properties: Option<&XmlElement>, name: &str) -> (Option<f32>, Option<f32>) {
    let spacing = properties.and_then(|properties| properties.find(name));
    let percent = spacing
        .and_then(|spacing| spacing.find("spcPct"))
        .and_then(|value| value.attr("val"))
        .and_then(|value| value.parse::<f32>().ok())
        .map(|value| value / 100_000.0);
    let points = spacing
        .and_then(|spacing| spacing.find("spcPts"))
        .and_then(|value| value.attr("val"))
        .and_then(|value| value.parse::<f32>().ok())
        .map(|value| value / 100.0);
    (percent, points)
}

fn inherited_paragraph_attr<'a>(
    properties: Option<&'a XmlElement>,
    level_properties: Option<&'a XmlElement>,
    default_properties: Option<&'a XmlElement>,
    name: &str,
) -> Option<&'a str> {
    [properties, level_properties, default_properties]
        .into_iter()
        .flatten()
        .find_map(|properties| properties.attr(name))
}

fn inherited_paragraph_spacing(
    properties: Option<&XmlElement>,
    level_properties: Option<&XmlElement>,
    default_properties: Option<&XmlElement>,
    name: &str,
) -> (Option<f32>, Option<f32>) {
    [properties, level_properties, default_properties]
        .into_iter()
        .flatten()
        .find(|properties| properties.find(name).is_some())
        .map(|properties| paragraph_spacing(Some(properties), name))
        .unwrap_or((None, None))
}

fn paragraph_bullet(
    properties: Option<&XmlElement>,
    level_properties: Option<&XmlElement>,
    default_properties: Option<&XmlElement>,
) -> Option<String> {
    for properties in [properties, level_properties, default_properties]
        .into_iter()
        .flatten()
    {
        if properties.find("buNone").is_some() {
            return None;
        }
        if let Some(character) = properties
            .find("buChar")
            .and_then(|bullet| bullet.attr("char"))
        {
            return Some(character.to_string());
        }
    }
    None
}

fn paragraph_bullet_font(
    properties: Option<&XmlElement>,
    level_properties: Option<&XmlElement>,
    default_properties: Option<&XmlElement>,
    theme: &Theme,
) -> Option<String> {
    for properties in [properties, level_properties, default_properties]
        .into_iter()
        .flatten()
    {
        if properties.find("buNone").is_some() || properties.find("buFontTx").is_some() {
            return None;
        }
        if let Some(typeface) = properties
            .find("buFont")
            .and_then(|font| font.attr("typeface"))
        {
            return Some(theme_font(typeface, theme));
        }
    }
    None
}

fn shape_style(
    properties: Option<&XmlElement>,
    style: Option<&XmlElement>,
    theme: &Theme,
) -> (Option<String>, Option<String>, Option<i64>, Option<String>) {
    let fill = properties
        .and_then(|p| fill_css(p, theme))
        .or_else(|| style_color(style, "fillRef", theme));
    let line = properties.and_then(|p| p.find("ln"));
    let stroke = match line {
        Some(line) if line.find("noFill").is_some() => None,
        Some(line) => fill_css(line, theme).or_else(|| style_color(style, "lnRef", theme)),
        None => style_color(style, "lnRef", theme),
    };
    let width = line.and_then(|l| l.attr("w")).and_then(|v| v.parse().ok());
    let dash = line
        .and_then(|l| l.find("prstDash"))
        .and_then(|d| d.attr("val"))
        .map(str::to_string);
    (fill, stroke, width, dash)
}

fn style_color(style: Option<&XmlElement>, reference: &str, theme: &Theme) -> Option<String> {
    let color_ref = style.and_then(|style| style.find(reference))?;
    if color_ref.attr("idx") == Some("0") {
        return None;
    }
    color_css(color_ref, theme)
}

fn shape_text_defaults(style: Option<&XmlElement>, theme: &Theme) -> TextDefaults {
    let font_ref = style.and_then(|style| style.find("fontRef"));
    let font = match font_ref.and_then(|font| font.attr("idx")) {
        Some("major") => theme.major_ea.clone(),
        Some("minor") => theme.minor_ea.clone(),
        _ => None,
    };
    TextDefaults {
        font,
        color: font_ref.and_then(|font| color_css(font, theme)),
    }
}

fn line_ends(properties: Option<&XmlElement>) -> (Option<String>, Option<String>) {
    let line = properties.and_then(|p| p.find("ln"));
    let end_type = |name: &str| {
        line.and_then(|l| l.find(name))
            .and_then(|end| end.attr("type"))
            .filter(|value| *value != "none")
            .map(str::to_string)
    };
    (end_type("headEnd"), end_type("tailEnd"))
}

fn parse_shadow(properties: Option<&XmlElement>, theme: &Theme) -> Option<Shadow> {
    let shadow = properties
        .and_then(|properties| properties.find("effectLst"))
        .and_then(|effects| effects.find("outerShdw"))?;
    Some(Shadow {
        color: color_css(shadow, theme)?,
        blur_radius_emu: attr_i64(shadow, "blurRad", 0),
        distance_emu: attr_i64(shadow, "dist", 0),
        direction_deg: attr_i64(shadow, "dir", 0) as f32 / 60_000.0,
    })
}

fn resolved_shape_shadow(
    properties: Option<&XmlElement>,
    style: Option<&XmlElement>,
    theme: &Theme,
) -> Option<Shadow> {
    parse_shadow(properties, theme).or_else(|| {
        let index = style
            .and_then(|style| style.find("effectRef"))
            .and_then(|effect| effect.attr("idx"))
            .and_then(|value| value.parse::<usize>().ok())?;
        index
            .checked_sub(1)
            .and_then(|index| theme.effect_shadows.get(index))
            .cloned()
            .flatten()
    })
}

fn geometry_adjustments(properties: Option<&XmlElement>) -> Vec<i64> {
    properties
        .and_then(|properties| properties.find("prstGeom"))
        .and_then(|geometry| geometry.find("avLst"))
        .map(|list| {
            list.children
                .iter()
                .filter(|guide| guide.local_name == "gd")
                .filter_map(|guide| {
                    guide
                        .attr("fmla")
                        .and_then(|formula| formula.strip_prefix("val "))
                        .and_then(|value| value.parse::<i64>().ok())
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_custom_paths(properties: Option<&XmlElement>) -> Vec<CustomPath> {
    let Some(path_list) = properties
        .and_then(|p| p.find("custGeom"))
        .and_then(|g| g.find("pathLst"))
    else {
        return Vec::new();
    };

    path_list
        .children
        .iter()
        .filter(|path| path.local_name == "path")
        .filter_map(|path| {
            let mut data = Vec::new();
            for command in &path.children {
                match command.local_name.as_str() {
                    "moveTo" | "lnTo" => {
                        let point = command.find("pt")?;
                        let x = point.attr("x")?.parse::<i64>().ok()?;
                        let y = point.attr("y")?.parse::<i64>().ok()?;
                        data.push(format!(
                            "{} {x} {y}",
                            if command.local_name == "moveTo" {
                                "M"
                            } else {
                                "L"
                            }
                        ));
                    }
                    "cubicBezTo" => {
                        let points: Vec<_> = command
                            .children
                            .iter()
                            .filter(|point| point.local_name == "pt")
                            .collect();
                        if points.len() != 3 {
                            return None;
                        }
                        let values: Option<Vec<_>> = points
                            .iter()
                            .map(|point| {
                                Some((
                                    point.attr("x")?.parse::<i64>().ok()?,
                                    point.attr("y")?.parse::<i64>().ok()?,
                                ))
                            })
                            .collect();
                        let values = values?;
                        data.push(format!(
                            "C {} {} {} {} {} {}",
                            values[0].0,
                            values[0].1,
                            values[1].0,
                            values[1].1,
                            values[2].0,
                            values[2].1
                        ));
                    }
                    "quadBezTo" => {
                        let points: Vec<_> = command
                            .children
                            .iter()
                            .filter(|point| point.local_name == "pt")
                            .collect();
                        if points.len() != 2 {
                            return None;
                        }
                        let x1 = points[0].attr("x")?.parse::<i64>().ok()?;
                        let y1 = points[0].attr("y")?.parse::<i64>().ok()?;
                        let x2 = points[1].attr("x")?.parse::<i64>().ok()?;
                        let y2 = points[1].attr("y")?.parse::<i64>().ok()?;
                        data.push(format!("Q {x1} {y1} {x2} {y2}"));
                    }
                    "close" => data.push("Z".into()),
                    _ => {}
                }
            }
            (!data.is_empty()).then(|| CustomPath {
                width: attr_i64(path, "w", 21_600),
                height: attr_i64(path, "h", 21_600),
                data: data.join(" "),
                fill: path.attr("fill") != Some("none"),
                stroke: !matches!(path.attr("stroke"), Some("0" | "false")),
            })
        })
        .collect()
}

fn fill_css(element: &XmlElement, theme: &Theme) -> Option<String> {
    if element.find("noFill").is_some() {
        return Some("transparent".into());
    }
    if let Some(solid) = element.find("solidFill") {
        return color_css(solid, theme);
    }
    if let Some(gradient) = element.find("gradFill") {
        let mut stops = Vec::new();
        if let Some(list) = gradient.find("gsLst") {
            for stop in list.find_all("gs") {
                if let Some(color) = color_css(stop, theme) {
                    stops.push(format!(
                        "{} {}%",
                        color,
                        attr_i64(stop, "pos", 0) as f64 / 1000.0
                    ));
                }
            }
        }
        if !stops.is_empty() {
            let angle = gradient
                .find("lin")
                .map(|l| 90.0 + attr_i64(l, "ang", 0) as f64 / 60_000.0)
                .unwrap_or(180.0);
            return Some(format!("linear-gradient({angle}deg,{})", stops.join(",")));
        }
    }
    None
}

fn color_css(element: &XmlElement, theme: &Theme) -> Option<String> {
    let color = element.children.iter().find(|c| {
        matches!(
            c.local_name.as_str(),
            "srgbClr" | "schemeClr" | "sysClr" | "prstClr"
        )
    })?;
    let base = match color.local_name.as_str() {
        "srgbClr" => color.attr("val")?.to_string(),
        "sysClr" => color
            .attr("lastClr")
            .or_else(|| color.attr("val"))?
            .to_string(),
        "schemeClr" => {
            let key = color.attr("val")?;
            let mapped = theme.color_map.get(key).map(String::as_str).unwrap_or(key);
            theme
                .colors
                .get(mapped)
                .cloned()
                .unwrap_or_else(|| "000000".into())
        }
        "prstClr" => preset_color(color.attr("val")?).to_string(),
        _ => return None,
    };
    let mut rgb = parse_hex(&base)?;
    let lum_mod = color
        .find("lumMod")
        .map(|v| attr_i64(v, "val", 100_000) as f64 / 100_000.0)
        .unwrap_or(1.0);
    let lum_off = color
        .find("lumOff")
        .map(|v| attr_i64(v, "val", 0) as f64 / 100_000.0)
        .unwrap_or(0.0);
    for value in &mut rgb {
        *value =
            ((*value as f64 / 255.0 * lum_mod + lum_off).clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    let alpha = color
        .find("alpha")
        .map(|v| attr_i64(v, "val", 100_000) as f64 / 100_000.0)
        .unwrap_or(1.0);
    if alpha < 0.999 {
        Some(format!("rgba({},{},{},{alpha:.3})", rgb[0], rgb[1], rgb[2]))
    } else {
        Some(format!("#{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]))
    }
}

fn parse_theme(root: &XmlElement) -> Theme {
    let mut theme = Theme::default();
    if let Some(scheme) = root.find_descendant("clrScheme") {
        for entry in &scheme.children {
            if let Some(color) = color_css(entry, &Theme::default()) {
                theme.colors.insert(
                    entry.local_name.clone(),
                    color.trim_start_matches('#').to_string(),
                );
            }
        }
    }
    theme.color_map = default_color_map();
    if let Some(major) = root.find_descendant("majorFont") {
        theme.major_ea = east_asian_theme_font(major);
    }
    if let Some(minor) = root.find_descendant("minorFont") {
        theme.minor_ea = east_asian_theme_font(minor);
    }
    theme.effect_shadows = root
        .find_descendant("effectStyleLst")
        .map(|list| {
            list.children
                .iter()
                .filter(|child| child.local_name == "effectStyle")
                .map(|effect| parse_shadow(Some(effect), &theme))
                .collect()
        })
        .unwrap_or_default();
    theme
}

fn east_asian_theme_font(element: &XmlElement) -> Option<String> {
    element
        .find("ea")
        .and_then(|v| v.attr("typeface"))
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .or_else(|| {
            element
                .find_all("font")
                .into_iter()
                .find(|v| v.attr("script") == Some("Hans"))
                .and_then(|v| v.attr("typeface"))
                .map(str::to_string)
        })
}

fn parse_color_map(master: &XmlElement) -> HashMap<String, String> {
    let mut result = default_color_map();
    if let Some(map) = master.find_descendant("clrMap") {
        for (key, value) in &map.attributes {
            result.insert(key.clone(), value.clone());
        }
    }
    result
}

fn default_color_map() -> HashMap<String, String> {
    [
        ("bg1", "lt1"),
        ("tx1", "dk1"),
        ("bg2", "lt2"),
        ("tx2", "dk2"),
        ("accent1", "accent1"),
        ("accent2", "accent2"),
        ("accent3", "accent3"),
        ("accent4", "accent4"),
        ("accent5", "accent5"),
        ("accent6", "accent6"),
        ("hlink", "hlink"),
        ("folHlink", "folHlink"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

fn collect_placeholder_boxes(root: &XmlElement, result: &mut HashMap<String, BoxTransform>) {
    let Some(tree) = root.find_descendant("spTree") else {
        return;
    };
    for shape in tree.find_all("sp") {
        let Some(key) = placeholder_key(shape) else {
            continue;
        };
        let Some(transform) = shape
            .find("spPr")
            .and_then(|p| p.find("xfrm"))
            .map(parse_transform)
        else {
            continue;
        };
        result.insert(key, transform);
    }
}

fn placeholder_key(element: &XmlElement) -> Option<String> {
    let ph = element.find_descendant("nvPr")?.find("ph")?;
    Some(format!(
        "{}:{}",
        ph.attr("type").unwrap_or("body"),
        ph.attr("idx").unwrap_or("0")
    ))
}

fn background_fill(root: &XmlElement, theme: &Theme) -> Option<String> {
    let background = root.find_descendant("bg")?;
    background
        .find("bgPr")
        .and_then(|properties| fill_css(properties, theme))
        .or_else(|| {
            background
                .find("bgRef")
                .and_then(|reference| color_css(reference, theme))
        })
}

fn parse_transform(xfrm: &XmlElement) -> BoxTransform {
    let off = xfrm.find("off");
    let ext = xfrm.find("ext");
    BoxTransform {
        x: off.map(|v| attr_i64(v, "x", 0)).unwrap_or(0),
        y: off.map(|v| attr_i64(v, "y", 0)).unwrap_or(0),
        w: ext.map(|v| attr_i64(v, "cx", 0)).unwrap_or(0),
        h: ext.map(|v| attr_i64(v, "cy", 0)).unwrap_or(0),
        rotation: xfrm
            .attr("rot")
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| v / 60_000.0)
            .unwrap_or(0.0),
        flip_h: is_true(xfrm.attr("flipH")),
        flip_v: is_true(xfrm.attr("flipV")),
    }
}

fn group_space(group: &XmlElement, parent: CoordinateSpace) -> CoordinateSpace {
    let Some(xfrm) = group.find("grpSpPr").and_then(|p| p.find("xfrm")) else {
        return parent;
    };
    let mapped = parent.map(parse_transform(xfrm));
    let child_off = xfrm.find("chOff");
    let child_ext = xfrm.find("chExt");
    let child_w = child_ext
        .map(|v| attr_i64(v, "cx", mapped.w))
        .unwrap_or(mapped.w)
        .max(1);
    let child_h = child_ext
        .map(|v| attr_i64(v, "cy", mapped.h))
        .unwrap_or(mapped.h)
        .max(1);
    CoordinateSpace {
        out_x: mapped.x as f64,
        out_y: mapped.y as f64,
        child_x: child_off.map(|v| attr_i64(v, "x", 0) as f64).unwrap_or(0.0),
        child_y: child_off.map(|v| attr_i64(v, "y", 0) as f64).unwrap_or(0.0),
        scale_x: mapped.w as f64 / child_w as f64,
        scale_y: mapped.h as f64 / child_h as f64,
    }
}

fn relationship_target(rels: &HashMap<String, Rel>, needle: &str) -> Option<String> {
    rels.values()
        .find(|r| r.rel_type.ends_with(needle) || r.rel_type.contains(&format!("/{needle}")))
        .map(|r| r.target.clone())
}

fn relationships_path(source: &str) -> String {
    let (dir, file) = source.rsplit_once('/').unwrap_or(("", source));
    if dir.is_empty() {
        format!("_rels/{file}.rels")
    } else {
        format!("{dir}/_rels/{file}.rels")
    }
}

fn normalize_target(source: &str, target: &str) -> String {
    if let Some(path) = target.strip_prefix('/') {
        return path.to_string();
    }
    let mut segments: Vec<&str> = source
        .rsplit_once('/')
        .map(|(dir, _)| dir.split('/').collect())
        .unwrap_or_default();
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            value => segments.push(value),
        }
    }
    segments.join("/")
}

fn element_name(element: &XmlElement) -> String {
    element
        .find_descendant("cNvPr")
        .and_then(|v| v.attr("name"))
        .unwrap_or(element.local_name.as_str())
        .to_string()
}

fn theme_font(value: &str, theme: &Theme) -> String {
    match value {
        "+mj-ea" | "+mj-lt" => theme.major_ea.clone().unwrap_or_else(|| value.to_string()),
        "+mn-ea" | "+mn-lt" => theme.minor_ea.clone().unwrap_or_else(|| value.to_string()),
        _ => value.to_string(),
    }
}

fn paragraph_align(value: &str) -> String {
    match value {
        "ctr" => "center",
        "r" => "right",
        "just" | "dist" => "justify",
        _ => "left",
    }
    .into()
}

fn vertical_align(value: &str) -> String {
    match value {
        "ctr" => "center",
        "b" => "bottom",
        _ => "top",
    }
    .into()
}

fn preset_color(value: &str) -> &str {
    match value {
        "black" => "000000",
        "white" => "FFFFFF",
        "red" => "FF0000",
        "blue" => "0000FF",
        "green" => "008000",
        "yellow" => "FFFF00",
        "gray" | "grey" => "808080",
        _ => "000000",
    }
}

fn parse_hex(value: &str) -> Option<[u8; 3]> {
    let value = value.trim_start_matches('#');
    if value.len() != 6 {
        return None;
    }
    Some([
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ])
}

fn mime_type(path: &str) -> Option<&'static str> {
    match extension(path).as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "svg" => Some("image/svg+xml"),
        "bmp" => Some("image/bmp"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

fn extension(path: &str) -> String {
    path.rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0];
        let b = *chunk.get(1).unwrap_or(&0);
        let c = *chunk.get(2).unwrap_or(&0);
        result.push(ALPHABET[(a >> 2) as usize] as char);
        result.push(ALPHABET[(((a & 0x03) << 4) | (b >> 4)) as usize] as char);
        result.push(if chunk.len() > 1 {
            ALPHABET[(((b & 0x0f) << 2) | (c >> 6)) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            ALPHABET[(c & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    result
}

fn attr_i64(element: &XmlElement, name: &str, default: i64) -> i64 {
    element
        .attr(name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
fn attr_u64(element: &XmlElement, name: &str, default: u64) -> u64 {
    element
        .attr(name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
fn attr_u32(element: &XmlElement, name: &str, default: u32) -> u32 {
    element
        .attr(name)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
fn is_true(value: Option<&str>) -> bool {
    value.map(is_true_value).unwrap_or(false)
}
fn is_true_value(value: &str) -> bool {
    matches!(value, "1" | "true")
}
fn numeric_suffix(path: &str) -> u32 {
    path.trim_end_matches(".xml")
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_theme_color_with_luminance_modifiers() {
        let root = XmlParser::parse_str(r#"<a:solidFill xmlns:a="urn:a"><a:schemeClr val="accent1"><a:lumMod val="50000"/><a:lumOff val="20000"/></a:schemeClr></a:solidFill>"#).unwrap();
        let mut theme = Theme::default();
        theme.colors.insert("accent1".into(), "336699".into());
        theme.color_map = default_color_map();
        assert_eq!(color_css(&root, &theme).as_deref(), Some("#4D6680"));
    }

    #[test]
    fn maps_group_child_coordinates_to_slide_space() {
        let group = XmlParser::parse_str(r#"<p:grpSp xmlns:p="urn:p" xmlns:a="urn:a"><p:grpSpPr><a:xfrm><a:off x="100" y="200"/><a:ext cx="400" cy="200"/><a:chOff x="10" y="20"/><a:chExt cx="200" cy="100"/></a:xfrm></p:grpSpPr></p:grpSp>"#).unwrap();
        let space = group_space(&group, CoordinateSpace::default());
        let mapped = space.map(BoxTransform {
            x: 60,
            y: 45,
            w: 50,
            h: 25,
            ..Default::default()
        });
        assert_eq!(
            (mapped.x, mapped.y, mapped.w, mapped.h),
            (200, 250, 100, 50)
        );
    }

    #[test]
    fn base64_encodes_embedded_images_without_an_extra_dependency() {
        assert_eq!(base64(b"PowerPoint"), "UG93ZXJQb2ludA==");
    }

    #[test]
    fn detects_hidden_slides_from_show_attribute() {
        let hidden = XmlParser::parse_str(r#"<p:sld xmlns:p="urn:p" show="0"/>"#).unwrap();
        let visible = XmlParser::parse_str(r#"<p:sld xmlns:p="urn:p"/>"#).unwrap();
        assert!(is_hidden_slide(&hidden));
        assert!(!is_hidden_slide(&visible));
    }

    #[test]
    fn reads_source_text_autofit_semantics() {
        let shrink = XmlParser::parse_str(
            r#"<a:bodyPr xmlns:a="urn:a"><a:normAutofit fontScale="85000" lnSpcReduction="12000"/></a:bodyPr>"#,
        )
        .unwrap();
        let resize =
            XmlParser::parse_str(r#"<a:bodyPr xmlns:a="urn:a"><a:spAutoFit/></a:bodyPr>"#).unwrap();
        assert_eq!(text_fit(Some(&shrink)), (Some("shrink".into()), 0.85, 0.12));
        assert_eq!(text_fit(Some(&resize)), (Some("resize".into()), 1.0, 0.0));
    }

    #[test]
    fn preserves_connector_arrow_ends() {
        let root = XmlParser::parse_str(
            r#"<p:cxnSp xmlns:p="urn:p" xmlns:a="urn:a"><p:spPr><a:xfrm><a:off x="10" y="20"/><a:ext cx="30" cy="40"/></a:xfrm><a:prstGeom prst="straightConnector1"/><a:ln><a:headEnd type="arrow"/><a:tailEnd type="triangle"/></a:ln></p:spPr></p:cxnSp>"#,
        )
        .unwrap();
        let shape = parse_connector(&root, &Theme::default(), CoordinateSpace::default());
        assert_eq!(shape.head_end.as_deref(), Some("arrow"));
        assert_eq!(shape.tail_end.as_deref(), Some("triangle"));
    }

    #[test]
    fn converts_custom_geometry_to_svg_path_data() {
        let root = XmlParser::parse_str(
            r#"<p:sp xmlns:p="urn:p" xmlns:a="urn:a"><p:spPr><a:custGeom><a:pathLst><a:path w="100" h="80"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="100" y="0"/></a:lnTo><a:cubicBezTo><a:pt x="100" y="20"/><a:pt x="80" y="80"/><a:pt x="0" y="80"/></a:cubicBezTo><a:close/></a:path></a:pathLst></a:custGeom></p:spPr></p:sp>"#,
        )
        .unwrap();
        let shape = parse_shape(
            &root,
            &Theme::default(),
            None,
            CoordinateSpace::default(),
            None,
            None,
        );
        assert_eq!(shape.shape_type.as_deref(), Some("custom"));
        assert_eq!(shape.custom_paths.len(), 1);
        assert_eq!(
            shape.custom_paths[0].data,
            "M 0 0 L 100 0 C 100 20 80 80 0 80 Z"
        );
    }

    #[test]
    fn falls_back_to_shape_style_fill_for_custom_geometry() {
        let root = XmlParser::parse_str(
            r#"<p:sp xmlns:p="urn:p" xmlns:a="urn:a"><p:spPr><a:custGeom><a:pathLst><a:path w="10" h="10"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="10" y="10"/></a:lnTo></a:path></a:pathLst></a:custGeom></p:spPr><p:style><a:lnRef idx="0"><a:schemeClr val="accent1"/></a:lnRef><a:fillRef idx="1"><a:schemeClr val="accent5"><a:tint val="60000"/></a:schemeClr></a:fillRef></p:style></p:sp>"#,
        )
        .unwrap();
        let mut theme = Theme::default();
        theme.colors.insert("accent5".into(), "336699".into());
        theme.color_map = default_color_map();
        let shape = parse_shape(&root, &theme, None, CoordinateSpace::default(), None, None);
        assert!(shape.fill.is_some());
        assert!(shape.stroke.is_none());
    }

    #[test]
    fn uses_inherited_group_fill_for_group_filled_geometry() {
        let root = XmlParser::parse_str(
            r#"<p:sp xmlns:p="urn:p" xmlns:a="urn:a"><p:spPr><a:custGeom><a:pathLst><a:path w="10" h="10"><a:moveTo><a:pt x="0" y="0"/></a:moveTo><a:lnTo><a:pt x="10" y="10"/></a:lnTo></a:path></a:pathLst></a:custGeom><a:grpFill/></p:spPr></p:sp>"#,
        )
        .unwrap();
        let shape = parse_shape(
            &root,
            &Theme::default(),
            None,
            CoordinateSpace::default(),
            Some("#336699"),
            None,
        );
        assert_eq!(shape.fill.as_deref(), Some("#336699"));
    }

    #[test]
    fn uses_slide_background_fill_for_background_filled_shape() {
        let root = XmlParser::parse_str(
            r#"<p:sp xmlns:p="urn:p" xmlns:a="urn:a" useBgFill="1"><p:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:effectLst><a:outerShdw blurRad="215900" dist="50800" dir="5400000"><a:srgbClr val="000000"><a:alpha val="15000"/></a:srgbClr></a:outerShdw></a:effectLst></p:spPr></p:sp>"#,
        )
        .unwrap();
        let shape = parse_shape(
            &root,
            &Theme::default(),
            None,
            CoordinateSpace::default(),
            None,
            Some("#F4F5F6"),
        );
        assert_eq!(shape.fill.as_deref(), Some("#F4F5F6"));
        assert!(shape.shadow.is_some());
    }

    #[test]
    fn preserves_table_cell_margins() {
        let root = XmlParser::parse_str(
            r#"<p:graphicFrame xmlns:p="urn:p" xmlns:a="urn:a"><p:xfrm><a:off x="0" y="0"/><a:ext cx="1000" cy="500"/></p:xfrm><a:graphic><a:graphicData><a:tbl><a:tblGrid><a:gridCol w="1000"/></a:tblGrid><a:tr h="500"><a:tc><a:txBody><a:bodyPr/><a:p><a:r><a:rPr sz="800"/><a:t>事项</a:t></a:r></a:p></a:txBody><a:tcPr marL="13017" marR="26034" marT="39051" marB="0"/></a:tc></a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#,
        )
        .unwrap();
        let table = parse_table(&root, &Theme::default(), CoordinateSpace::default()).unwrap();
        let cell = &table.rows[0].cells[0];
        assert_eq!(cell.margin_left_emu, 13_017);
        assert_eq!(cell.margin_right_emu, 26_034);
        assert_eq!(cell.margin_top_emu, 39_051);
        assert_eq!(cell.margin_bottom_emu, 0);
    }

    #[test]
    fn preserves_rounding_adjustment_and_shape_shadow() {
        let root = XmlParser::parse_str(
            r#"<p:sp xmlns:p="urn:p" xmlns:a="urn:a"><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1000" cy="500"/></a:xfrm><a:prstGeom prst="roundRect"><a:avLst><a:gd name="adj" fmla="val 8528"/></a:avLst></a:prstGeom><a:effectLst><a:outerShdw blurRad="444500" dist="127000" dir="5400000"><a:srgbClr val="000000"><a:alpha val="25000"/></a:srgbClr></a:outerShdw></a:effectLst></p:spPr></p:sp>"#,
        )
        .unwrap();
        let shape = parse_shape(
            &root,
            &Theme::default(),
            None,
            CoordinateSpace::default(),
            None,
            None,
        );
        assert_eq!(shape.adjustments, vec![8_528]);
        assert_eq!(
            shape.shadow,
            Some(Shadow {
                color: "rgba(0,0,0,0.250)".into(),
                blur_radius_emu: 444_500,
                distance_emu: 127_000,
                direction_deg: 90.0,
            })
        );
    }

    #[test]
    fn resolves_shape_shadow_from_theme_effect_reference() {
        let theme_root = XmlParser::parse_str(
            r#"<a:theme xmlns:a="urn:a"><a:themeElements><a:clrScheme name="x"><a:dk1><a:srgbClr val="000000"/></a:dk1></a:clrScheme><a:fontScheme name="x"/><a:fmtScheme name="x"><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst><a:outerShdw blurRad="57150" dist="19050" dir="5400000"><a:srgbClr val="000000"><a:alpha val="63000"/></a:srgbClr></a:outerShdw></a:effectLst></a:effectStyle></a:effectStyleLst></a:fmtScheme></a:themeElements></a:theme>"#,
        )
        .unwrap();
        let theme = parse_theme(&theme_root);
        let root = XmlParser::parse_str(
            r#"<p:sp xmlns:p="urn:p" xmlns:a="urn:a"><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1000" cy="500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:style><a:effectRef idx="3"><a:schemeClr val="accent1"/></a:effectRef></p:style></p:sp>"#,
        )
        .unwrap();
        let shape = parse_shape(&root, &theme, None, CoordinateSpace::default(), None, None);
        assert_eq!(
            shape.shadow,
            Some(Shadow {
                color: "rgba(0,0,0,0.630)".into(),
                blur_radius_emu: 57_150,
                distance_emu: 19_050,
                direction_deg: 90.0,
            })
        );
    }

    #[test]
    fn inherits_shape_font_reference_when_runs_have_no_font() {
        let theme_root = XmlParser::parse_str(
            r#"<a:theme xmlns:a="urn:a"><a:themeElements><a:clrScheme name="x"><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1></a:clrScheme><a:fontScheme name="x"><a:majorFont><a:ea typeface="宋体"/></a:majorFont><a:minorFont><a:ea typeface="微软雅黑"/></a:minorFont></a:fontScheme><a:fmtScheme name="x"/></a:themeElements></a:theme>"#,
        )
        .unwrap();
        let theme = parse_theme(&theme_root);
        let root = XmlParser::parse_str(
            r#"<p:sp xmlns:p="urn:p" xmlns:a="urn:a"><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1000" cy="500"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr><p:style><a:fontRef idx="minor"><a:schemeClr val="lt1"/></a:fontRef></p:style><p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="zh-CN"/><a:t>多边形文字</a:t></a:r></a:p></p:txBody></p:sp>"#,
        )
        .unwrap();
        let shape = parse_shape(&root, &theme, None, CoordinateSpace::default(), None, None);
        assert_eq!(
            shape.paragraphs[0].runs[0].font.as_deref(),
            Some("微软雅黑")
        );
        assert_eq!(
            shape.paragraphs[0].runs[0].color.as_deref(),
            Some("#FFFFFF")
        );
    }

    #[test]
    fn inherits_paragraph_layout_from_list_level() {
        let body = XmlParser::parse_str(
            r#"<p:txBody xmlns:p="urn:p" xmlns:a="urn:a"><a:bodyPr/><a:lstStyle><a:lvl1pPr algn="ctr" marL="57150" indent="-57150"><a:lnSpc><a:spcPct val="90000"/></a:lnSpc><a:defRPr sz="1200"/></a:lvl1pPr></a:lstStyle><a:p><a:r><a:t>居中文字</a:t></a:r></a:p></p:txBody>"#,
        )
        .unwrap();
        let paragraphs = parse_paragraphs(&body, &Theme::default(), None);
        let paragraph = &paragraphs[0];
        assert_eq!(paragraph.align.as_deref(), Some("center"));
        assert_eq!(paragraph.margin_left_emu, Some(57_150));
        assert_eq!(paragraph.indent_emu, Some(-57_150));
        assert_eq!(paragraph.line_spacing, Some(0.9));
    }

    #[test]
    fn preserves_symbol_bullet_font() {
        let body = XmlParser::parse_str(
            r#"<p:txBody xmlns:p="urn:p" xmlns:a="urn:a"><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:buFont typeface="Wingdings"/><a:buChar char="Ø"/></a:pPr><a:r><a:rPr><a:ea typeface="微软雅黑"/></a:rPr><a:t>上线前：</a:t></a:r></a:p></p:txBody>"#,
        )
        .unwrap();
        let paragraphs = parse_paragraphs(&body, &Theme::default(), None);
        let paragraph = &paragraphs[0];
        assert_eq!(paragraph.bullet.as_deref(), Some("Ø"));
        assert_eq!(paragraph.bullet_font.as_deref(), Some("Wingdings"));
    }

    #[test]
    fn cascades_run_and_paragraph_text_properties() {
        let body = XmlParser::parse_str(
            r#"<p:txBody xmlns:p="urn:p" xmlns:a="urn:a"><a:bodyPr/><a:lstStyle/><a:p><a:pPr marL="285750" indent="-285750"><a:lnSpc><a:spcPct val="85000"/></a:lnSpc><a:spcAft><a:spcPts val="600"/></a:spcAft><a:defRPr sz="2400"><a:solidFill><a:srgbClr val="FFFFFF"/></a:solidFill><a:ea typeface="宋体"/><a:effectLst><a:outerShdw blurRad="38100" dist="19050" dir="2700000"><a:srgbClr val="000000"><a:alpha val="40000"/></a:srgbClr></a:outerShdw></a:effectLst></a:defRPr></a:pPr><a:r><a:rPr lang="zh-CN"/><a:t>多边形文字</a:t></a:r></a:p></p:txBody>"#,
        )
        .unwrap();
        let paragraphs = parse_paragraphs(&body, &Theme::default(), None);
        let paragraph = &paragraphs[0];
        let run = &paragraph.runs[0];
        assert_eq!(paragraph.margin_left_emu, Some(285_750));
        assert_eq!(paragraph.indent_emu, Some(-285_750));
        assert_eq!(paragraph.line_spacing, Some(0.85));
        assert_eq!(paragraph.space_after_pt, Some(6.0));
        assert_eq!(run.font.as_deref(), Some("宋体"));
        assert_eq!(run.size_pt, Some(24.0));
        assert_eq!(run.color.as_deref(), Some("#FFFFFF"));
        assert!(run.shadow.is_some());
    }
}
