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
    pub z_order: usize,
    pub kind: String,
    pub name: String,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    pub rotation: f32,
    pub flip_h: bool,
    pub flip_v: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<[f64; 6]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_width_emu: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_height_emu: Option<i64>,
    pub shape_type: Option<String>,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_gradient: Option<Gradient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke_gradient: Option<Gradient>,
    pub stroke_width_emu: Option<i64>,
    pub dash: Option<String>,
    pub shadow: Option<Shadow>,
    pub head_end: Option<String>,
    pub tail_end: Option<String>,
    pub adjustments: Vec<i64>,
    pub custom_paths: Vec<CustomPath>,
    pub image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crop: Option<Crop>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_text: Option<String>,
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

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Gradient {
    #[serde(rename = "type")]
    pub kind: &'static str,
    pub angle: f64,
    pub stops: Vec<GradientStop>,
    pub center_x: f64,
    pub center_y: f64,
}

#[derive(Serialize, Clone, Debug)]
pub struct GradientStop {
    pub color: String,
    pub position: f64,
}

#[derive(Serialize, Clone, Debug)]
pub struct Crop {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_gradient: Option<Gradient>,
    pub shadow: Option<Shadow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Table {
    pub z_order: usize,
    pub x: i64,
    pub y: i64,
    pub w: i64,
    pub h: i64,
    pub rotation: f32,
    pub flip_h: bool,
    pub flip_v: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transform: Option<[f64; 6]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_width_emu: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_height_emu: Option<i64>,
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
    pub paragraphs: Vec<Para>,
    pub borders: CellBorders,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_align: Option<String>,
    pub is_header: bool,
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

#[derive(Serialize, Clone, Default)]
pub struct CellBorders {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub right: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bottom: Option<Border>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub left: Option<Border>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Border {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub width_emu: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dash: Option<String>,
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
    transform: Option<[f64; 6]>,
    local_width_emu: Option<i64>,
    local_height_emu: Option<i64>,
}

#[derive(Clone, Copy)]
struct CoordinateSpace {
    out_x: f64,
    out_y: f64,
    child_x: f64,
    child_y: f64,
    scale_x: f64,
    scale_y: f64,
    matrix: [f64; 6],
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
            matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        }
    }
}

impl CoordinateSpace {
    fn map(&self, value: BoxTransform) -> BoxTransform {
        let w = (value.w as f64 * self.scale_x).round() as i64;
        let h = (value.h as f64 * self.scale_y).round() as i64;
        let matrix = multiply_matrix(self.matrix, box_matrix(value));
        BoxTransform {
            x: (self.out_x + (value.x as f64 - self.child_x) * self.scale_x).round() as i64,
            y: (self.out_y + (value.y as f64 - self.child_y) * self.scale_y).round() as i64,
            w,
            h,
            transform: Some(matrix),
            local_width_emu: Some(value.w),
            local_height_emu: Some(value.h),
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
    table_styles: HashMap<String, XmlElement>,
    default_table_style: Option<String>,
    placeholders: HashMap<String, XmlElement>,
    master_text_styles: Option<XmlElement>,
    default_text_style: Option<XmlElement>,
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
            theme.master_text_styles = master.find("txStyles").cloned();
            collect_placeholder_styles(master, &mut theme.placeholders);
        }
        if let Some(layout) = layout_root.as_ref() {
            collect_placeholder_styles(layout, &mut theme.placeholders);
        }
        theme.default_text_style = presentation.find("defaultTextStyle").cloned();
        if let Some(styles) = relationship_target(&presentation_rels, "tableStyles")
            .or_else(|| {
                archive
                    .parts
                    .contains_key("ppt/tableStyles.xml")
                    .then(|| "ppt/tableStyles.xml".into())
            })
            .and_then(|path| archive.xml(&path).ok())
        {
            theme.default_table_style = styles.attr("def").map(str::to_owned);
            theme.table_styles = styles
                .find_all("tblStyle")
                .into_iter()
                .filter_map(|style| Some((style.attr("styleId")?.to_owned(), style.clone())))
                .collect();
        }
        if !theme
            .table_styles
            .contains_key("{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}")
        {
            theme.table_styles.insert(
                "{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}".into(),
                standard_medium2_accent1(),
            );
        }

        let mut placeholder_boxes = HashMap::new();
        if let Some(master) = master_root.as_ref() {
            collect_placeholder_boxes(master, &mut placeholder_boxes);
        }
        if let Some(layout) = layout_root.as_ref() {
            collect_placeholder_boxes(layout, &mut placeholder_boxes);
        }

        let background =
            background_for_part(&slide_root, &theme, &slide_rels, &archive, &mut warnings)
                .or_else(|| {
                    layout_root.as_ref().and_then(|r| {
                        background_for_part(r, &theme, &layout_rels, &archive, &mut warnings)
                    })
                })
                .or_else(|| {
                    master_root.as_ref().and_then(|r| {
                        background_for_part(r, &theme, &master_rels, &archive, &mut warnings)
                    })
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
            "sp" => {
                let resolved = resolve_placeholder_shape(child, theme);
                let mut shape = parse_shape(
                    &resolved,
                    theme,
                    placeholder
                        .as_deref()
                        .and_then(|k| placeholder_boxes.get(k)),
                    space,
                    inherited_fill.as_deref(),
                    background_fill,
                );
                if let Some(fill) = child.find("spPr").and_then(|p| p.find("blipFill")) {
                    if let Some(image) = image_fill_url(fill, rels, archive, warnings) {
                        shape.kind = "picture".into();
                        shape.image = Some(image);
                        shape.crop = picture_crop(fill);
                    }
                }
                shape.z_order = shapes.len() + tables.len();
                warn_unsupported_effects(child, warnings);
                shapes.push(shape);
            }
            "pic" => {
                if let Some(mut shape) = parse_picture(child, rels, archive, theme, space, warnings)
                {
                    shape.z_order = shapes.len() + tables.len();
                    shapes.push(shape);
                }
            }
            "cxnSp" => {
                let mut shape = parse_connector(child, theme, space);
                shape.z_order = shapes.len() + tables.len();
                shapes.push(shape);
            }
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
                if let Some(mut table) = parse_table(child, theme, space) {
                    table.z_order = shapes.len() + tables.len();
                    if let Some(style_id) = child.find_descendant("tableStyleId") {
                        let id = style_id.text_content();
                        if !theme.table_styles.contains_key(id.trim()) {
                            warnings
                                .push(format!("表格样式未内嵌，暂未还原预设样式: {}", id.trim()));
                        }
                    }
                    tables.push(table);
                } else if let Some((drawing_path, drawing, drawing_rels)) =
                    cached_diagram(child, source_path, rels, archive)
                {
                    if let Some(tree) = drawing.find_descendant("spTree") {
                        let mut group = XmlElement::new("p:grpSp");
                        let mut properties = XmlElement::new("p:grpSpPr");
                        let mut xfrm = child
                            .find("xfrm")
                            .cloned()
                            .unwrap_or_else(|| XmlElement::new("a:xfrm"));
                        if let Some(cached) = tree.find("grpSpPr").and_then(|p| p.find("xfrm")) {
                            for name in ["chOff", "chExt"] {
                                if let Some(value) = cached.find(name) {
                                    replace_child(&mut xfrm, value.clone());
                                }
                            }
                        }
                        properties.children.push(xfrm);
                        group.children.push(properties);
                        parse_children(
                            tree,
                            &drawing_path,
                            &drawing_rels,
                            archive,
                            theme,
                            false,
                            placeholder_boxes,
                            group_space(&group, space),
                            inherited_fill.clone(),
                            background_fill,
                            shapes,
                            tables,
                            warnings,
                        );
                    }
                } else if child.find_descendant("chart").is_some() {
                    warnings.push("暂未渲染 PowerPoint 原生图表".into());
                    shapes.push(unsupported_frame(
                        child,
                        theme,
                        space,
                        shapes.len() + tables.len(),
                        "图表（暂不支持）",
                    ));
                } else if child.find_descendant("relIds").is_some() {
                    warnings.push("暂未渲染 SmartArt 图形".into());
                    shapes.push(unsupported_frame(
                        child,
                        theme,
                        space,
                        shapes.len() + tables.len(),
                        "SmartArt（缺少可用绘图缓存）",
                    ));
                } else {
                    warnings.push("暂未渲染此 PowerPoint 图形框内容".into());
                    shapes.push(unsupported_frame(
                        child,
                        theme,
                        space,
                        shapes.len() + tables.len(),
                        "暂不支持的图形内容",
                    ));
                }
            }
            "AlternateContent" => {
                let branch = child
                    .find_all("Choice")
                    .into_iter()
                    .find(|choice| has_supported_visual(choice, rels, archive))
                    .or_else(|| child.find("Fallback"))
                    .or_else(|| child.find("Choice"));
                if let Some(branch) = branch {
                    parse_children(
                        branch,
                        source_path,
                        rels,
                        archive,
                        theme,
                        skip_placeholders,
                        placeholder_boxes,
                        space,
                        inherited_fill.clone(),
                        background_fill,
                        shapes,
                        tables,
                        warnings,
                    );
                }
            }
            _ => {}
        }
    }
}

fn has_supported_visual(
    branch: &XmlElement,
    rels: &HashMap<String, Rel>,
    archive: &Archive,
) -> bool {
    branch
        .children
        .iter()
        .any(|child| match child.local_name.as_str() {
            "sp" | "cxnSp" => true,
            // A picture tag alone is not a usable Choice: Office commonly stores
            // an EMF/WMF here and a browser-compatible image in Fallback.
            "pic" => child
                .find_descendant("blip")
                .and_then(|blip| blip.attr("r:embed").or_else(|| blip.attr("embed")))
                .and_then(|id| rels.get(id))
                .is_some_and(|rel| {
                    archive.parts.contains_key(&rel.target) && mime_type(&rel.target).is_some()
                }),
            "grpSp" => has_supported_visual(child, rels, archive),
            "graphicFrame" => child.find_descendant("tbl").is_some(),
            _ => false,
        })
}

fn unsupported_frame(
    frame: &XmlElement,
    theme: &Theme,
    space: CoordinateSpace,
    z_order: usize,
    label: &str,
) -> Shape {
    let mut source = XmlElement::new("p:sp");
    let mut properties = XmlElement::new("p:spPr");
    if let Some(xfrm) = frame.find("xfrm") {
        properties.children.push(xfrm.clone());
    }
    let mut preset = XmlElement::new("a:prstGeom");
    preset.set_attr("prst", "rect");
    properties.children.push(preset);
    source.children.push(properties);
    let mut body = XmlElement::new("p:txBody");
    let mut paragraph = XmlElement::new("a:p");
    let mut run = XmlElement::new("a:r");
    let mut props = XmlElement::new("a:rPr");
    props.set_attr("sz", "1200");
    run.children.push(props);
    let mut text = XmlElement::new("a:t");
    text.text = label.into();
    run.children.push(text);
    paragraph.children.push(run);
    body.children.push(paragraph);
    source.children.push(body);
    let mut shape = parse_shape(&source, theme, None, space, None, None);
    shape.z_order = z_order;
    shape.name = element_name(frame);
    shape.fill = Some("#f4f5f7".into());
    shape.stroke = Some("#a8afb8".into());
    shape.dash = Some("dash".into());
    shape
}

fn cached_diagram(
    frame: &XmlElement,
    source_path: &str,
    rels: &HashMap<String, Rel>,
    archive: &Archive,
) -> Option<(String, XmlElement, HashMap<String, Rel>)> {
    // Cached drawings contain ordinary shapes. Do not follow recursive diagram
    // frames inside cache parts; a malformed package must not form a graph loop.
    if source_path.contains("/diagrams/") {
        return None;
    }
    let ids = frame.find_descendant("relIds")?;
    let data_id = ids.attr("r:dm").or_else(|| ids.attr("dm"))?;
    let data_path = &rels.get(data_id)?.target;
    let data = archive.xml(data_path).ok()?;
    let data_rels = archive.relationships(data_path).ok()?;
    let drawing_path = data
        .find_descendant("dataModelExt")
        .and_then(|e| e.attr("relId"))
        .and_then(|id| data_rels.get(id))
        .map(|r| r.target.clone())
        .or_else(|| relationship_target(&data_rels, "diagramDrawing"))?;
    let drawing = archive.xml(&drawing_path).ok()?;
    drawing.find_descendant("spTree")?;
    let drawing_rels = archive.relationships(&drawing_path).ok()?;
    Some((drawing_path, drawing, drawing_rels))
}

fn picture_crop(fill: &XmlElement) -> Option<Crop> {
    fill.find("srcRect").map(|crop| Crop {
        left: attr_i64(crop, "l", 0) as f64 / 100_000.0,
        top: attr_i64(crop, "t", 0) as f64 / 100_000.0,
        right: attr_i64(crop, "r", 0) as f64 / 100_000.0,
        bottom: attr_i64(crop, "b", 0) as f64 / 100_000.0,
    })
}

fn image_fill_url(
    fill: &XmlElement,
    rels: &HashMap<String, Rel>,
    archive: &Archive,
    warnings: &mut Vec<String>,
) -> Option<String> {
    let blip = fill.find("blip")?;
    let id = blip.attr("r:embed").or_else(|| blip.attr("embed"))?;
    let target = &rels.get(id)?.target;
    let mime = match mime_type(target) {
        Some(mime) => mime,
        None => {
            warnings.push(format!("暂不支持图片格式: {}", extension(target)));
            return None;
        }
    };
    if fill.find("tile").is_some() {
        warnings.push("平铺图片填充暂按拉伸显示".into());
    }
    Some(format!(
        "data:{mime};base64,{}",
        base64(archive.parts.get(target)?)
    ))
}

fn background_for_part(
    root: &XmlElement,
    theme: &Theme,
    rels: &HashMap<String, Rel>,
    archive: &Archive,
    warnings: &mut Vec<String>,
) -> Option<String> {
    background_fill(root, theme).or_else(|| {
        let fill = root.find_descendant("bg")?.find("bgPr")?.find("blipFill")?;
        let url = image_fill_url(fill, rels, archive, warnings)?;
        if picture_crop(fill)
            .is_some_and(|c| c.left != 0.0 || c.right != 0.0 || c.top != 0.0 || c.bottom != 0.0)
        {
            warnings.push("背景图片裁剪暂未还原".into());
        }
        Some(format!("url(\"{url}\") center / 100% 100% no-repeat"))
    })
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
        z_order: 0,
        kind: "shape".into(),
        name: element_name(element),
        x: transform.x,
        y: transform.y,
        w: transform.w,
        h: transform.h,
        rotation: transform.rotation,
        flip_h: transform.flip_h,
        flip_v: transform.flip_v,
        transform: transform.transform,
        local_width_emu: transform.local_width_emu,
        local_height_emu: transform.local_height_emu,
        shape_type: properties
            .and_then(|p| p.find("prstGeom"))
            .and_then(|g| g.attr("prst"))
            .map(str::to_string)
            .or_else(|| (!custom_paths.is_empty()).then(|| "custom".into())),
        fill,
        stroke,
        fill_gradient: properties.and_then(|p| parse_gradient(p, theme)),
        stroke_gradient: properties
            .and_then(|p| p.find("ln"))
            .and_then(|p| parse_gradient(p, theme)),
        stroke_width_emu,
        dash,
        shadow,
        head_end,
        tail_end,
        adjustments,
        custom_paths,
        image: None,
        crop: None,
        vertical_text: body_pr.and_then(|p| p.attr("vert")).map(str::to_owned),
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
        z_order: 0,
        kind: "picture".into(),
        name: element_name(element),
        x: transform.x,
        y: transform.y,
        w: transform.w,
        h: transform.h,
        rotation: transform.rotation,
        flip_h: transform.flip_h,
        flip_v: transform.flip_v,
        transform: transform.transform,
        local_width_emu: transform.local_width_emu,
        local_height_emu: transform.local_height_emu,
        shape_type: properties
            .and_then(|p| p.find("prstGeom"))
            .and_then(|g| g.attr("prst"))
            .map(str::to_owned)
            .or_else(|| Some("rect".into())),
        fill,
        stroke,
        fill_gradient: properties.and_then(|p| parse_gradient(p, theme)),
        stroke_gradient: properties
            .and_then(|p| p.find("ln"))
            .and_then(|p| parse_gradient(p, theme)),
        stroke_width_emu,
        dash,
        shadow: resolved_shape_shadow(properties, element.find("style"), theme),
        head_end: None,
        tail_end: None,
        adjustments: geometry_adjustments(properties),
        custom_paths: parse_custom_paths(properties),
        image: Some(format!("data:{};base64,{}", mime.unwrap(), base64(bytes))),
        crop: element
            .find("blipFill")
            .and_then(|p| p.find("srcRect"))
            .map(|crop| Crop {
                left: attr_i64(crop, "l", 0) as f64 / 100_000.0,
                top: attr_i64(crop, "t", 0) as f64 / 100_000.0,
                right: attr_i64(crop, "r", 0) as f64 / 100_000.0,
                bottom: attr_i64(crop, "b", 0) as f64 / 100_000.0,
            }),
        vertical_text: None,
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
        z_order: 0,
        kind: "line".into(),
        name: element_name(element),
        x: transform.x,
        y: transform.y,
        w: transform.w,
        h: transform.h,
        rotation: transform.rotation,
        flip_h: transform.flip_h,
        flip_v: transform.flip_v,
        transform: transform.transform,
        local_width_emu: transform.local_width_emu,
        local_height_emu: transform.local_height_emu,
        shape_type: properties
            .and_then(|p| p.find("prstGeom"))
            .and_then(|g| g.attr("prst"))
            .map(str::to_string),
        fill: None,
        fill_gradient: None,
        stroke_gradient: properties
            .and_then(|p| p.find("ln"))
            .and_then(|p| parse_gradient(p, theme)),
        stroke: stroke.or_else(|| Some("#000000".into())),
        stroke_width_emu,
        dash,
        shadow: resolved_shape_shadow(properties, element.find("style"), theme),
        head_end,
        tail_end,
        adjustments: Vec::new(),
        custom_paths: Vec::new(),
        image: None,
        crop: None,
        vertical_text: None,
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
    let table_pr = table.find("tblPr");
    let style_id = table_pr
        .and_then(|p| p.find("tableStyleId"))
        .map(XmlElement::text_content)
        .or_else(|| theme.default_table_style.clone());
    let style = style_id
        .as_ref()
        .and_then(|id| theme.table_styles.get(id.trim()));
    let source_rows = table.find_all("tr");
    let row_count = source_rows.len();
    let column_count = table
        .find("tblGrid")
        .map(|g| g.find_all("gridCol").len())
        .unwrap_or(0);
    let first_row = table_pr.is_some_and(|p| is_true(p.attr("firstRow")));
    let mut rows = Vec::new();
    for (row_index, row) in source_rows.into_iter().enumerate() {
        let mut cells = Vec::new();
        for (column_index, cell) in row.find_all("tc").into_iter().enumerate() {
            if is_true(cell.attr("hMerge")) || is_true(cell.attr("vMerge")) {
                continue;
            }
            let col_span = attr_u32(cell, "gridSpan", 1);
            let row_span = attr_u32(cell, "rowSpan", 1);
            let (styled_pr, text_pr) = resolved_table_cell_style(
                style,
                table_pr,
                row_index,
                column_index,
                row_count,
                column_count,
                row_span as usize,
                col_span as usize,
                theme,
            );
            let cell_pr = cell
                .find("tcPr")
                .map(|direct| merge_properties(&styled_pr, direct))
                .unwrap_or(styled_pr);
            let paragraphs = cell
                .find("txBody")
                .map(|body| parse_paragraphs(&body_with_text_defaults(body, &text_pr), theme, None))
                .unwrap_or_default();
            let text = paragraphs
                .iter()
                .map(|p| p.runs.iter().map(|r| r.text.as_str()).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n");
            let first_run = paragraphs.first().and_then(|p| p.runs.first());
            let color = first_run.and_then(|r| r.color.clone());
            let font_size_pt = first_run.and_then(|r| r.size_pt);
            let align = paragraphs.first().and_then(|p| p.align.clone());
            cells.push(TableCell {
                text,
                paragraphs,
                col_span,
                row_span,
                fill: fill_css(&cell_pr, theme),
                color,
                font_size_pt,
                align,
                borders: CellBorders {
                    top: cell_pr.find("lnT").map(|line| parse_border(line, theme)),
                    right: cell_pr.find("lnR").map(|line| parse_border(line, theme)),
                    bottom: cell_pr.find("lnB").map(|line| parse_border(line, theme)),
                    left: cell_pr.find("lnL").map(|line| parse_border(line, theme)),
                },
                vertical_align: cell_pr.attr("anchor").map(vertical_align),
                is_header: first_row && row_index == 0,
                margin_left_emu: attr_i64(&cell_pr, "marL", 91_440),
                margin_right_emu: attr_i64(&cell_pr, "marR", 91_440),
                margin_top_emu: attr_i64(&cell_pr, "marT", 45_720),
                margin_bottom_emu: attr_i64(&cell_pr, "marB", 45_720),
            });
        }
        rows.push(TableRow {
            height_emu: attr_i64(row, "h", 0),
            cells,
        });
    }
    Some(Table {
        z_order: 0,
        x: transform.x,
        y: transform.y,
        w: transform.w,
        h: transform.h,
        rotation: transform.rotation,
        flip_h: transform.flip_h,
        flip_v: transform.flip_v,
        transform: transform.transform,
        local_width_emu: transform.local_width_emu,
        local_height_emu: transform.local_height_emu,
        columns,
        rows,
    })
}

fn parse_border(line: &XmlElement, theme: &Theme) -> Border {
    Border {
        color: fill_css(line, theme),
        width_emu: if line.find("noFill").is_some() {
            0
        } else {
            attr_i64(line, "w", 12_700)
        },
        dash: line
            .find("prstDash")
            .and_then(|d| d.attr("val"))
            .map(str::to_owned),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolved_table_cell_style(
    style: Option<&XmlElement>,
    table_pr: Option<&XmlElement>,
    row: usize,
    col: usize,
    row_count: usize,
    col_count: usize,
    row_span: usize,
    col_span: usize,
    theme: &Theme,
) -> (XmlElement, XmlElement) {
    let mut cell = XmlElement::new("a:tcPr");
    let mut text = XmlElement::new("a:defRPr");
    let Some(style) = style else {
        return (cell, text);
    };
    let enabled = |key: &str| table_pr.is_some_and(|p| is_true(p.attr(key)));
    let mut regions = vec![("wholeTbl", 0, row_count, 0, col_count)];
    let body_row = row.saturating_sub(usize::from(enabled("firstRow")));
    let body_col = col.saturating_sub(usize::from(enabled("firstCol")));
    if enabled("bandRow") {
        regions.push((
            if body_row % 2 == 0 {
                "band1H"
            } else {
                "band2H"
            },
            row,
            row + row_span,
            0,
            col_count,
        ));
    }
    if enabled("bandCol") {
        regions.push((
            if body_col % 2 == 0 {
                "band1V"
            } else {
                "band2V"
            },
            0,
            row_count,
            col,
            col + col_span,
        ));
    }
    if enabled("lastCol") && col + col_span == col_count {
        regions.push(("lastCol", 0, row_count, col, col_count));
    }
    if enabled("firstCol") && col == 0 {
        regions.push(("firstCol", 0, row_count, 0, col_span));
    }
    if enabled("lastRow") && row + row_span == row_count {
        regions.push(("lastRow", row, row_count, 0, col_count));
    }
    if enabled("firstRow") && row == 0 {
        regions.push(("firstRow", 0, row_span, 0, col_count));
    }
    if enabled("firstRow") && enabled("firstCol") && row == 0 && col == 0 {
        regions.push(("nwCell", 0, row_span, 0, col_span));
    }
    if enabled("firstRow") && enabled("lastCol") && row == 0 && col + col_span == col_count {
        regions.push(("neCell", 0, row_span, col, col_count));
    }
    if enabled("lastRow") && enabled("firstCol") && row + row_span == row_count && col == 0 {
        regions.push(("swCell", row, row_count, 0, col_span));
    }
    if enabled("lastRow")
        && enabled("lastCol")
        && row + row_span == row_count
        && col + col_span == col_count
    {
        regions.push(("seCell", row, row_count, col, col_count));
    }
    for (name, top, bottom, left, right) in regions {
        let Some(region) = style.find(name) else {
            continue;
        };
        if let Some(properties) = region.find("tcStyle") {
            if let Some(fill) = properties.find("fill") {
                cell = merge_properties(&cell, fill);
                cell.local_name = "tcPr".into();
            }
            if let Some(borders) = properties.find("tcBdr") {
                for (target, source) in [
                    ("lnT", if row == top { "top" } else { "insideH" }),
                    (
                        "lnB",
                        if row + row_span == bottom {
                            "bottom"
                        } else {
                            "insideH"
                        },
                    ),
                    ("lnL", if col == left { "left" } else { "insideV" }),
                    (
                        "lnR",
                        if col + col_span == right {
                            "right"
                        } else {
                            "insideV"
                        },
                    ),
                ] {
                    if let Some(line) = borders.find(source).and_then(|edge| edge.find("ln")) {
                        let mut line = line.clone();
                        line.local_name = target.into();
                        line.tag = format!("a:{target}");
                        if let Some(index) =
                            cell.children.iter().position(|c| c.local_name == target)
                        {
                            cell.children[index] = line;
                        } else {
                            cell.children.push(line);
                        }
                    }
                }
            }
        }
        if let Some(properties) = region.find("tcTxStyle") {
            let mut run = properties.clone();
            run.local_name = "defRPr".into();
            run.tag = "a:defRPr".into();
            if let Some(color) = color_css(properties, theme) {
                // Keep source color nodes, so alpha and theme modifiers remain available.
                let mut fill = XmlElement::new("a:solidFill");
                fill.children = properties
                    .children
                    .iter()
                    .filter(|c| {
                        matches!(
                            c.local_name.as_str(),
                            "srgbClr" | "schemeClr" | "sysClr" | "prstClr"
                        )
                    })
                    .cloned()
                    .collect();
                if !color.is_empty() {
                    run.children.push(fill);
                }
            }
            if let Some(font) = shape_text_defaults(Some(properties), theme).font {
                for name in ["latin", "ea"] {
                    let mut el = XmlElement::new(&format!("a:{name}"));
                    el.set_attr("typeface", &font);
                    run.children.push(el);
                }
            }
            text = merge_properties(&text, &run);
        }
    }
    (cell, text)
}

fn body_with_text_defaults(body: &XmlElement, run: &XmlElement) -> XmlElement {
    let mut inherited = XmlElement::new("a:txBody");
    let mut list = XmlElement::new("a:lstStyle");
    let mut defaults = XmlElement::new("a:defPPr");
    defaults.children.push(run.clone());
    list.children.push(defaults);
    inherited.children.push(list);
    merge_properties(&inherited, body)
}

fn standard_medium2_accent1() -> XmlElement {
    // Office's standard Medium Style 2 / Accent 1 is commonly referenced without
    // an embedded definition. Keep its real conditional rules, not a CSS header guess.
    let line = |width| {
        format!("<a:ln w=\"{width}\"><a:solidFill><a:schemeClr val=\"lt1\"/></a:solidFill></a:ln>")
    };
    let borders = ["left", "right", "top", "bottom", "insideH", "insideV"]
        .iter()
        .map(|edge| format!("<a:{edge}>{}</a:{edge}>", line(12700)))
        .collect::<String>();
    let fill = |tint: Option<u32>| {
        format!("<a:fill><a:solidFill><a:schemeClr val=\"accent1\">{}</a:schemeClr></a:solidFill></a:fill>",tint.map(|n|format!("<a:tint val=\"{n}\"/>")).unwrap_or_default())
    };
    let text =
        "<a:tcTxStyle b=\"on\"><a:fontRef idx=\"minor\"/><a:schemeClr val=\"lt1\"/></a:tcTxStyle>";
    let xml = format!("<a:tblStyle xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\"><a:wholeTbl><a:tcTxStyle><a:fontRef idx=\"minor\"/><a:schemeClr val=\"dk1\"/></a:tcTxStyle><a:tcStyle><a:tcBdr>{borders}</a:tcBdr>{}</a:tcStyle></a:wholeTbl><a:band1H><a:tcStyle>{}</a:tcStyle></a:band1H><a:band1V><a:tcStyle>{}</a:tcStyle></a:band1V><a:firstRow>{text}<a:tcStyle><a:tcBdr><a:bottom>{}</a:bottom></a:tcBdr>{}</a:tcStyle></a:firstRow><a:lastRow>{text}<a:tcStyle><a:tcBdr><a:top>{}</a:top></a:tcBdr>{}</a:tcStyle></a:lastRow><a:firstCol>{text}<a:tcStyle>{}</a:tcStyle></a:firstCol><a:lastCol>{text}<a:tcStyle>{}</a:tcStyle></a:lastCol></a:tblStyle>",fill(Some(20000)),fill(Some(40000)),fill(Some(40000)),line(38100),fill(None),line(38100),fill(None),fill(None),fill(None));
    XmlParser::parse_str(&xml).expect("static standard table style is valid XML")
}

fn parse_paragraphs(
    body: &XmlElement,
    theme: &Theme,
    text_defaults: Option<&TextDefaults>,
) -> Vec<Para> {
    let mut result = Vec::new();
    let mut counters: HashMap<u32, (String, u32, u32)> = HashMap::new();
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
            let bullet = if let Some(numbering) = [p_pr, level_pr, default_pr]
                .into_iter()
                .flatten()
                .find(|p| {
                    p.find("buNone").is_some()
                        || p.find("buChar").is_some()
                        || p.find("buAutoNum").is_some()
                })
                .and_then(|p| p.find("buAutoNum"))
            {
                let kind = numbering.attr("type").unwrap_or("arabicPeriod");
                let start = attr_u32(numbering, "startAt", 1).max(1);
                counters.retain(|candidate, _| *candidate <= level);
                let counter =
                    counters
                        .entry(level)
                        .or_insert((kind.into(), start, start.saturating_sub(1)));
                if counter.0 != kind || counter.1 != start {
                    *counter = (kind.into(), start, start.saturating_sub(1));
                }
                counter.2 = counter.2.saturating_add(1);
                Some(numbered_bullet(kind, counter.2))
            } else {
                counters.retain(|candidate, _| *candidate < level);
                paragraph_bullet(p_pr, level_pr, default_pr)
            };
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
                bullet,
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
    let paint_source = explicit
        .into_iter()
        .chain(fallbacks.iter().copied())
        .find(|properties| fill_css(properties, theme).is_some());
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
        color: paint_source
            .and_then(|p| fill_css(p, theme))
            .or_else(|| text_defaults.and_then(|defaults| defaults.color.clone())),
        color_gradient: paint_source.and_then(|p| parse_gradient(p, theme)),
        shadow: explicit
            .and_then(|p| parse_shadow(Some(p), theme))
            .or_else(|| fallbacks.iter().find_map(|p| parse_shadow(Some(p), theme))),
    }
}

fn numbered_bullet(kind: &str, value: u32) -> String {
    let mut text = if kind.starts_with("alpha") {
        let mut n = value;
        let mut letters = String::new();
        while n > 0 {
            n -= 1;
            letters.insert(0, (b'A' + (n % 26) as u8) as char);
            n /= 26;
        }
        letters
    } else if kind.starts_with("roman") && value < 4000 {
        let mut n = value;
        let mut roman = String::new();
        for (number, symbol) in [
            (1000, "M"),
            (900, "CM"),
            (500, "D"),
            (400, "CD"),
            (100, "C"),
            (90, "XC"),
            (50, "L"),
            (40, "XL"),
            (10, "X"),
            (9, "IX"),
            (5, "V"),
            (4, "IV"),
            (1, "I"),
        ] {
            while n >= number {
                roman.push_str(symbol);
                n -= number;
            }
        }
        roman
    } else {
        value.to_string()
    };
    if kind.contains("Lc") {
        text = text.to_lowercase();
    }
    if kind.ends_with("ParenBoth") {
        format!("({text})")
    } else if kind.ends_with("ParenR") {
        format!("{text})")
    } else if kind.ends_with("Plain") {
        text
    } else {
        format!("{text}.")
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
    if let Some(gradient) = parse_gradient(element, theme) {
        let stops = gradient
            .stops
            .iter()
            .map(|s| format!("{} {}%", s.color, s.position * 100.0))
            .collect::<Vec<_>>()
            .join(",");
        return Some(if gradient.kind == "radial" {
            format!(
                "radial-gradient(ellipse at {}% {}%,{stops})",
                gradient.center_x * 100.0,
                gradient.center_y * 100.0
            )
        } else {
            format!("linear-gradient({}deg,{stops})", gradient.angle)
        });
    }
    None
}

fn parse_gradient(element: &XmlElement, theme: &Theme) -> Option<Gradient> {
    // Direct solid/noFill overrides inherited gradients, even if malformed XML contains both.
    if element.find("noFill").is_some() || element.find("solidFill").is_some() {
        return None;
    }
    let gradient = element.find("gradFill")?;
    let mut stops = gradient
        .find("gsLst")?
        .find_all("gs")
        .into_iter()
        .filter_map(|stop| {
            Some(GradientStop {
                color: color_css(stop, theme)?,
                position: (attr_i64(stop, "pos", 0) as f64 / 100_000.0).clamp(0.0, 1.0),
            })
        })
        .collect::<Vec<_>>();
    if stops.is_empty() {
        return None;
    }
    stops.sort_by(|a, b| a.position.total_cmp(&b.position));
    let path = gradient.find("path");
    let rect = path.and_then(|p| p.find("fillToRect"));
    Some(Gradient {
        kind: if path.is_some() { "radial" } else { "linear" },
        angle: gradient
            .find("lin")
            .map(|l| 90.0 + attr_i64(l, "ang", 0) as f64 / 60_000.0)
            .unwrap_or(180.0),
        stops,
        center_x: rect
            .map(|r| (attr_i64(r, "l", 0) + 100_000 - attr_i64(r, "r", 0)) as f64 / 200_000.0)
            .unwrap_or(0.5),
        center_y: rect
            .map(|r| (attr_i64(r, "t", 0) + 100_000 - attr_i64(r, "b", 0)) as f64 / 200_000.0)
            .unwrap_or(0.5),
    })
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
    for transform in &color.children {
        let amount = attr_i64(transform, "val", 100_000) as f64 / 100_000.0;
        match transform.local_name.as_str() {
            "tint" => {
                for value in &mut rgb {
                    *value = (255.0 - (255.0 - *value as f64) * amount)
                        .clamp(0.0, 255.0)
                        .round() as u8;
                }
            }
            "shade" => {
                for value in &mut rgb {
                    *value = (*value as f64 * amount).clamp(0.0, 255.0).round() as u8;
                }
            }
            _ => {}
        }
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

fn merge_properties(base: &XmlElement, direct: &XmlElement) -> XmlElement {
    let mut result = base.clone();
    result.tag = direct.tag.clone();
    result.local_name = direct.local_name.clone();
    result.attributes.extend(direct.attributes.clone());
    if !direct.text.is_empty() {
        result.text = direct.text.clone();
    }
    for family in [
        &[
            "noFill",
            "solidFill",
            "gradFill",
            "blipFill",
            "pattFill",
            "grpFill",
        ][..],
        &["buNone", "buChar", "buAutoNum", "buBlip"][..],
        &[
            "srgbClr",
            "schemeClr",
            "sysClr",
            "prstClr",
            "scrgbClr",
            "hslClr",
        ][..],
        &["spAutoFit", "normAutofit", "noAutofit"][..],
    ] {
        if direct
            .children
            .iter()
            .any(|child| family.contains(&child.local_name.as_str()))
        {
            result
                .children
                .retain(|child| !family.contains(&child.local_name.as_str()));
        }
    }
    let mut visited = std::collections::HashSet::new();
    for child in &direct.children {
        if !visited.insert(child.local_name.as_str()) {
            continue;
        }
        let siblings: Vec<_> = direct
            .children
            .iter()
            .filter(|c| c.local_name == child.local_name)
            .collect();
        if siblings.len() > 1
            || matches!(
                child.local_name.as_str(),
                "p" | "r" | "fld" | "br" | "gs" | "gd" | "tr" | "tc"
            )
        {
            result.children.retain(|c| c.local_name != child.local_name);
            result.children.extend(siblings.into_iter().cloned());
        } else if let Some(index) = result
            .children
            .iter()
            .position(|c| c.local_name == child.local_name)
        {
            result.children[index] = merge_properties(&result.children[index], child);
        } else {
            result.children.push(child.clone());
        }
    }
    result
}

fn replace_child(root: &mut XmlElement, child: XmlElement) {
    if let Some(index) = root
        .children
        .iter()
        .position(|c| c.local_name == child.local_name)
    {
        root.children[index] = child;
    } else {
        root.children.push(child);
    }
}

fn merge_shape_styles(base: &XmlElement, direct: &XmlElement) -> XmlElement {
    let mut result = direct.clone();
    for name in ["spPr", "style"] {
        if let Some(inherited) = base.find(name) {
            replace_child(
                &mut result,
                direct
                    .find(name)
                    .map(|current| merge_properties(inherited, current))
                    .unwrap_or_else(|| inherited.clone()),
            );
        }
    }
    if let Some(body) = direct.find("txBody") {
        let mut merged = body.clone();
        if let Some(inherited) = base.find("txBody") {
            for name in ["bodyPr", "lstStyle"] {
                if let Some(properties) = inherited.find(name) {
                    replace_child(
                        &mut merged,
                        body.find(name)
                            .map(|current| merge_properties(properties, current))
                            .unwrap_or_else(|| properties.clone()),
                    );
                }
            }
            if let Some(properties) = inherited.find("p").and_then(|p| p.find("pPr")) {
                for paragraph in &mut merged.children {
                    if paragraph.local_name != "p" {
                        continue;
                    }
                    let inherited_level = attr_u32(properties, "lvl", 0);
                    let level = paragraph
                        .find("pPr")
                        .map(|p| attr_u32(p, "lvl", 0))
                        .unwrap_or(0);
                    if level == inherited_level {
                        let properties = paragraph
                            .find("pPr")
                            .map(|current| merge_properties(properties, current))
                            .unwrap_or_else(|| properties.clone());
                        replace_child(paragraph, properties);
                    }
                }
            }
        }
        replace_child(&mut result, merged);
    }
    result
}

fn collect_placeholder_styles(root: &XmlElement, result: &mut HashMap<String, XmlElement>) {
    let Some(tree) = root.find_descendant("spTree") else {
        return;
    };
    for shape in tree.find_all("sp") {
        let Some(ph) = shape.find_descendant("nvPr").and_then(|p| p.find("ph")) else {
            continue;
        };
        let kind = ph.attr("type").unwrap_or("body");
        let type_key = format!("type:{kind}");
        let inherited = result
            .get(&type_key)
            .map(|base| merge_shape_styles(base, shape))
            .unwrap_or_else(|| shape.clone());
        if root.local_name == "sldMaster" {
            result.insert(type_key, inherited);
        } else {
            result.insert(format!("idx:{}", ph.attr("idx").unwrap_or("0")), inherited);
        }
    }
}

fn resolve_placeholder_shape(shape: &XmlElement, theme: &Theme) -> XmlElement {
    let ph = shape.find_descendant("nvPr").and_then(|p| p.find("ph"));
    let kind = ph.and_then(|p| p.attr("type")).unwrap_or("body");
    let inherited = ph
        .and_then(|p| {
            theme
                .placeholders
                .get(&format!("idx:{}", p.attr("idx").unwrap_or("0")))
        })
        .or_else(|| ph.and_then(|_| theme.placeholders.get(&format!("type:{kind}"))));
    let kind = ph
        .and_then(|p| p.attr("type"))
        .or_else(|| {
            inherited
                .and_then(|s| s.find_descendant("nvPr"))
                .and_then(|p| p.find("ph"))
                .and_then(|p| p.attr("type"))
        })
        .unwrap_or(kind);
    let mut result = inherited
        .map(|base| merge_shape_styles(base, shape))
        .unwrap_or_else(|| shape.clone());
    if let Some(body) = result.find("txBody").cloned() {
        let text_kind = if ph.is_none() {
            "otherStyle"
        } else if matches!(kind, "title" | "ctrTitle") {
            "titleStyle"
        } else {
            "bodyStyle"
        };
        let mut defaults = theme
            .default_text_style
            .clone()
            .unwrap_or_else(|| XmlElement::new("a:lstStyle"));
        if let Some(master) = theme
            .master_text_styles
            .as_ref()
            .and_then(|s| s.find(text_kind))
        {
            defaults = merge_properties(&defaults, master);
        }
        defaults.local_name = "lstStyle".into();
        defaults.tag = "a:lstStyle".into();
        let list = body
            .find("lstStyle")
            .map(|current| merge_properties(&defaults, current))
            .unwrap_or(defaults);
        let mut body = body;
        replace_child(&mut body, list);
        replace_child(&mut result, body);
    }
    result
}

fn warn_unsupported_effects(shape: &XmlElement, warnings: &mut Vec<String>) {
    for (tag, label) in [
        ("innerShdw", "内阴影"),
        ("glow", "发光"),
        ("reflection", "倒影"),
        ("softEdge", "柔化边缘"),
        ("scene3d", "三维场景"),
        ("sp3d", "三维形状"),
    ] {
        if shape.find_descendant(tag).is_some() {
            warnings.push(format!("暂未还原 PowerPoint {label}效果"));
        }
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
        transform: None,
        local_width_emu: None,
        local_height_emu: None,
    }
}

fn group_space(group: &XmlElement, parent: CoordinateSpace) -> CoordinateSpace {
    let Some(xfrm) = group.find("grpSpPr").and_then(|p| p.find("xfrm")) else {
        return parent;
    };
    let local = parse_transform(xfrm);
    let mapped = parent.map(local);
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
    let child_x = child_off.map(|v| attr_i64(v, "x", 0) as f64).unwrap_or(0.0);
    let child_y = child_off.map(|v| attr_i64(v, "y", 0) as f64).unwrap_or(0.0);
    let local_scale_x = local.w as f64 / child_w as f64;
    let local_scale_y = local.h as f64 / child_h as f64;
    CoordinateSpace {
        out_x: mapped.x as f64,
        out_y: mapped.y as f64,
        child_x,
        child_y,
        scale_x: mapped.w as f64 / child_w as f64,
        scale_y: mapped.h as f64 / child_h as f64,
        matrix: multiply_matrix(
            parent.matrix,
            multiply_matrix(
                box_matrix(local),
                [
                    local_scale_x,
                    0.0,
                    0.0,
                    local_scale_y,
                    -child_x * local_scale_x,
                    -child_y * local_scale_y,
                ],
            ),
        ),
    }
}

fn multiply_matrix(l: [f64; 6], r: [f64; 6]) -> [f64; 6] {
    [
        l[0] * r[0] + l[2] * r[1],
        l[1] * r[0] + l[3] * r[1],
        l[0] * r[2] + l[2] * r[3],
        l[1] * r[2] + l[3] * r[3],
        l[0] * r[4] + l[2] * r[5] + l[4],
        l[1] * r[4] + l[3] * r[5] + l[5],
    ]
}

fn box_matrix(value: BoxTransform) -> [f64; 6] {
    let radians = (value.rotation as f64).to_radians();
    let (sin, cos) = radians.sin_cos();
    let sx = if value.flip_h { -1.0 } else { 1.0 };
    let sy = if value.flip_v { -1.0 } else { 1.0 };
    let (a, b, c, d) = (cos * sx, sin * sx, -sin * sy, cos * sy);
    let (cx, cy) = (value.w as f64 / 2.0, value.h as f64 / 2.0);
    [
        a,
        b,
        c,
        d,
        value.x as f64 + cx - a * cx - c * cy,
        value.y as f64 + cy - b * cx - d * cy,
    ]
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
    matches!(value, "1" | "true" | "on")
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
    fn automatic_list_numbering_keeps_start_and_level_sequence() {
        let body = XmlParser::parse_str(r#"<a:txBody xmlns:a="urn:a"><a:lstStyle><a:lvl1pPr><a:buAutoNum type="arabicPeriod" startAt="3"/></a:lvl1pPr></a:lstStyle><a:p><a:r><a:t>First</a:t></a:r></a:p><a:p><a:r><a:t>Second</a:t></a:r></a:p></a:txBody>"#).unwrap();
        let paragraphs = parse_paragraphs(&body, &Theme::default(), None);
        assert_eq!(paragraphs[0].bullet.as_deref(), Some("3."));
        assert_eq!(paragraphs[1].bullet.as_deref(), Some("4."));
    }

    #[test]
    fn path_gradient_is_not_replaced_by_linear_gradient() {
        let properties = XmlParser::parse_str(r#"<a:spPr xmlns:a="urn:a"><a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs></a:gsLst><a:path path="circle"><a:fillToRect l="25000" t="25000" r="25000" b="25000"/></a:path></a:gradFill></a:spPr>"#).unwrap();
        assert!(fill_css(&properties, &Theme::default())
            .unwrap()
            .starts_with("radial-gradient("));
    }

    #[test]
    fn preserves_table_rich_text_and_direct_border_override() {
        let root = XmlParser::parse_str(r#"<p:graphicFrame xmlns:p="urn:p" xmlns:a="urn:a"><p:xfrm rot="5400000" flipH="1"><a:off x="100" y="200"/><a:ext cx="1000" cy="500"/></p:xfrm><a:graphic><a:graphicData><a:tbl><a:tblPr firstRow="1"><a:tableStyleId>test</a:tableStyleId></a:tblPr><a:tblGrid><a:gridCol w="1000"/></a:tblGrid><a:tr h="500"><a:tc><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr b="0" sz="800"/><a:t>Plain</a:t></a:r><a:r><a:rPr i="1" sz="1600"/><a:t>Italic</a:t></a:r></a:p><a:p><a:pPr><a:buChar char="•"/></a:pPr><a:r><a:t>Next</a:t></a:r></a:p></a:txBody><a:tcPr anchor="ctr"><a:lnB><a:noFill/></a:lnB></a:tcPr></a:tc></a:tr></a:tbl></a:graphicData></a:graphic></p:graphicFrame>"#).unwrap();
        let style=XmlParser::parse_str(r#"<a:tblStyle xmlns:a="urn:a"><a:wholeTbl><a:tcStyle><a:tcBdr><a:bottom><a:ln w="25400"><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:ln></a:bottom></a:tcBdr></a:tcStyle></a:wholeTbl><a:firstRow><a:tcTxStyle b="on"><a:srgbClr val="FFFFFF"/></a:tcTxStyle><a:tcStyle><a:fill><a:solidFill><a:srgbClr val="112233"/></a:solidFill></a:fill></a:tcStyle></a:firstRow></a:tblStyle>"#).unwrap();
        let mut theme = Theme::default();
        theme.table_styles.insert("test".into(), style);
        let table = parse_table(&root, &theme, CoordinateSpace::default()).unwrap();
        let cell = &table.rows[0].cells[0];
        assert_eq!(cell.text, "PlainItalic\nNext");
        assert_eq!(cell.paragraphs.len(), 2);
        assert!(!cell.paragraphs[0].runs[0].bold);
        assert!(cell.paragraphs[0].runs[1].bold && cell.paragraphs[0].runs[1].italic);
        assert_eq!(cell.paragraphs[0].runs[1].size_pt, Some(16.0));
        assert_eq!(cell.paragraphs[1].bullet.as_deref(), Some("•"));
        assert_eq!(cell.fill.as_deref(), Some("#112233"));
        assert_eq!(cell.borders.bottom.as_ref().unwrap().width_emu, 0);
        assert_eq!(cell.vertical_align.as_deref(), Some("center"));
        assert!(cell.is_header);
        assert_eq!(table.rotation, 90.0);
        assert!(table.flip_h);
        assert!(table.transform.is_some());
    }

    #[test]
    fn group_matrix_preserves_rotation_after_scaling_without_double_scale() {
        let group=XmlParser::parse_str(r#"<p:grpSp xmlns:p="urn:p" xmlns:a="urn:a"><p:grpSpPr><a:xfrm rot="5400000"><a:off x="100" y="200"/><a:ext cx="400" cy="200"/><a:chOff x="10" y="20"/><a:chExt cx="200" cy="100"/></a:xfrm></p:grpSpPr></p:grpSp>"#).unwrap();
        let space = group_space(&group, CoordinateSpace::default());
        let mapped = space.map(BoxTransform {
            x: 60,
            y: 45,
            w: 50,
            h: 25,
            ..Default::default()
        });
        assert_eq!((mapped.w, mapped.h), (100, 50));
        assert_eq!(
            (mapped.local_width_emu, mapped.local_height_emu),
            (Some(50), Some(25))
        );
        let matrix = mapped.transform.unwrap();
        for (actual, expected) in matrix.into_iter().zip([0.0, 2.0, -2.0, 0.0, 350.0, 200.0]) {
            assert!((actual - expected).abs() < 0.0001, "matrix {matrix:?}");
        }
        let nested = space.map(BoxTransform {
            x: 60,
            y: 45,
            w: 50,
            h: 25,
            flip_h: true,
            ..Default::default()
        });
        let matrix = nested.transform.unwrap();
        assert!((matrix[1] + 2.0).abs() < 0.0001);
        assert!((matrix[5] - 300.0).abs() < 0.0001);
    }

    #[test]
    fn shared_draw_order_and_unsupported_chart_warning_survive_tree_walk() {
        let root=XmlParser::parse_str(r#"<p:sld xmlns:p="urn:p" xmlns:a="urn:a" xmlns:c="urn:c"><p:cSld><p:spTree><p:sp/><p:graphicFrame><a:graphic><a:graphicData><a:tbl><a:tblGrid/><a:tr/></a:tbl></a:graphicData></a:graphic></p:graphicFrame><p:sp/><p:graphicFrame><a:graphic><a:graphicData><c:chart/></a:graphicData></a:graphic></p:graphicFrame></p:spTree></p:cSld></p:sld>"#).unwrap();
        let (mut shapes, mut tables, mut warnings) = (vec![], vec![], vec![]);
        parse_visual_tree(
            &root,
            "ppt/slides/slide1.xml",
            &HashMap::new(),
            &Archive {
                parts: HashMap::new(),
            },
            &Theme::default(),
            false,
            &HashMap::new(),
            None,
            &mut shapes,
            &mut tables,
            &mut warnings,
        );
        assert_eq!(
            shapes.iter().map(|s| s.z_order).collect::<Vec<_>>(),
            vec![0, 2, 3]
        );
        assert_eq!(tables[0].z_order, 1);
        assert_eq!(shapes[2].paragraphs[0].runs[0].text, "图表（暂不支持）");
        assert_eq!(warnings, vec!["暂未渲染 PowerPoint 原生图表"]);
    }

    #[test]
    fn cropped_picture_keeps_geometry_and_vertical_shape_text() {
        let picture=XmlParser::parse_str(r#"<p:pic xmlns:p="urn:p" xmlns:a="urn:a" xmlns:r="urn:r"><p:blipFill><a:blip r:embed="img"/><a:srcRect l="25000" t="0" r="10000" b="5000"/></p:blipFill><p:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1000" cy="500"/></a:xfrm><a:prstGeom prst="ellipse"/></p:spPr></p:pic>"#).unwrap();
        let rels = HashMap::from([(
            "img".into(),
            Rel {
                target: "ppt/media/image.png".into(),
                rel_type: "image".into(),
            },
        )]);
        let archive = Archive {
            parts: HashMap::from([("ppt/media/image.png".into(), vec![1, 2, 3])]),
        };
        let picture = parse_picture(
            &picture,
            &rels,
            &archive,
            &Theme::default(),
            CoordinateSpace::default(),
            &mut vec![],
        )
        .unwrap();
        assert_eq!(picture.shape_type.as_deref(), Some("ellipse"));
        let crop = picture.crop.unwrap();
        assert_eq!((crop.left, crop.right, crop.bottom), (0.25, 0.1, 0.05));
        let shape=XmlParser::parse_str(r#"<p:sp xmlns:p="urn:p" xmlns:a="urn:a"><p:txBody><a:bodyPr vert="eaVert"/><a:p><a:r><a:t>竖排</a:t></a:r></a:p></p:txBody></p:sp>"#).unwrap();
        assert_eq!(
            parse_shape(
                &shape,
                &Theme::default(),
                None,
                CoordinateSpace::default(),
                None,
                None
            )
            .vertical_text
            .as_deref(),
            Some("eaVert")
        );
    }

    #[test]
    fn placeholder_style_chain_keeps_source_content_and_direct_overrides() {
        let master=XmlParser::parse_str(r#"<p:sldMaster xmlns:p="urn:p" xmlns:a="urn:a"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:nvPr><p:ph type="body"/></p:nvPr></p:nvSpPr><p:txBody><a:bodyPr lIns="12700"/><a:lstStyle><a:lvl1pPr marL="25000"><a:defRPr sz="2400" b="1"/></a:lvl1pPr></a:lstStyle><a:p><a:r><a:t>Master prompt</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sldMaster>"#).unwrap();
        let layout=XmlParser::parse_str(r#"<p:sldLayout xmlns:p="urn:p" xmlns:a="urn:a"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:nvPr><p:ph type="body" idx="7"/></p:nvPr></p:nvSpPr><p:txBody><a:bodyPr anchor="ctr"/><a:lstStyle><a:lvl1pPr><a:defRPr sz="2000"/></a:lvl1pPr></a:lstStyle><a:p/></p:txBody></p:sp></p:spTree></p:cSld></p:sldLayout>"#).unwrap();
        let source=XmlParser::parse_str(r#"<p:sp xmlns:p="urn:p" xmlns:a="urn:a"><p:nvSpPr><p:nvPr><p:ph idx="7"/></p:nvPr></p:nvSpPr><p:txBody><a:bodyPr/><a:p><a:r><a:rPr b="0"/><a:t>Actual first</a:t></a:r></a:p><a:p><a:r><a:t>Actual second</a:t></a:r></a:p></p:txBody></p:sp>"#).unwrap();
        let mut theme = Theme::default();
        collect_placeholder_styles(&master, &mut theme.placeholders);
        collect_placeholder_styles(&layout, &mut theme.placeholders);
        let resolved = resolve_placeholder_shape(&source, &theme);
        let shape = parse_shape(
            &resolved,
            &theme,
            None,
            CoordinateSpace::default(),
            None,
            None,
        );
        assert_eq!(shape.paragraphs.len(), 2);
        assert_eq!(shape.paragraphs[0].runs[0].text, "Actual first");
        assert_eq!(shape.paragraphs[0].runs[0].size_pt, Some(20.0));
        assert!(!shape.paragraphs[0].runs[0].bold);
        assert!(shape.paragraphs[1].runs[0].bold);
        assert_eq!(shape.margin_left_emu, 12700);
        assert_eq!(shape.vertical_align.as_deref(), Some("center"));
    }

    #[test]
    fn standard_table_style_resolves_header_and_banding_from_flags() {
        let mut theme = Theme::default();
        theme.colors = HashMap::from([
            ("accent1".into(), "204060".into()),
            ("lt1".into(), "FFFFFF".into()),
            ("dk1".into(), "000000".into()),
        ]);
        let style = standard_medium2_accent1();
        let props =
            XmlParser::parse_str(r#"<a:tblPr xmlns:a="urn:a" firstRow="1" bandRow="1"/>"#).unwrap();
        let (header, text) =
            resolved_table_cell_style(Some(&style), Some(&props), 0, 0, 3, 2, 1, 1, &theme);
        assert_eq!(fill_css(&header, &theme).as_deref(), Some("#204060"));
        assert_eq!(header.find("lnB").and_then(|l| l.attr("w")), Some("38100"));
        assert_eq!(text.attr("b"), Some("on"));
        let (band, _) =
            resolved_table_cell_style(Some(&style), Some(&props), 1, 0, 3, 2, 1, 1, &theme);
        assert_eq!(fill_css(&band, &theme).as_deref(), Some("#A6B3BF"));
    }

    #[test]
    fn alternate_content_renders_only_one_supported_branch() {
        let root=XmlParser::parse_str(r#"<p:sld xmlns:p="urn:p" xmlns:a="urn:a" xmlns:c="urn:c" xmlns:mc="urn:mc"><p:cSld><p:spTree><mc:AlternateContent><mc:Choice Requires="c"><p:graphicFrame><a:graphic><a:graphicData><c:chart/></a:graphicData></a:graphic></p:graphicFrame></mc:Choice><mc:Fallback><p:sp><p:nvSpPr><p:cNvPr name="fallback"/></p:nvSpPr></p:sp></mc:Fallback></mc:AlternateContent><mc:AlternateContent><mc:Choice Requires="p"><p:sp><p:nvSpPr><p:cNvPr name="choice"/></p:nvSpPr></p:sp></mc:Choice><mc:Fallback><p:sp/></mc:Fallback></mc:AlternateContent></p:spTree></p:cSld></p:sld>"#).unwrap();
        let (mut shapes, mut tables, mut warnings) = (vec![], vec![], vec![]);
        parse_visual_tree(
            &root,
            "ppt/slides/slide1.xml",
            &HashMap::new(),
            &Archive {
                parts: HashMap::new(),
            },
            &Theme::default(),
            false,
            &HashMap::new(),
            None,
            &mut shapes,
            &mut tables,
            &mut warnings,
        );
        assert_eq!(
            shapes.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["fallback", "choice"]
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn alternate_content_uses_png_fallback_when_choice_picture_cannot_render() {
        use std::io::Write;

        let png = [
            137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1,
            8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 11, 73, 68, 65, 84, 120, 156, 99, 96, 0, 2,
            0, 0, 5, 0, 1, 169, 245, 232, 132, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
        ];
        let presentation = r#"<p:presentation xmlns:p="urn:p" xmlns:r="urn:r"><p:sldIdLst><p:sldId r:id="slide"/></p:sldIdLst></p:presentation>"#;
        let presentation_rels = r#"<Relationships><Relationship Id="slide" Target="slides/slide1.xml" Type="slide"/></Relationships>"#;
        let slide = r#"<p:sld xmlns:p="urn:p" xmlns:a="urn:a" xmlns:r="urn:r" xmlns:mc="urn:mc"><p:cSld><p:spTree><mc:AlternateContent><mc:Choice Requires="a"><p:pic><p:nvPicPr><p:cNvPr name="choice"/></p:nvPicPr><p:blipFill><a:blip r:embed="choice"/></p:blipFill><p:spPr><a:xfrm><a:off x="100" y="200"/><a:ext cx="300" cy="400"/></a:xfrm></p:spPr></p:pic></mc:Choice><mc:Fallback><p:pic><p:nvPicPr><p:cNvPr name="fallback"/></p:nvPicPr><p:blipFill><a:blip r:embed="fallback"/></p:blipFill><p:spPr><a:xfrm><a:off x="100" y="200"/><a:ext cx="300" cy="400"/></a:xfrm></p:spPr></p:pic></mc:Fallback></mc:AlternateContent></p:spTree></p:cSld></p:sld>"#;
        for (extension, include_choice, expected_name) in [
            ("emf", true, "fallback"),
            ("wmf", true, "fallback"),
            ("png", false, "fallback"),
            ("png", true, "choice"),
        ] {
            let choice_path = format!("ppt/media/choice.{extension}");
            let slide_rels = format!(
                r#"<Relationships><Relationship Id="choice" Target="../media/choice.{extension}" Type="image"/><Relationship Id="fallback" Target="../media/fallback.png" Type="image"/></Relationships>"#
            );
            let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
            let mut parts: Vec<(&str, &[u8])> = vec![
                ("ppt/presentation.xml", presentation.as_bytes()),
                (
                    "ppt/_rels/presentation.xml.rels",
                    presentation_rels.as_bytes(),
                ),
                ("ppt/slides/slide1.xml", slide.as_bytes()),
                ("ppt/slides/_rels/slide1.xml.rels", slide_rels.as_bytes()),
                ("ppt/media/fallback.png", &png),
            ];
            if include_choice {
                parts.push((&choice_path, &png));
            }
            for (path, bytes) in parts {
                zip.start_file(path, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(bytes).unwrap();
            }
            let doc = parse_document(&zip.finish().unwrap().into_inner()).unwrap();
            let shapes = &doc.slides[0].shapes;
            assert_eq!(shapes.len(), 1, "{extension}, present={include_choice}");
            assert_eq!(shapes[0].name, expected_name);
            assert_eq!(
                (shapes[0].x, shapes[0].y, shapes[0].w, shapes[0].h),
                (100, 200, 300, 400)
            );
            assert!(shapes[0]
                .image
                .as_ref()
                .unwrap()
                .starts_with("data:image/png;base64,"));
            assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
        }
    }

    #[test]
    fn smartart_cached_drawing_reuses_shapes_in_frame_coordinates() {
        let root=XmlParser::parse_str(r#"<p:sld xmlns:p="urn:p" xmlns:a="urn:a" xmlns:dgm="urn:dgm" xmlns:r="urn:r"><p:cSld><p:spTree><p:graphicFrame><p:xfrm><a:off x="100" y="200"/><a:ext cx="1000" cy="500"/></p:xfrm><a:graphic><a:graphicData><dgm:relIds r:dm="diagram"/></a:graphicData></a:graphic></p:graphicFrame></p:spTree></p:cSld></p:sld>"#).unwrap();
        let data = r#"<dgm:dataModel xmlns:dgm="urn:dgm" xmlns:dsp="urn:dsp"><dgm:extLst><dsp:dataModelExt relId="drawing"/></dgm:extLst></dgm:dataModel>"#;
        let relationships = r#"<Relationships><Relationship Id="drawing" Target="drawing1.xml" Type="http://schemas.microsoft.com/office/2007/relationships/diagramDrawing"/></Relationships>"#;
        let drawing = r#"<dsp:drawing xmlns:dsp="urn:dsp" xmlns:a="urn:a"><dsp:spTree><dsp:grpSpPr><a:xfrm><a:chOff x="0" y="0"/><a:chExt cx="1000" cy="500"/></a:xfrm></dsp:grpSpPr><dsp:sp><dsp:nvSpPr><dsp:cNvPr name="cached"/></dsp:nvSpPr><dsp:spPr><a:xfrm><a:off x="50" y="60"/><a:ext cx="200" cy="100"/></a:xfrm><a:prstGeom prst="roundRect"/></dsp:spPr><dsp:txBody><a:p><a:r><a:t>Cached node</a:t></a:r></a:p></dsp:txBody></dsp:sp></dsp:spTree></dsp:drawing>"#;
        let archive = Archive {
            parts: HashMap::from([
                ("ppt/diagrams/data1.xml".into(), data.as_bytes().to_vec()),
                (
                    "ppt/diagrams/_rels/data1.xml.rels".into(),
                    relationships.as_bytes().to_vec(),
                ),
                (
                    "ppt/diagrams/drawing1.xml".into(),
                    drawing.as_bytes().to_vec(),
                ),
            ]),
        };
        let rels = HashMap::from([(
            "diagram".into(),
            Rel {
                target: "ppt/diagrams/data1.xml".into(),
                rel_type: "diagramData".into(),
            },
        )]);
        let (mut shapes, mut tables, mut warnings) = (vec![], vec![], vec![]);
        parse_visual_tree(
            &root,
            "ppt/slides/slide1.xml",
            &rels,
            &archive,
            &Theme::default(),
            false,
            &HashMap::new(),
            None,
            &mut shapes,
            &mut tables,
            &mut warnings,
        );
        assert_eq!(shapes.len(), 1);
        assert!(warnings.is_empty());
        assert_eq!(shapes[0].paragraphs[0].runs[0].text, "Cached node");
        assert_eq!(
            shapes[0].transform.unwrap(),
            [1.0, 0.0, 0.0, 1.0, 150.0, 260.0]
        );
    }

    #[test]
    fn solid_text_fill_overrides_inherited_gradient() {
        let body=XmlParser::parse_str(r#"<a:txBody xmlns:a="urn:a"><a:lstStyle><a:lvl1pPr><a:defRPr><a:gradFill><a:gsLst><a:gs pos="0"><a:srgbClr val="FF0000"/></a:gs><a:gs pos="100000"><a:srgbClr val="0000FF"/></a:gs></a:gsLst></a:gradFill></a:defRPr></a:lvl1pPr></a:lstStyle><a:p><a:r><a:rPr><a:solidFill><a:srgbClr val="00FF00"/></a:solidFill></a:rPr><a:t>Solid</a:t></a:r><a:r><a:t>Gradient</a:t></a:r></a:p></a:txBody>"#).unwrap();
        let paragraphs = parse_paragraphs(&body, &Theme::default(), None);
        assert!(paragraphs[0].runs[0].color_gradient.is_none());
        assert_eq!(paragraphs[0].runs[0].color.as_deref(), Some("#00FF00"));
        assert_eq!(
            paragraphs[0].runs[1]
                .color_gradient
                .as_ref()
                .unwrap()
                .stops
                .len(),
            2
        );
    }

    #[test]
    fn source_samples_keep_pages_merged_text_and_known_style_without_warnings() {
        let samples = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../apps/ppt-view/public/samples");
        let doc = parse_document(&std::fs::read(samples.join("styles-ppt.pptx")).unwrap()).unwrap();
        assert_eq!(doc.slides.len(), 2);
        assert!(doc.warnings.is_empty(), "{:?}", doc.warnings);
        assert_eq!(doc.slides[1].tables[0].rows[1].cells[0].text, "r1c0\nr1c1");
        let arch = parse_document(&std::fs::read(samples.join("arch.pptx")).unwrap()).unwrap();
        assert_eq!(arch.slides.len(), 39);
        assert!(arch
            .slides
            .iter()
            .flat_map(|s| &s.shapes)
            .any(|s| s.vertical_text.as_deref() == Some("eaVert")));
    }

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
