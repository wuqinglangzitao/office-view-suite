//! `.docx` style sheet (`word/styles.xml`) → paragraph-style heading levels,
//! display names, and inherited run defaults, the OOXML analogue of the `.doc`
//! STSH resolver (`stsh.rs`).
//!
//! A heading level is derived from the `w:styleId` (`Heading1`…), the localized
//! `w:name` (`heading 1` / `제목 1`), or the style's own `w:outlineLvl` — reusing
//! [`crate::stsh::heading_from_name`] so both backends recognize the same names.

use std::collections::HashMap;

use quick_xml::events::{BytesDecl, BytesStart, Event};
use quick_xml::Reader;

use super::xml_text::{
    resolve_reference, skip_alternate_content_branch, skip_subtree, AlternateContentBranchState,
};
use super::{
    attr_local, attr_local_trimmed, attr_u16, attr_u8, local, parse_rgb_hex_color, toggle_on,
};
use crate::model::{
    CharProps, Color, Indent, LineSpacingHint, Spacing, TabAlignment, TabLeader, TabStop,
    VertAlign, MAX_TAB_STOPS,
};
use crate::stsh::heading_from_name;

const STYLE_CHAIN_LIMIT: usize = 32;

/// Resolved per-`styleId` heading level, display name, and run defaults.
#[derive(Debug, Default)]
pub(crate) struct Styles {
    heading: HashMap<String, u8>,
    name: HashMap<String, String>,
    doc_defaults_run: RunProps,
    doc_defaults_paragraph: ParagraphProps,
    default_paragraph_style: Option<String>,
    paragraph_run: HashMap<String, RunProps>,
    paragraph: HashMap<String, ParagraphProps>,
    character_run: HashMap<String, RunProps>,
    table_row: HashMap<String, TableRowStyleProps>,
    table_cell: HashMap<String, TableStyleCellProps>,
    table_borders: HashMap<String, super::body::TableBorderTuple>,
    table_geometry: HashMap<String, super::body::TableStyleGeometry>,
}

impl Styles {
    /// Heading level (1–9) for a paragraph `styleId`, or `None` for body styles.
    pub(crate) fn heading_level(&self, style_id: &str) -> Option<u8> {
        self.heading.get(style_id).copied()
    }

    /// Display name for a `styleId` (e.g. `heading 1`, `제목 1`), if known.
    pub(crate) fn name(&self, style_id: &str) -> Option<&str> {
        self.name
            .get(style_id)
            .map(String::as_str)
            .filter(|s| !s.is_empty())
    }

    pub(crate) fn resolved_run_props(
        &self,
        paragraph_style_id: Option<&str>,
        character_style_id: Option<&str>,
    ) -> RunProps {
        let mut props = self.doc_defaults_run.clone();
        let paragraph_style_id = paragraph_style_id.or(self.default_paragraph_style.as_deref());
        if let Some(style_id) = paragraph_style_id {
            if let Some(style_props) = self.paragraph_run.get(style_id) {
                props.overlay(style_props);
            }
        }
        if let Some(style_id) = character_style_id {
            if let Some(style_props) = self.character_run.get(style_id) {
                props.overlay(style_props);
            }
        }
        props
    }

    pub(crate) fn paragraph_props(&self, style_id: Option<&str>) -> ParagraphProps {
        let mut props = self.doc_defaults_paragraph.clone();
        let style_id = style_id.or(self.default_paragraph_style.as_deref());
        if let Some(style_id) = style_id {
            if let Some(style_props) = self.paragraph.get(style_id) {
                props.overlay(style_props);
            }
        }
        props
    }

    #[cfg(test)]
    fn table_row_cant_split(&self, style_id: Option<&str>) -> Option<bool> {
        style_id
            .and_then(|style_id| self.table_row.get(style_id))
            .and_then(|props| props.direct.cant_split)
    }

    pub(crate) fn table_row_cant_split_for_regions(
        &self,
        style_id: Option<&str>,
        regions: TableRowStyleRegions,
    ) -> Option<bool> {
        let props = style_id.and_then(|style_id| self.table_row.get(style_id))?;
        let mut value = props.direct.cant_split;
        // ISO/IEC 29500-1 17.7.6.6: later matching conditional regions
        // override earlier ones; direct row formatting is applied by body.rs.
        overlay_cant_split(&mut value, props.whole_table);
        if regions.band1_horizontal {
            overlay_cant_split(&mut value, props.band1_horizontal);
        }
        if regions.band2_horizontal {
            overlay_cant_split(&mut value, props.band2_horizontal);
        }
        if regions.band1_vertical {
            overlay_cant_split(&mut value, props.band1_vertical);
        }
        if regions.band2_vertical {
            overlay_cant_split(&mut value, props.band2_vertical);
        }
        if regions.first_column {
            overlay_cant_split(&mut value, props.first_column);
        }
        if regions.last_column {
            overlay_cant_split(&mut value, props.last_column);
        }
        if regions.first_row {
            overlay_cant_split(&mut value, props.first_row);
        }
        if regions.last_row {
            overlay_cant_split(&mut value, props.last_row);
        }
        if regions.north_west {
            overlay_cant_split(&mut value, props.north_west);
        }
        if regions.north_east {
            overlay_cant_split(&mut value, props.north_east);
        }
        if regions.south_west {
            overlay_cant_split(&mut value, props.south_west);
        }
        if regions.south_east {
            overlay_cant_split(&mut value, props.south_east);
        }
        value
    }

    #[cfg(test)]
    pub(crate) fn table_cell_presentation_for_regions(
        &self,
        style_id: Option<&str>,
        regions: TableRowStyleRegions,
    ) -> TableStyleCellPresentation {
        self.table_cell_props(style_id)
            .presentation_for_regions(regions.into())
    }

    pub(crate) fn table_cell_props(&self, style_id: Option<&str>) -> TableStyleCellProps {
        style_id
            .and_then(|style_id| self.table_cell.get(style_id))
            .copied()
            .unwrap_or_default()
    }

    /// A table style's borders, resolved through `basedOn` with its
    /// `wholeTable` region applied last. Row- and column-scoped regions stay
    /// unsupported.
    pub(crate) fn table_borders(
        &self,
        style_id: Option<&str>,
    ) -> Option<super::body::TableBorderTuple> {
        style_id
            .and_then(|style_id| self.table_borders.get(style_id))
            .copied()
    }

    /// A table style's own width, indent, and alignment, resolved through
    /// `basedOn` with its `wholeTable` region applied last.
    pub(crate) fn table_geometry(&self, style_id: Option<&str>) -> super::body::TableStyleGeometry {
        style_id
            .and_then(|style_id| self.table_geometry.get(style_id))
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn table_row_band_size(&self, style_id: Option<&str>) -> Option<u8> {
        style_id
            .and_then(|style_id| self.table_row.get(style_id))
            .and_then(|props| props.row_band_size)
    }

    pub(crate) fn table_col_band_size(&self, style_id: Option<&str>) -> Option<u8> {
        style_id
            .and_then(|style_id| self.table_cell.get(style_id))
            .and_then(|props| props.col_band_size)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TableRowStyleRegions {
    pub(crate) first_row: bool,
    pub(crate) last_row: bool,
    pub(crate) first_column: bool,
    pub(crate) last_column: bool,
    pub(crate) band1_vertical: bool,
    pub(crate) band2_vertical: bool,
    pub(crate) band1_horizontal: bool,
    pub(crate) band2_horizontal: bool,
    pub(crate) north_west: bool,
    pub(crate) north_east: bool,
    pub(crate) south_west: bool,
    pub(crate) south_east: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TableCellStyleRegions {
    pub(crate) first_row: bool,
    pub(crate) last_row: bool,
    pub(crate) first_column: bool,
    pub(crate) last_column: bool,
    pub(crate) band1_vertical: bool,
    pub(crate) band2_vertical: bool,
    pub(crate) band1_horizontal: bool,
    pub(crate) band2_horizontal: bool,
    pub(crate) north_east: bool,
    pub(crate) north_west: bool,
    pub(crate) south_east: bool,
    pub(crate) south_west: bool,
}

impl From<TableRowStyleRegions> for TableCellStyleRegions {
    fn from(regions: TableRowStyleRegions) -> Self {
        Self {
            first_row: regions.first_row,
            last_row: regions.last_row,
            first_column: regions.first_column,
            last_column: regions.last_column,
            band1_vertical: regions.band1_vertical,
            band2_vertical: regions.band2_vertical,
            band1_horizontal: regions.band1_horizontal,
            band2_horizontal: regions.band2_horizontal,
            north_west: regions.north_west,
            north_east: regions.north_east,
            south_west: regions.south_west,
            south_east: regions.south_east,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TableStyleCellPresentation {
    pub(crate) margins: super::body::CellMarginSpec,
    pub(crate) defaults: super::body::TableStyleCellDefaults,
}

impl TableStyleCellPresentation {
    fn overlay(&mut self, other: Self) {
        self.margins.overlay(other.margins);
        self.defaults.overlay(other.defaults);
    }

    fn is_empty(self) -> bool {
        self.margins.is_empty() && self.defaults.is_empty()
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
enum CascadedValue<T> {
    #[default]
    Inherit,
    Value(T),
    Suppress,
}

impl<T: Copy> CascadedValue<T> {
    fn overlay(&mut self, other: Self) {
        if !matches!(other, Self::Inherit) {
            *self = other;
        }
    }

    fn value(self) -> Option<T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Inherit | Self::Suppress => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParagraphLineRule {
    Auto,
    Exact,
    AtLeast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelativeUnitValue {
    Zero,
    NonZero,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct ParagraphLayoutProps {
    before_pt: CascadedValue<f32>,
    before_lines: CascadedValue<RelativeUnitValue>,
    before_auto: CascadedValue<bool>,
    after_pt: CascadedValue<f32>,
    after_lines: CascadedValue<RelativeUnitValue>,
    after_auto: CascadedValue<bool>,
    line: CascadedValue<f32>,
    line_rule: CascadedValue<ParagraphLineRule>,
    first_line_pt: CascadedValue<f32>,
    hanging_pt: CascadedValue<f32>,
    first_line_chars: CascadedValue<RelativeUnitValue>,
    hanging_chars: CascadedValue<RelativeUnitValue>,
    shading: CascadedValue<Color>,
    page_break_before: CascadedValue<bool>,
}

impl ParagraphLayoutProps {
    pub(crate) fn overlay(&mut self, other: Self) {
        self.before_pt.overlay(other.before_pt);
        self.before_lines.overlay(other.before_lines);
        self.before_auto.overlay(other.before_auto);
        self.after_pt.overlay(other.after_pt);
        self.after_lines.overlay(other.after_lines);
        self.after_auto.overlay(other.after_auto);
        self.line.overlay(other.line);
        self.line_rule.overlay(other.line_rule);
        self.first_line_pt.overlay(other.first_line_pt);
        self.hanging_pt.overlay(other.hanging_pt);
        self.first_line_chars.overlay(other.first_line_chars);
        self.hanging_chars.overlay(other.hanging_chars);
        self.shading.overlay(other.shading);
        self.page_break_before.overlay(other.page_break_before);
    }

    pub(crate) fn spacing(self) -> Spacing {
        Spacing {
            before_pt: resolved_paragraph_spacing(
                self.before_pt,
                self.before_lines,
                self.before_auto,
            ),
            after_pt: resolved_paragraph_spacing(self.after_pt, self.after_lines, self.after_auto),
            line_pct: match (self.line, self.line_rule) {
                (
                    CascadedValue::Value(line),
                    CascadedValue::Inherit | CascadedValue::Value(ParagraphLineRule::Auto),
                ) => {
                    let value = line / 240.0;
                    value
                        .is_finite()
                        .then_some(value)
                        .filter(|value| *value > 0.0)
                }
                _ => None,
            },
        }
    }

    pub(crate) fn line_spacing_hint(self) -> Option<LineSpacingHint> {
        let CascadedValue::Value(line) = self.line else {
            return None;
        };
        let points = line / 20.0;
        if !points.is_finite() || points <= 0.0 {
            return None;
        }
        match self.line_rule {
            CascadedValue::Value(ParagraphLineRule::Exact) => Some(LineSpacingHint::Exact(points)),
            CascadedValue::Value(ParagraphLineRule::AtLeast) => {
                Some(LineSpacingHint::AtLeast(points))
            }
            CascadedValue::Inherit
            | CascadedValue::Value(ParagraphLineRule::Auto)
            | CascadedValue::Suppress => None,
        }
    }

    pub(crate) fn apply_indent(self, indent: &mut Indent) {
        if !relative_unit_allows_twips(self.first_line_chars)
            || !relative_unit_allows_twips(self.hanging_chars)
        {
            indent.first_line_pt = None;
            indent.hanging_pt = None;
            return;
        }
        indent.first_line_pt = self.first_line_pt.value();
        indent.hanging_pt = self.hanging_pt.value();
    }

    pub(crate) fn shading(self) -> Option<Color> {
        self.shading.value()
    }

    pub(crate) fn page_break_before(self) -> bool {
        self.page_break_before.value().unwrap_or(false)
    }
}

fn resolved_paragraph_spacing(
    twips: CascadedValue<f32>,
    lines: CascadedValue<RelativeUnitValue>,
    automatic: CascadedValue<bool>,
) -> Option<f32> {
    if !matches!(
        automatic,
        CascadedValue::Inherit | CascadedValue::Value(false)
    ) {
        return None;
    }
    match lines {
        CascadedValue::Inherit => twips.value(),
        CascadedValue::Value(RelativeUnitValue::Zero) => match twips {
            CascadedValue::Inherit => Some(0.0),
            CascadedValue::Value(value) => Some(value),
            CascadedValue::Suppress => None,
        },
        CascadedValue::Value(RelativeUnitValue::NonZero) | CascadedValue::Suppress => None,
    }
}

fn relative_unit_allows_twips(value: CascadedValue<RelativeUnitValue>) -> bool {
    matches!(
        value,
        CascadedValue::Inherit | CascadedValue::Value(RelativeUnitValue::Zero)
    )
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ParagraphProps {
    pub(crate) bidi: Option<bool>,
    pub(crate) keep_next: Option<bool>,
    pub(crate) keep_lines: Option<bool>,
    pub(crate) widow_control: Option<bool>,
    pub(crate) jc: Option<String>,
    pub(crate) indent_left_pt: Option<f32>,
    pub(crate) indent_right_pt: Option<f32>,
    pub(crate) indent_start_pt: Option<f32>,
    pub(crate) indent_end_pt: Option<f32>,
    pub(crate) tab_stops: Vec<TabStop>,
    pub(crate) layout: ParagraphLayoutProps,
    /// List membership a paragraph style declares (`w:numPr`).
    pub(crate) num: Option<(String, u8)>,
}

impl ParagraphProps {
    fn overlay(&mut self, other: &ParagraphProps) {
        if other.num.is_some() {
            self.num = other.num.clone();
        }
        if other.bidi.is_some() {
            self.bidi = other.bidi;
        }
        if other.keep_next.is_some() {
            self.keep_next = other.keep_next;
        }
        if other.keep_lines.is_some() {
            self.keep_lines = other.keep_lines;
        }
        if other.widow_control.is_some() {
            self.widow_control = other.widow_control;
        }
        if other.jc.is_some() {
            self.jc = other.jc.clone();
        }
        if other.indent_left_pt.is_some() {
            self.indent_left_pt = other.indent_left_pt;
        }
        if other.indent_right_pt.is_some() {
            self.indent_right_pt = other.indent_right_pt;
        }
        if other.indent_start_pt.is_some() {
            self.indent_start_pt = other.indent_start_pt;
        }
        if other.indent_end_pt.is_some() {
            self.indent_end_pt = other.indent_end_pt;
        }
        let remaining = MAX_TAB_STOPS.saturating_sub(self.tab_stops.len());
        self.tab_stops
            .extend(other.tab_stops.iter().take(remaining).copied());
        self.layout.overlay(other.layout);
    }
}

fn twips_attr(e: &BytesStart<'_>, name: &[u8]) -> Option<f32> {
    attr_local(e, name)
        .and_then(|value| value.trim().parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .map(|value| value / 20.0)
}

fn nonnegative_twips(value: Option<String>) -> CascadedValue<f32> {
    let Some(value) = value else {
        return CascadedValue::Inherit;
    };
    value
        .trim()
        .parse::<u64>()
        .ok()
        .map(|value| value as f64 / 20.0)
        .filter(|value| value.is_finite() && *value <= f64::from(f32::MAX))
        .map(|value| CascadedValue::Value(value as f32))
        .unwrap_or(CascadedValue::Suppress)
}

fn positive_integer(value: Option<String>) -> CascadedValue<f32> {
    let Some(value) = value else {
        return CascadedValue::Inherit;
    };
    value
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .map(|value| value as f64)
        .filter(|value| value.is_finite() && *value <= f64::from(f32::MAX))
        .map(|value| CascadedValue::Value(value as f32))
        .unwrap_or(CascadedValue::Suppress)
}

fn relative_unit(value: Option<String>) -> CascadedValue<RelativeUnitValue> {
    let Some(value) = value else {
        return CascadedValue::Inherit;
    };
    match value.trim().parse::<i32>() {
        Ok(0) => CascadedValue::Value(RelativeUnitValue::Zero),
        Ok(_) => CascadedValue::Value(RelativeUnitValue::NonZero),
        Err(_) => CascadedValue::Suppress,
    }
}

fn strict_on_off(value: &str) -> Option<bool> {
    match value {
        "1" | "true" | "on" => Some(true),
        "0" | "false" | "off" => Some(false),
        _ => None,
    }
}

fn on_off_attribute(value: Option<String>) -> CascadedValue<bool> {
    let Some(value) = value else {
        return CascadedValue::Inherit;
    };
    strict_on_off(&value)
        .map(CascadedValue::Value)
        .unwrap_or(CascadedValue::Suppress)
}

fn on_off_element(e: &BytesStart<'_>) -> CascadedValue<bool> {
    attr_local(e, b"val").map_or(CascadedValue::Value(true), |value| {
        strict_on_off(&value)
            .map(CascadedValue::Value)
            .unwrap_or(CascadedValue::Suppress)
    })
}

fn line_rule(value: Option<String>) -> CascadedValue<ParagraphLineRule> {
    let Some(value) = value else {
        return CascadedValue::Inherit;
    };
    match value.trim() {
        "auto" => CascadedValue::Value(ParagraphLineRule::Auto),
        "exact" => CascadedValue::Value(ParagraphLineRule::Exact),
        "atLeast" => CascadedValue::Value(ParagraphLineRule::AtLeast),
        _ => CascadedValue::Suppress,
    }
}

fn paragraph_spacing(props: &mut ParagraphLayoutProps, e: &BytesStart<'_>) {
    props
        .before_pt
        .overlay(nonnegative_twips(attr_local(e, b"before")));
    props
        .before_lines
        .overlay(relative_unit(attr_local(e, b"beforeLines")));
    props
        .before_auto
        .overlay(on_off_attribute(attr_local(e, b"beforeAutospacing")));
    props
        .after_pt
        .overlay(nonnegative_twips(attr_local(e, b"after")));
    props
        .after_lines
        .overlay(relative_unit(attr_local(e, b"afterLines")));
    props
        .after_auto
        .overlay(on_off_attribute(attr_local(e, b"afterAutospacing")));
    let line = attr_local(e, b"line");
    let line_rule_value = attr_local(e, b"lineRule");
    props.line.overlay(positive_integer(line.clone()));
    if line_rule_value.is_some() {
        props.line_rule.overlay(line_rule(line_rule_value));
    } else if line.is_some() {
        props
            .line_rule
            .overlay(CascadedValue::Value(ParagraphLineRule::Auto));
    }
}

fn paragraph_first_line_indent(props: &mut ParagraphLayoutProps, e: &BytesStart<'_>) {
    let hanging_chars = attr_local(e, b"hangingChars");
    let hanging = attr_local(e, b"hanging");
    let first_line_chars = attr_local(e, b"firstLineChars");
    let first_line = attr_local(e, b"firstLine");

    props.hanging_chars.overlay(relative_unit(hanging_chars));
    props
        .first_line_chars
        .overlay(relative_unit(first_line_chars));
    if hanging.is_some() {
        props.first_line_pt = CascadedValue::Suppress;
        props.hanging_pt = nonnegative_twips(hanging);
    } else if first_line.is_some() {
        props.first_line_pt = nonnegative_twips(first_line);
        props.hanging_pt = CascadedValue::Suppress;
    }
}

fn paragraph_shading(e: &BytesStart<'_>) -> CascadedValue<Color> {
    let unsupported_theme = [b"themeFill".as_slice(), b"themeFillTint", b"themeFillShade"]
        .into_iter()
        .any(|name| attr_local(e, name).is_some());
    let supported_pattern = attr_local(e, b"val")
        .as_deref()
        .is_none_or(|value| value.trim() == "clear");
    if unsupported_theme || !supported_pattern {
        return CascadedValue::Suppress;
    }
    attr_local(e, b"fill")
        .and_then(|value| parse_rgb_hex_color(&value))
        .map(CascadedValue::Value)
        .unwrap_or(CascadedValue::Suppress)
}

pub(crate) fn apply_paragraph_layout_child(props: &mut ParagraphLayoutProps, e: &BytesStart<'_>) {
    match local(e.name().as_ref()) {
        b"spacing" => paragraph_spacing(props, e),
        b"ind" => paragraph_first_line_indent(props, e),
        b"shd" => props.shading = paragraph_shading(e),
        b"pageBreakBefore" => {
            props.page_break_before.overlay(on_off_element(e));
        }
        _ => {}
    }
}

pub(super) fn tab_stop(e: &BytesStart<'_>) -> Option<TabStop> {
    let position_pt = twips_attr(e, b"pos").filter(|position| *position >= 0.0)?;
    let alignment = match attr_local_trimmed(e, b"val").as_deref() {
        None | Some("left") | Some("start") => TabAlignment::Left,
        Some("center") => TabAlignment::Center,
        Some("right") | Some("end") => TabAlignment::Right,
        Some("decimal") | Some("num") => TabAlignment::Decimal,
        Some("bar") => TabAlignment::Bar,
        Some("clear") => TabAlignment::Clear,
        _ => return None,
    };
    let leader = match attr_local_trimmed(e, b"leader").as_deref() {
        Some("dot") => TabLeader::Dot,
        Some("hyphen") => TabLeader::Hyphen,
        Some("underscore") => TabLeader::Underscore,
        Some("heavy") => TabLeader::Heavy,
        Some("middleDot") => TabLeader::MiddleDot,
        _ => TabLeader::None,
    };
    Some(TabStop {
        position_pt,
        alignment,
        leader,
    })
}

fn apply_paragraph_props_child(props: &mut ParagraphProps, e: &BytesStart<'_>) {
    apply_paragraph_layout_child(&mut props.layout, e);
    match local(e.name().as_ref()) {
        b"bidi" => props.bidi = Some(toggle_on(attr_local(e, b"val"))),
        b"keepNext" => props.keep_next = Some(toggle_on(attr_local(e, b"val"))),
        b"keepLines" => props.keep_lines = Some(toggle_on(attr_local(e, b"val"))),
        b"widowControl" => props.widow_control = Some(toggle_on(attr_local(e, b"val"))),
        b"jc" => props.jc = attr_local_trimmed(e, b"val"),
        b"ind" => {
            props.indent_left_pt = twips_attr(e, b"left");
            props.indent_right_pt = twips_attr(e, b"right");
            props.indent_start_pt = twips_attr(e, b"start");
            props.indent_end_pt = twips_attr(e, b"end");
        }
        b"tab" if props.tab_stops.len() < MAX_TAB_STOPS => {
            if let Some(tab) = tab_stop(e) {
                props.tab_stops.push(tab);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RunProps {
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strike: Option<bool>,
    hidden: Option<bool>,
    font: Option<String>,
    font_cs: Option<String>,
    size_half_pt: Option<u16>,
    color: Option<Color>,
    highlight: Option<String>,
    vert_align: Option<VertAlign>,
    small_caps: Option<bool>,
    caps: Option<bool>,
    rtl: Option<bool>,
}

impl RunProps {
    pub(crate) fn apply_to(&self, props: &mut CharProps) {
        if let Some(value) = self.bold {
            props.bold = value;
        }
        if let Some(value) = self.italic {
            props.italic = value;
        }
        if let Some(value) = self.underline {
            props.underline = value;
        }
        if let Some(value) = self.strike {
            props.strike = value;
        }
        if let Some(value) = self.hidden {
            props.hidden = value;
        }
        if let Some(value) = &self.font {
            props.font = Some(value.clone());
        }
        if let Some(value) = self.size_half_pt {
            props.size_half_pt = Some(value);
        }
        if let Some(value) = self.color {
            props.color = Some(value);
        }
        if let Some(value) = &self.highlight {
            props.highlight = Some(value.clone());
        }
        if let Some(value) = self.vert_align {
            props.vert_align = value;
        }
        if let Some(value) = self.small_caps {
            props.small_caps = value;
        }
        if let Some(value) = self.caps {
            props.caps = value;
        }
        if let Some(value) = self.rtl {
            props.rtl = value;
        }
        if props.rtl {
            if let Some(value) = &self.font_cs {
                props.font = Some(value.clone());
            }
        }
    }

    pub(crate) fn overlay(&mut self, other: &RunProps) {
        if other.bold.is_some() {
            self.bold = other.bold;
        }
        if other.italic.is_some() {
            self.italic = other.italic;
        }
        if other.underline.is_some() {
            self.underline = other.underline;
        }
        if other.strike.is_some() {
            self.strike = other.strike;
        }
        if other.hidden.is_some() {
            self.hidden = other.hidden;
        }
        if other.font.is_some() {
            self.font = other.font.clone();
        }
        if other.font_cs.is_some() {
            self.font_cs = other.font_cs.clone();
        }
        if other.size_half_pt.is_some() {
            self.size_half_pt = other.size_half_pt;
        }
        if other.color.is_some() {
            self.color = other.color;
        }
        if other.highlight.is_some() {
            self.highlight = other.highlight.clone();
        }
        if other.vert_align.is_some() {
            self.vert_align = other.vert_align;
        }
        if other.small_caps.is_some() {
            self.small_caps = other.small_caps;
        }
        if other.caps.is_some() {
            self.caps = other.caps;
        }
        if other.rtl.is_some() {
            self.rtl = other.rtl;
        }
    }
}

pub(crate) fn apply_run_props_child(props: &mut RunProps, e: &BytesStart<'_>) {
    match local(e.name().as_ref()) {
        b"b" => props.bold = Some(toggle_on(attr_local(e, b"val"))),
        b"i" => props.italic = Some(toggle_on(attr_local(e, b"val"))),
        b"strike" | b"dstrike" => props.strike = Some(toggle_on(attr_local(e, b"val"))),
        b"vanish" => props.hidden = Some(toggle_on(attr_local(e, b"val"))),
        b"u" => {
            props.underline = Some(
                attr_local(e, b"val")
                    .map(|v| v.trim() != "none")
                    .unwrap_or(true),
            )
        }
        b"smallCaps" => props.small_caps = Some(toggle_on(attr_local(e, b"val"))),
        b"caps" => props.caps = Some(toggle_on(attr_local(e, b"val"))),
        b"rtl" => props.rtl = Some(toggle_on(attr_local(e, b"val"))),
        b"rFonts" => {
            props.font =
                attr_local_trimmed(e, b"eastAsia").or_else(|| attr_local_trimmed(e, b"ascii"));
            props.font_cs = attr_local_trimmed(e, b"cs");
        }
        b"sz" => props.size_half_pt = attr_u16(e, b"val"),
        b"color" => props.color = attr_local(e, b"val").and_then(|v| parse_rgb_hex_color(&v)),
        b"highlight" => props.highlight = attr_local_trimmed(e, b"val"),
        b"vertAlign" => {
            props.vert_align = Some(match attr_local_trimmed(e, b"val").as_deref() {
                Some("superscript") => VertAlign::Super,
                Some("subscript") => VertAlign::Sub,
                _ => VertAlign::Baseline,
            });
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParagraphPropertyTarget {
    DocumentDefaults,
    Style,
}

fn well_formed_xml(xml: &str) -> bool {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().check_comments = true;
    let mut depth = 0usize;
    let mut root_seen = false;
    let mut root_closed = false;
    let mut declaration_seen = false;
    let mut doctype_seen = false;
    let mut prolog_content_seen = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                if !valid_xml_attributes(&e) || (depth == 0 && root_seen) {
                    return false;
                }
                if depth == 0 {
                    root_seen = true;
                }
                let Some(next) = depth.checked_add(1) else {
                    return false;
                };
                depth = next;
            }
            Ok(Event::Empty(e)) => {
                if !valid_xml_attributes(&e) || (depth == 0 && root_seen) {
                    return false;
                }
                if depth == 0 {
                    root_seen = true;
                    root_closed = true;
                }
            }
            Ok(Event::End(_)) => {
                let Some(next) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next;
                if depth == 0 {
                    root_closed = true;
                }
            }
            Ok(Event::Text(e)) => {
                let Ok(value) = e.decode() else {
                    return false;
                };
                if !valid_xml_chars(&value) {
                    return false;
                }
                if depth == 0 {
                    if !e.as_ref().iter().all(u8::is_ascii_whitespace) {
                        return false;
                    }
                    if !root_seen {
                        prolog_content_seen = true;
                    }
                }
            }
            Ok(Event::GeneralRef(reference)) => {
                let Some(value) = resolve_reference(&reference) else {
                    return false;
                };
                if depth == 0 || !valid_xml_chars(&value) {
                    return false;
                }
            }
            Ok(Event::CData(e)) => {
                let Ok(value) = std::str::from_utf8(e.as_ref()) else {
                    return false;
                };
                if depth == 0 || !valid_xml_chars(value) {
                    return false;
                }
            }
            Ok(Event::Decl(e)) => {
                if declaration_seen
                    || root_seen
                    || depth != 0
                    || doctype_seen
                    || prolog_content_seen
                    || !valid_xml_declaration(&e)
                {
                    return false;
                }
                declaration_seen = true;
            }
            Ok(Event::DocType(_)) => {
                if doctype_seen || root_seen || depth != 0 {
                    return false;
                }
                doctype_seen = true;
                prolog_content_seen = true;
            }
            Ok(Event::Comment(_) | Event::PI(_)) if depth == 0 && !root_seen => {
                prolog_content_seen = true;
            }
            Ok(Event::Eof) => return root_seen && root_closed && depth == 0,
            Err(_) => return false,
            _ => {}
        }
    }
}

fn valid_xml_attributes(e: &BytesStart<'_>) -> bool {
    e.attributes().all(|attribute| {
        let Ok(attribute) = attribute else {
            return false;
        };
        attribute
            .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, e.decoder())
            .ok()
            .is_some_and(|value| valid_xml_chars(&value))
    })
}

fn valid_xml_chars(value: &str) -> bool {
    value.chars().all(|character| {
        matches!(character, '\u{9}' | '\u{A}' | '\u{D}')
            || ('\u{20}'..='\u{D7FF}').contains(&character)
            || ('\u{E000}'..='\u{FFFD}').contains(&character)
            || ('\u{10000}'..='\u{10FFFF}').contains(&character)
    })
}

fn valid_xml_declaration(declaration: &BytesDecl<'_>) -> bool {
    let Ok(content) = std::str::from_utf8(declaration.as_ref()) else {
        return false;
    };
    let start = BytesStart::from_content(content, 3);
    let mut attributes = start.attributes();
    let Some(Ok(version)) = attributes.next() else {
        return false;
    };
    if version.key.as_ref() != b"version" {
        return false;
    }
    if !matches!(version.value.as_ref(), b"1.0" | b"1.1") {
        return false;
    }

    let mut encoding_seen = false;
    let mut standalone_seen = false;
    for attribute in attributes {
        let Ok(attribute) = attribute else {
            return false;
        };
        let value = attribute.value.as_ref();
        match attribute.key.as_ref() {
            b"encoding"
                if !encoding_seen && !standalone_seen && value.eq_ignore_ascii_case(b"UTF-8") =>
            {
                encoding_seen = true;
            }
            b"standalone" if !standalone_seen && matches!(value, b"yes" | b"no") => {
                standalone_seen = true;
            }
            _ => return false,
        }
    }
    true
}

fn paragraph_property_scope_start(name: &[u8]) -> bool {
    matches!(
        name,
        b"Choice"
            | b"Fallback"
            | b"bidi"
            | b"keepNext"
            | b"keepLines"
            | b"widowControl"
            | b"jc"
            | b"ind"
            | b"spacing"
            | b"shd"
            | b"pageBreakBefore"
            | b"outlineLvl"
    )
}

fn read_paragraph_tabs(r: &mut Reader<&[u8]>, props: &mut ParagraphProps) {
    let mut alternate_content_stack = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e))
                if skip_alternate_content_branch(
                    &mut alternate_content_stack,
                    local(e.name().as_ref()),
                ) =>
            {
                skip_subtree(r);
            }
            Ok(Event::Empty(e))
                if skip_alternate_content_branch(
                    &mut alternate_content_stack,
                    local(e.name().as_ref()),
                ) => {}
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                if alternate_content_stack.len() >= STYLE_CHAIN_LIMIT {
                    skip_subtree(r);
                } else {
                    alternate_content_stack.push(AlternateContentBranchState::default());
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tab" => {
                if props.tab_stops.len() < MAX_TAB_STOPS {
                    if let Some(tab) = tab_stop(&e) {
                        props.tab_stops.push(tab);
                    }
                }
                skip_subtree(r);
            }
            Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"tab" && props.tab_stops.len() < MAX_TAB_STOPS =>
            {
                if let Some(tab) = tab_stop(&e) {
                    props.tab_stops.push(tab);
                }
            }
            Ok(Event::Start(e)) if matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") => {}
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.pop();
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"tabs" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

/// Parse `word/styles.xml`. Returns an empty sheet on absence/malformation —
/// headings then simply aren't detected (lists/body text are unaffected).
pub(crate) fn parse(xml: &str) -> Styles {
    if !well_formed_xml(xml) {
        return Styles::default();
    }
    let mut r = Reader::from_str(xml);
    let mut styles = Styles::default();
    let mut raw_styles: HashMap<String, RawStyle> = HashMap::new();
    // State for the style currently being parsed.
    let mut cur_style: Option<RawStyle> = None;
    let mut in_doc_defaults = false;
    let mut in_rpr_default = false;
    let mut in_ppr_default = false;
    let mut paragraph_property_target = None;
    let mut alternate_content_stack = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e))
                if skip_alternate_content_branch(
                    &mut alternate_content_stack,
                    local(e.name().as_ref()),
                ) =>
            {
                skip_subtree(&mut r);
            }
            Ok(Event::Empty(e))
                if skip_alternate_content_branch(
                    &mut alternate_content_stack,
                    local(e.name().as_ref()),
                ) => {}
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.push(AlternateContentBranchState::default());
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"docDefaults" => {
                in_doc_defaults = true;
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"rPrDefault" => {
                in_rpr_default = true;
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"pPrDefault" => {
                in_ppr_default = true;
            }
            // A new <w:style> opens; capture its id and reset per-style state.
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"style" => {
                paragraph_property_target = None;
                let kind = StyleKind::from_attr(attr_local_trimmed(&e, b"type").as_deref());
                cur_style = attr_local_trimmed(&e, b"styleId").map(|id| {
                    if kind == Some(StyleKind::Paragraph)
                        && attr_local(&e, b"default")
                            .as_deref()
                            .and_then(strict_on_off)
                            == Some(true)
                    {
                        styles.default_paragraph_style = Some(id.clone());
                    }
                    RawStyle {
                        id,
                        kind,
                        ..RawStyle::default()
                    }
                });
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"style" => {
                if let Some(id) = attr_local_trimmed(&e, b"styleId") {
                    let kind = StyleKind::from_attr(attr_local_trimmed(&e, b"type").as_deref());
                    if kind == Some(StyleKind::Paragraph)
                        && attr_local(&e, b"default")
                            .as_deref()
                            .and_then(strict_on_off)
                            == Some(true)
                    {
                        styles.default_paragraph_style = Some(id.clone());
                    }
                    raw_styles.insert(
                        id.clone(),
                        RawStyle {
                            id,
                            kind,
                            ..RawStyle::default()
                        },
                    );
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"pPrChange" => {
                skip_subtree(&mut r);
            }
            Ok(Event::Start(e))
                if local(e.name().as_ref()) == b"pPr" && paragraph_property_target.is_some() =>
            {
                skip_subtree(&mut r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"pPr" => {
                paragraph_property_target = if in_doc_defaults && in_ppr_default {
                    Some(ParagraphPropertyTarget::DocumentDefaults)
                } else if cur_style
                    .as_ref()
                    .is_some_and(|style| style.kind == Some(StyleKind::Paragraph))
                {
                    Some(ParagraphPropertyTarget::Style)
                } else {
                    None
                };
            }
            Ok(Event::Start(e))
                if paragraph_property_target.is_some() && local(e.name().as_ref()) == b"numPr" =>
            {
                let mut num_id = None;
                let mut ilvl = 0u8;
                super::body::read_num_pr_content(&mut r, &mut num_id, &mut ilvl, b"numPr", 0);
                let value = num_id.map(|id| (id, ilvl));
                match paragraph_property_target {
                    Some(ParagraphPropertyTarget::DocumentDefaults) => {
                        styles.doc_defaults_paragraph.num = value;
                    }
                    Some(ParagraphPropertyTarget::Style) => {
                        if let Some(style) = &mut cur_style {
                            style.paragraph_props.num = value;
                        }
                    }
                    None => {}
                }
            }
            Ok(Event::Start(e))
                if paragraph_property_target.is_some() && local(e.name().as_ref()) == b"tabs" =>
            {
                match paragraph_property_target {
                    Some(ParagraphPropertyTarget::DocumentDefaults) => {
                        read_paragraph_tabs(&mut r, &mut styles.doc_defaults_paragraph);
                    }
                    Some(ParagraphPropertyTarget::Style) => {
                        if let Some(style) = &mut cur_style {
                            read_paragraph_tabs(&mut r, &mut style.paragraph_props);
                        } else {
                            skip_subtree(&mut r);
                        }
                    }
                    None => {}
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblPr" => {
                if let Some(style) = &mut cur_style {
                    if style.kind == Some(StyleKind::Table) {
                        let values = read_style_table_properties(&mut r, b"tblPr", 0);
                        style
                            .table_cell_props
                            .direct
                            .margins
                            .overlay(values.margins);
                        if values.borders.is_some() {
                            style.table_borders = values.borders;
                        }
                        style.table_geometry.overlay(values.geometry);
                        if values.row_band_size.is_some() {
                            style.table_row_props.row_band_size = values.row_band_size;
                        }
                        if values.col_band_size.is_some() {
                            style.table_cell_props.col_band_size = values.col_band_size;
                        }
                    } else {
                        skip_subtree(&mut r);
                    }
                } else {
                    skip_subtree(&mut r);
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblStylePr" => {
                let region =
                    TableStyleRegion::from_attr(attr_local_trimmed(&e, b"type").as_deref());
                if let (Some(style), Some(region)) = (&mut cur_style, region) {
                    if style.kind == Some(StyleKind::Table) {
                        let values = read_conditional_table_region(&mut r, b"tblStylePr", 0);
                        if let Some(row) = style.table_row_props.region_mut(region) {
                            row.overlay(values.row);
                        }
                        style
                            .table_cell_props
                            .region_mut(region)
                            .overlay(values.presentation);
                        if matches!(region, TableStyleRegion::WholeTable) {
                            if values.borders.is_some() {
                                style.whole_table_borders = values.borders;
                            }
                            style.whole_table_geometry.overlay(values.geometry);
                        }
                    } else {
                        skip_subtree(&mut r);
                    }
                } else {
                    skip_subtree(&mut r);
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"trPr" => {
                if let Some(style) = &mut cur_style {
                    if style.kind == Some(StyleKind::Table) {
                        style
                            .table_row_props
                            .direct
                            .overlay(read_table_row_props(&mut r, b"trPr"));
                    } else {
                        skip_subtree(&mut r);
                    }
                } else {
                    skip_subtree(&mut r);
                }
            }
            Ok(Event::Start(e))
                if paragraph_property_target.is_some()
                    && !paragraph_property_scope_start(local(e.name().as_ref())) =>
            {
                skip_subtree(&mut r);
            }
            Ok(event @ (Event::Start(_) | Event::Empty(_))) => {
                let (e, is_start) = match event {
                    Event::Start(e) => (e, true),
                    Event::Empty(e) => (e, false),
                    _ => unreachable!(),
                };
                let qname = e.name();
                let name = local(qname.as_ref());
                if paragraph_property_target.is_some() {
                    match name {
                        b"bidi" | b"keepNext" | b"keepLines" | b"widowControl" | b"jc" | b"ind"
                        | b"spacing" | b"shd" | b"pageBreakBefore" => {
                            if paragraph_property_target
                                == Some(ParagraphPropertyTarget::DocumentDefaults)
                            {
                                apply_paragraph_props_child(&mut styles.doc_defaults_paragraph, &e);
                            } else if paragraph_property_target
                                == Some(ParagraphPropertyTarget::Style)
                            {
                                if let Some(style) = &mut cur_style {
                                    apply_paragraph_props_child(&mut style.paragraph_props, &e);
                                }
                            }
                            if is_start {
                                skip_subtree(&mut r);
                            }
                        }
                        b"outlineLvl"
                            if paragraph_property_target
                                == Some(ParagraphPropertyTarget::Style) =>
                        {
                            if let Some(style) = &mut cur_style {
                                style.outline = attr_u8(&e, b"val");
                            }
                            if is_start {
                                skip_subtree(&mut r);
                            }
                        }
                        b"Choice" | b"Fallback" => {}
                        _ => {}
                    }
                    continue;
                }

                match name {
                    // An empty `<w:rPr/>` carries no run properties, so both rPr targets
                    // (doc defaults, current style) only act on a non-empty element.
                    b"rPr" if is_start => {
                        if in_doc_defaults && in_rpr_default {
                            styles.doc_defaults_run = read_run_props(&mut r, b"rPr");
                        } else if let Some(style) = &mut cur_style {
                            style.run_props = read_run_props(&mut r, b"rPr");
                        } else {
                            skip_subtree(&mut r);
                        }
                    }
                    b"name" => {
                        if let Some(v) = attr_local_trimmed(&e, b"val") {
                            if let Some(style) = &mut cur_style {
                                style.name = v;
                            }
                        }
                        if is_start {
                            skip_subtree(&mut r);
                        }
                    }
                    b"basedOn" => {
                        if let Some(v) = attr_local_trimmed(&e, b"val") {
                            if let Some(style) = &mut cur_style {
                                style.based_on = Some(v);
                            }
                        }
                        if is_start {
                            skip_subtree(&mut r);
                        }
                    }
                    b"Choice" | b"Fallback" => {}
                    _ if is_start && cur_style.is_some() => skip_subtree(&mut r),
                    _ => {}
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"pPr" => {
                paragraph_property_target = None;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"style" => {
                paragraph_property_target = None;
                if let Some(style) = cur_style.take() {
                    raw_styles.insert(style.id.clone(), style);
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"rPrDefault" => {
                in_rpr_default = false;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"pPrDefault" => {
                in_ppr_default = false;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"docDefaults" => {
                in_doc_defaults = false;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    for style in raw_styles.values() {
        let level = heading_from_name(&style.id)
            .or_else(|| heading_from_name(&style.name))
            .or_else(|| style.outline.filter(|&o| o <= 8).map(|o| o + 1));
        if let Some(level) = level {
            styles.heading.insert(style.id.clone(), level);
        }
        if !style.name.is_empty() {
            styles.name.insert(style.id.clone(), style.name.clone());
        }
    }
    let paragraph_ids = raw_styles
        .iter()
        .filter(|(_, style)| style.kind == Some(StyleKind::Paragraph))
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    let character_ids = raw_styles
        .iter()
        .filter(|(_, style)| style.kind == Some(StyleKind::Character))
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    let mut paragraph_cache = HashMap::new();
    for id in paragraph_ids {
        let props = resolve_style_run_props(
            &id,
            StyleKind::Paragraph,
            &raw_styles,
            &mut paragraph_cache,
            &mut Vec::new(),
            0,
        );
        styles.paragraph_run.insert(id.clone(), props);
        let paragraph_props = resolve_style_paragraph_props(&id, &raw_styles, &mut Vec::new(), 0);
        styles.paragraph.insert(id, paragraph_props);
    }
    let mut character_cache = HashMap::new();
    for id in character_ids {
        let props = resolve_style_run_props(
            &id,
            StyleKind::Character,
            &raw_styles,
            &mut character_cache,
            &mut Vec::new(),
            0,
        );
        styles.character_run.insert(id, props);
    }
    for (id, _) in raw_styles
        .iter()
        .filter(|(_, style)| style.kind == Some(StyleKind::Table))
    {
        let props = resolve_style_table_row_props(id, &raw_styles, &mut Vec::new(), 0);
        if props.has_any() {
            styles.table_row.insert(id.clone(), props);
        }
        let cell_props = resolve_style_table_cell_props(id, &raw_styles, &mut Vec::new(), 0);
        if !cell_props.is_empty() {
            styles.table_cell.insert(id.clone(), cell_props);
        }
        if let Some(borders) = resolve_style_table_borders(id, &raw_styles, &mut Vec::new(), 0) {
            styles.table_borders.insert(id.clone(), borders);
        }
        let geometry = resolve_style_table_geometry(id, &raw_styles, &mut Vec::new(), 0);
        if !geometry.is_empty() {
            styles.table_geometry.insert(id.clone(), geometry);
        }
    }
    styles
}

#[derive(Debug, Default)]
struct RawStyle {
    id: String,
    kind: Option<StyleKind>,
    name: String,
    based_on: Option<String>,
    table_cell_props: TableStyleCellProps,
    table_borders: Option<super::body::TableBorderTuple>,
    whole_table_borders: Option<super::body::TableBorderTuple>,
    table_geometry: super::body::TableStyleGeometry,
    whole_table_geometry: super::body::TableStyleGeometry,
    outline: Option<u8>,
    run_props: RunProps,
    paragraph_props: ParagraphProps,
    table_row_props: TableRowStyleProps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StyleKind {
    Paragraph,
    Character,
    Table,
}

impl StyleKind {
    fn from_attr(value: Option<&str>) -> Option<Self> {
        match value {
            None | Some("paragraph") => Some(Self::Paragraph),
            Some("character") => Some(Self::Character),
            Some("table") => Some(Self::Table),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct TableRowProps {
    cant_split: Option<bool>,
}

impl TableRowProps {
    fn overlay(&mut self, other: Self) {
        if other.cant_split.is_some() {
            self.cant_split = other.cant_split;
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct TableRowStyleProps {
    direct: TableRowProps,
    whole_table: TableRowProps,
    band1_horizontal: TableRowProps,
    band2_horizontal: TableRowProps,
    band1_vertical: TableRowProps,
    band2_vertical: TableRowProps,
    first_column: TableRowProps,
    last_column: TableRowProps,
    first_row: TableRowProps,
    last_row: TableRowProps,
    north_west: TableRowProps,
    north_east: TableRowProps,
    south_west: TableRowProps,
    south_east: TableRowProps,
    row_band_size: Option<u8>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TableStyleCellProps {
    direct: TableStyleCellPresentation,
    whole_table: TableStyleCellPresentation,
    band1_horizontal: TableStyleCellPresentation,
    band2_horizontal: TableStyleCellPresentation,
    band1_vertical: TableStyleCellPresentation,
    band2_vertical: TableStyleCellPresentation,
    first_column: TableStyleCellPresentation,
    last_column: TableStyleCellPresentation,
    first_row: TableStyleCellPresentation,
    last_row: TableStyleCellPresentation,
    north_west: TableStyleCellPresentation,
    north_east: TableStyleCellPresentation,
    south_west: TableStyleCellPresentation,
    south_east: TableStyleCellPresentation,
    col_band_size: Option<u8>,
}

impl TableStyleCellProps {
    fn overlay(&mut self, other: Self) {
        self.direct.overlay(other.direct);
        self.whole_table.overlay(other.whole_table);
        self.band1_horizontal.overlay(other.band1_horizontal);
        self.band2_horizontal.overlay(other.band2_horizontal);
        self.band1_vertical.overlay(other.band1_vertical);
        self.band2_vertical.overlay(other.band2_vertical);
        self.first_column.overlay(other.first_column);
        self.last_column.overlay(other.last_column);
        self.first_row.overlay(other.first_row);
        self.last_row.overlay(other.last_row);
        self.north_west.overlay(other.north_west);
        self.north_east.overlay(other.north_east);
        self.south_west.overlay(other.south_west);
        self.south_east.overlay(other.south_east);
        if other.col_band_size.is_some() {
            self.col_band_size = other.col_band_size;
        }
    }

    fn region_mut(&mut self, region: TableStyleRegion) -> &mut TableStyleCellPresentation {
        match region {
            TableStyleRegion::WholeTable => &mut self.whole_table,
            TableStyleRegion::Band1Horizontal => &mut self.band1_horizontal,
            TableStyleRegion::Band2Horizontal => &mut self.band2_horizontal,
            TableStyleRegion::Band1Vertical => &mut self.band1_vertical,
            TableStyleRegion::Band2Vertical => &mut self.band2_vertical,
            TableStyleRegion::FirstColumn => &mut self.first_column,
            TableStyleRegion::LastColumn => &mut self.last_column,
            TableStyleRegion::FirstRow => &mut self.first_row,
            TableStyleRegion::LastRow => &mut self.last_row,
            TableStyleRegion::NorthWest => &mut self.north_west,
            TableStyleRegion::NorthEast => &mut self.north_east,
            TableStyleRegion::SouthWest => &mut self.south_west,
            TableStyleRegion::SouthEast => &mut self.south_east,
        }
    }

    pub(crate) fn presentation_for_regions(
        self,
        regions: TableCellStyleRegions,
    ) -> TableStyleCellPresentation {
        let mut presentation = TableStyleCellPresentation::default();
        presentation.overlay(self.direct);
        presentation.overlay(self.whole_table);
        // MS-OI29500 2.1.252: later matching regions override earlier ones.
        if regions.band1_horizontal {
            presentation.overlay(self.band1_horizontal);
        }
        if regions.band2_horizontal {
            presentation.overlay(self.band2_horizontal);
        }
        if regions.band1_vertical {
            presentation.overlay(self.band1_vertical);
        }
        if regions.band2_vertical {
            presentation.overlay(self.band2_vertical);
        }
        if regions.first_column {
            presentation.overlay(self.first_column);
        }
        if regions.last_column {
            presentation.overlay(self.last_column);
        }
        if regions.first_row {
            presentation.overlay(self.first_row);
        }
        if regions.last_row {
            presentation.overlay(self.last_row);
        }
        if regions.north_west {
            presentation.overlay(self.north_west);
        }
        if regions.north_east {
            presentation.overlay(self.north_east);
        }
        if regions.south_west {
            presentation.overlay(self.south_west);
        }
        if regions.south_east {
            presentation.overlay(self.south_east);
        }
        presentation
    }

    fn is_empty(self) -> bool {
        self.direct.is_empty()
            && self.whole_table.is_empty()
            && self.band1_horizontal.is_empty()
            && self.band2_horizontal.is_empty()
            && self.band1_vertical.is_empty()
            && self.band2_vertical.is_empty()
            && self.first_column.is_empty()
            && self.last_column.is_empty()
            && self.first_row.is_empty()
            && self.last_row.is_empty()
            && self.north_west.is_empty()
            && self.north_east.is_empty()
            && self.south_west.is_empty()
            && self.south_east.is_empty()
            && self.col_band_size.is_none()
    }
}

impl TableRowStyleProps {
    fn has_any(self) -> bool {
        self.direct.cant_split.is_some()
            || self.whole_table.cant_split.is_some()
            || self.band1_horizontal.cant_split.is_some()
            || self.band2_horizontal.cant_split.is_some()
            || self.band1_vertical.cant_split.is_some()
            || self.band2_vertical.cant_split.is_some()
            || self.first_column.cant_split.is_some()
            || self.last_column.cant_split.is_some()
            || self.first_row.cant_split.is_some()
            || self.last_row.cant_split.is_some()
            || self.north_west.cant_split.is_some()
            || self.north_east.cant_split.is_some()
            || self.south_west.cant_split.is_some()
            || self.south_east.cant_split.is_some()
            || self.row_band_size.is_some()
    }

    fn overlay(&mut self, other: Self) {
        self.direct.overlay(other.direct);
        self.whole_table.overlay(other.whole_table);
        self.band1_horizontal.overlay(other.band1_horizontal);
        self.band2_horizontal.overlay(other.band2_horizontal);
        self.band1_vertical.overlay(other.band1_vertical);
        self.band2_vertical.overlay(other.band2_vertical);
        self.first_column.overlay(other.first_column);
        self.last_column.overlay(other.last_column);
        self.first_row.overlay(other.first_row);
        self.last_row.overlay(other.last_row);
        self.north_west.overlay(other.north_west);
        self.north_east.overlay(other.north_east);
        self.south_west.overlay(other.south_west);
        self.south_east.overlay(other.south_east);
        if other.row_band_size.is_some() {
            self.row_band_size = other.row_band_size;
        }
    }

    fn region_mut(&mut self, region: TableStyleRegion) -> Option<&mut TableRowProps> {
        match region {
            TableStyleRegion::WholeTable => Some(&mut self.whole_table),
            TableStyleRegion::Band1Horizontal => Some(&mut self.band1_horizontal),
            TableStyleRegion::Band2Horizontal => Some(&mut self.band2_horizontal),
            TableStyleRegion::Band1Vertical => Some(&mut self.band1_vertical),
            TableStyleRegion::Band2Vertical => Some(&mut self.band2_vertical),
            TableStyleRegion::FirstColumn => Some(&mut self.first_column),
            TableStyleRegion::LastColumn => Some(&mut self.last_column),
            TableStyleRegion::FirstRow => Some(&mut self.first_row),
            TableStyleRegion::LastRow => Some(&mut self.last_row),
            TableStyleRegion::NorthWest => Some(&mut self.north_west),
            TableStyleRegion::NorthEast => Some(&mut self.north_east),
            TableStyleRegion::SouthWest => Some(&mut self.south_west),
            TableStyleRegion::SouthEast => Some(&mut self.south_east),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum TableStyleRegion {
    WholeTable,
    Band1Horizontal,
    Band2Horizontal,
    Band1Vertical,
    Band2Vertical,
    FirstColumn,
    LastColumn,
    FirstRow,
    LastRow,
    NorthWest,
    NorthEast,
    SouthWest,
    SouthEast,
}

impl TableStyleRegion {
    fn from_attr(value: Option<&str>) -> Option<Self> {
        match value {
            Some("wholeTable") => Some(Self::WholeTable),
            Some("band1Horz") => Some(Self::Band1Horizontal),
            Some("band2Horz") => Some(Self::Band2Horizontal),
            Some("band1Vert") => Some(Self::Band1Vertical),
            Some("band2Vert") => Some(Self::Band2Vertical),
            Some("firstCol") => Some(Self::FirstColumn),
            Some("lastCol") => Some(Self::LastColumn),
            Some("firstRow") => Some(Self::FirstRow),
            Some("lastRow") => Some(Self::LastRow),
            Some("nwCell") => Some(Self::NorthWest),
            Some("neCell") => Some(Self::NorthEast),
            Some("swCell") => Some(Self::SouthWest),
            Some("seCell") => Some(Self::SouthEast),
            _ => None,
        }
    }
}

fn overlay_cant_split(value: &mut Option<bool>, props: TableRowProps) {
    if props.cant_split.is_some() {
        *value = props.cant_split;
    }
}

fn resolve_style_run_props(
    id: &str,
    kind: StyleKind,
    raw_styles: &HashMap<String, RawStyle>,
    cache: &mut HashMap<String, RunProps>,
    stack: &mut Vec<String>,
    depth: usize,
) -> RunProps {
    if let Some(props) = cache.get(id) {
        return props.clone();
    }
    if depth >= STYLE_CHAIN_LIMIT || stack.iter().any(|seen| seen == id) {
        return RunProps::default();
    }
    let Some(style) = raw_styles.get(id).filter(|style| style.kind == Some(kind)) else {
        return RunProps::default();
    };

    stack.push(id.to_string());
    let mut props = style
        .based_on
        .as_deref()
        .map(|base| resolve_style_run_props(base, kind, raw_styles, cache, stack, depth + 1))
        .unwrap_or_default();
    props.overlay(&style.run_props);
    stack.pop();

    cache.insert(id.to_string(), props.clone());
    props
}

fn resolve_style_paragraph_props(
    id: &str,
    raw_styles: &HashMap<String, RawStyle>,
    stack: &mut Vec<String>,
    depth: usize,
) -> ParagraphProps {
    if depth >= STYLE_CHAIN_LIMIT || stack.iter().any(|seen| seen == id) {
        return ParagraphProps::default();
    }
    let Some(style) = raw_styles
        .get(id)
        .filter(|style| style.kind == Some(StyleKind::Paragraph))
    else {
        return ParagraphProps::default();
    };

    stack.push(id.to_string());
    let mut props = style
        .based_on
        .as_deref()
        .map(|base| resolve_style_paragraph_props(base, raw_styles, stack, depth + 1))
        .unwrap_or_default();
    props.overlay(&style.paragraph_props);
    stack.pop();
    props
}

fn resolve_style_table_row_props(
    id: &str,
    raw_styles: &HashMap<String, RawStyle>,
    stack: &mut Vec<String>,
    depth: usize,
) -> TableRowStyleProps {
    if depth >= STYLE_CHAIN_LIMIT || stack.iter().any(|seen| seen == id) {
        return TableRowStyleProps::default();
    }
    let Some(style) = raw_styles
        .get(id)
        .filter(|style| style.kind == Some(StyleKind::Table))
    else {
        return TableRowStyleProps::default();
    };

    stack.push(id.to_string());
    let mut props = style
        .based_on
        .as_deref()
        .map(|base| resolve_style_table_row_props(base, raw_styles, stack, depth + 1))
        .unwrap_or_default();
    props.overlay(style.table_row_props);
    stack.pop();
    props
}

fn resolve_style_table_cell_props(
    id: &str,
    raw_styles: &HashMap<String, RawStyle>,
    stack: &mut Vec<String>,
    depth: usize,
) -> TableStyleCellProps {
    if depth >= STYLE_CHAIN_LIMIT || stack.iter().any(|seen| seen == id) {
        return TableStyleCellProps::default();
    }
    let Some(style) = raw_styles
        .get(id)
        .filter(|style| style.kind == Some(StyleKind::Table))
    else {
        return TableStyleCellProps::default();
    };
    stack.push(id.to_string());
    let mut props = style
        .based_on
        .as_deref()
        .map(|base| resolve_style_table_cell_props(base, raw_styles, stack, depth + 1))
        .unwrap_or_default();
    props.overlay(style.table_cell_props);
    stack.pop();
    props
}

fn resolve_style_table_borders(
    id: &str,
    raw_styles: &HashMap<String, RawStyle>,
    stack: &mut Vec<String>,
    depth: usize,
) -> Option<super::body::TableBorderTuple> {
    if depth >= STYLE_CHAIN_LIMIT || stack.iter().any(|seen| seen == id) {
        return None;
    }
    let style = raw_styles
        .get(id)
        .filter(|style| style.kind == Some(StyleKind::Table))?;
    stack.push(id.to_string());
    let mut borders = style
        .based_on
        .as_deref()
        .and_then(|base| resolve_style_table_borders(base, raw_styles, stack, depth + 1));
    if style.table_borders.is_some() {
        borders = style.table_borders;
    }
    if style.whole_table_borders.is_some() {
        borders = style.whole_table_borders;
    }
    stack.pop();
    borders
}

fn resolve_style_table_geometry(
    id: &str,
    raw_styles: &HashMap<String, RawStyle>,
    stack: &mut Vec<String>,
    depth: usize,
) -> super::body::TableStyleGeometry {
    if depth >= STYLE_CHAIN_LIMIT || stack.iter().any(|seen| seen == id) {
        return super::body::TableStyleGeometry::default();
    }
    let Some(style) = raw_styles
        .get(id)
        .filter(|style| style.kind == Some(StyleKind::Table))
    else {
        return super::body::TableStyleGeometry::default();
    };
    stack.push(id.to_string());
    let mut geometry = style
        .based_on
        .as_deref()
        .map(|base| resolve_style_table_geometry(base, raw_styles, stack, depth + 1))
        .unwrap_or_default();
    geometry.overlay(style.table_geometry);
    geometry.overlay(style.whole_table_geometry);
    stack.pop();
    geometry
}

#[derive(Default)]
struct StyleTableProperties {
    margins: super::body::CellMarginSpec,
    borders: Option<super::body::TableBorderTuple>,
    geometry: super::body::TableStyleGeometry,
    row_band_size: Option<u8>,
    col_band_size: Option<u8>,
}

impl StyleTableProperties {
    fn overlay(&mut self, other: Self) {
        self.margins.overlay(other.margins);
        if other.borders.is_some() {
            self.borders = other.borders;
        }
        self.geometry.overlay(other.geometry);
        if other.row_band_size.is_some() {
            self.row_band_size = other.row_band_size;
        }
        if other.col_band_size.is_some() {
            self.col_band_size = other.col_band_size;
        }
    }

    fn record_leaf(&mut self, e: &BytesStart<'_>) {
        match local(e.name().as_ref()) {
            b"tblW" | b"tblInd" | b"jc" | b"tblLayout" | b"bidiVisual" => {
                self.geometry.record(e);
            }
            b"tblStyleRowBandSize" => {
                if let Some(size) = attr_u8(e, b"val").filter(|size| *size <= 3) {
                    self.row_band_size = Some(size);
                }
            }
            b"tblStyleColBandSize" => {
                if let Some(size) = attr_u8(e, b"val").filter(|size| *size <= 3) {
                    self.col_band_size = Some(size);
                }
            }
            _ => {}
        }
    }
}

fn read_style_table_properties(
    r: &mut Reader<&[u8]>,
    end: &[u8],
    depth: usize,
) -> StyleTableProperties {
    if depth >= STYLE_CHAIN_LIMIT {
        skip_subtree(r);
        return StyleTableProperties::default();
    }
    let mut values = StyleTableProperties::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                if let Some(value) = read_style_table_properties_alternate_content(r, depth + 1) {
                    values.overlay(value);
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblCellMar" => {
                values.margins.overlay(super::body::read_cell_margins(
                    r,
                    b"tblCellMar",
                    depth as u32 + 1,
                ));
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblBorders" => {
                values.borders = Some(super::body::read_tbl_borders(r));
            }
            Ok(Event::Start(e))
                if matches!(
                    local(e.name().as_ref()),
                    b"tblW"
                        | b"tblInd"
                        | b"jc"
                        | b"tblLayout"
                        | b"bidiVisual"
                        | b"tblStyleRowBandSize"
                        | b"tblStyleColBandSize"
                ) =>
            {
                values.record_leaf(&e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e))
                if matches!(
                    local(e.name().as_ref()),
                    b"tblW"
                        | b"tblInd"
                        | b"jc"
                        | b"tblLayout"
                        | b"bidiVisual"
                        | b"tblStyleRowBandSize"
                        | b"tblStyleColBandSize"
                ) =>
            {
                values.record_leaf(&e);
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == end => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    values
}

fn read_style_table_properties_alternate_content(
    r: &mut Reader<&[u8]>,
    depth: usize,
) -> Option<StyleTableProperties> {
    if depth >= STYLE_CHAIN_LIMIT {
        skip_subtree(r);
        return None;
    }
    let mut took = false;
    let mut values = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        values = Some(read_style_table_properties(r, name, depth + 1));
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") && !took =>
            {
                took = true;
                values = Some(StyleTableProperties::default());
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    values
}

fn read_conditional_cell_presentation(
    r: &mut Reader<&[u8]>,
    end: &[u8],
    depth: usize,
) -> TableStyleCellPresentation {
    if depth >= STYLE_CHAIN_LIMIT {
        skip_subtree(r);
        return TableStyleCellPresentation::default();
    }
    let mut presentation = TableStyleCellPresentation::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tcPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tcMar" => {
                presentation.margins.overlay(super::body::read_cell_margins(
                    r,
                    b"tcMar",
                    depth as u32 + 1,
                ));
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                if let Some(value) =
                    read_conditional_cell_presentation_alternate_content(r, depth + 1)
                {
                    presentation.overlay(value);
                }
            }
            Ok(Event::Start(e))
                if matches!(local(e.name().as_ref()), b"shd" | b"vAlign" | b"tcW") =>
            {
                presentation.defaults.record(&e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"shd" | b"vAlign" | b"tcW") =>
            {
                presentation.defaults.record(&e);
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == end => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    presentation
}

fn read_conditional_cell_presentation_alternate_content(
    r: &mut Reader<&[u8]>,
    depth: usize,
) -> Option<TableStyleCellPresentation> {
    if depth >= STYLE_CHAIN_LIMIT {
        skip_subtree(r);
        return None;
    }
    let mut took = false;
    let mut presentation = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        presentation = Some(read_conditional_cell_presentation(r, name, depth + 1));
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") && !took =>
            {
                took = true;
                presentation = Some(TableStyleCellPresentation::default());
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    presentation
}

#[derive(Default)]
struct ConditionalTableRegion {
    row: TableRowProps,
    presentation: TableStyleCellPresentation,
    borders: Option<super::body::TableBorderTuple>,
    geometry: super::body::TableStyleGeometry,
}

impl ConditionalTableRegion {
    fn overlay(&mut self, other: Self) {
        self.row.overlay(other.row);
        self.presentation.overlay(other.presentation);
        if other.borders.is_some() {
            self.borders = other.borders;
        }
        self.geometry.overlay(other.geometry);
    }
}

fn read_conditional_table_region(
    r: &mut Reader<&[u8]>,
    end: &[u8],
    depth: usize,
) -> ConditionalTableRegion {
    if depth >= STYLE_CHAIN_LIMIT {
        skip_subtree(r);
        return ConditionalTableRegion::default();
    }
    let mut values = ConditionalTableRegion::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"trPr" => {
                values.row.overlay(read_table_row_props(r, b"trPr"));
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblCellMar" => {
                values
                    .presentation
                    .margins
                    .overlay(super::body::read_cell_margins(
                        r,
                        b"tblCellMar",
                        depth as u32 + 1,
                    ));
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblBorders" => {
                values.borders = Some(super::body::read_tbl_borders(r));
            }
            Ok(Event::Start(e))
                if matches!(
                    local(e.name().as_ref()),
                    b"tblW" | b"tblInd" | b"jc" | b"tblLayout" | b"bidiVisual"
                ) =>
            {
                values.geometry.record(&e);
                skip_subtree(r);
            }
            Ok(Event::Empty(e))
                if matches!(
                    local(e.name().as_ref()),
                    b"tblW" | b"tblInd" | b"jc" | b"tblLayout" | b"bidiVisual"
                ) =>
            {
                values.geometry.record(&e);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tcPr" => {
                values
                    .presentation
                    .overlay(read_conditional_cell_presentation(r, b"tcPr", depth + 1));
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                if let Some(value) = read_conditional_table_region_alternate_content(r, depth + 1) {
                    values.overlay(value);
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"tblPr" => {
                values.overlay(read_conditional_table_region(r, b"tblPr", depth + 1));
            }
            Ok(Event::Start(_)) => skip_subtree(r),
            Ok(Event::End(e)) if local(e.name().as_ref()) == end => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    values
}

fn read_conditional_table_region_alternate_content(
    r: &mut Reader<&[u8]>,
    depth: usize,
) -> Option<ConditionalTableRegion> {
    if depth >= STYLE_CHAIN_LIMIT {
        skip_subtree(r);
        return None;
    }
    let mut took = false;
    let mut values = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        values = Some(read_conditional_table_region(r, name, depth + 1));
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e))
                if matches!(local(e.name().as_ref()), b"Choice" | b"Fallback") && !took =>
            {
                took = true;
                values = Some(ConditionalTableRegion::default());
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    values
}

fn read_table_row_props(r: &mut Reader<&[u8]>, end: &[u8]) -> TableRowProps {
    let mut props = TableRowProps::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"trPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                if let Some(value) = read_table_row_props_alternate_content(r) {
                    props.overlay(value);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"cantSplit" =>
            {
                props.cant_split = Some(toggle_on(attr_local(&e, b"val")));
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == end => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    props
}

fn read_table_row_props_alternate_content(r: &mut Reader<&[u8]>) -> Option<TableRowProps> {
    let mut took = false;
    let mut props = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        props = Some(read_table_row_props(r, name));
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    props
}

fn read_run_props(r: &mut Reader<&[u8]>, end: &[u8]) -> RunProps {
    let mut props = RunProps::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"rPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_run_props_alternate_content(r, &mut props);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => apply_run_props_child(&mut props, &e),
            Ok(Event::End(e)) if local(e.name().as_ref()) == end => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    props
}

fn read_run_props_alternate_content(r: &mut Reader<&[u8]>, props: &mut RunProps) {
    let mut took = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        read_run_props_alternate_content_branch(r, props, name);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn read_run_props_alternate_content_branch(
    r: &mut Reader<&[u8]>,
    props: &mut RunProps,
    branch: &[u8],
) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"rPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                read_run_props_alternate_content(r, props);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => apply_run_props_child(props, &e),
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_from_style_id_name_and_outline() {
        let xml = r#"<w:styles>
            <w:style w:type="paragraph" w:styleId="Heading1"><w:name w:val="heading 1"/></w:style>
            <w:style w:type="paragraph" w:styleId="KrTitle"><w:name w:val="제목 2"/></w:style>
            <w:style w:type="paragraph" w:styleId="CustomH"><w:name w:val="MyStyle"/>
                <w:pPr><w:outlineLvl w:val=" 2 "/></w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
        </w:styles>"#;
        let s = parse(xml);
        assert_eq!(s.heading_level("Heading1"), Some(1));
        assert_eq!(s.heading_level("KrTitle"), Some(2)); // 제목 2
        assert_eq!(s.heading_level("CustomH"), Some(3)); // outlineLvl 2 → h3
        assert_eq!(s.heading_level("Normal"), None);
        assert_eq!(s.name("KrTitle"), Some("제목 2"));
    }

    #[test]
    fn uses_single_alternate_content_branch() {
        let xml = r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
            <mc:AlternateContent>
                <mc:Choice Requires="w14">
                    <w:style w:type="paragraph" w:styleId="ChoiceHeading"><w:name w:val="heading 1"/></w:style>
                </mc:Choice>
                <mc:Fallback>
                    <w:style w:type="paragraph" w:styleId="FallbackHeading"><w:name w:val="heading 1"/></w:style>
                </mc:Fallback>
            </mc:AlternateContent>
        </w:styles>"#;
        let s = parse(xml);

        assert_eq!(s.heading_level("ChoiceHeading"), Some(1));
        assert_eq!(s.name("ChoiceHeading"), Some("heading 1"));
        assert_eq!(s.heading_level("FallbackHeading"), None);
        assert_eq!(s.name("FallbackHeading"), None);
    }

    #[test]
    fn resolves_inherited_paragraph_direction_alignment_and_logical_indents() {
        let xml = r#"<w:styles>
            <w:docDefaults><w:pPrDefault><w:pPr><w:jc w:val="left"/></w:pPr></w:pPrDefault></w:docDefaults>
            <w:style w:type="paragraph" w:styleId="RtlBase">
                <w:pPr><w:bidi/><w:jc w:val="start"/><w:ind w:start="720" w:end="1440"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="RtlDerived"><w:basedOn w:val="RtlBase"/></w:style>
            <w:style w:type="paragraph" w:styleId="LtrOverride"><w:basedOn w:val="RtlBase"/>
                <w:pPr><w:bidi w:val="0"/><w:jc w:val="end"/></w:pPr>
            </w:style>
        </w:styles>"#;
        let styles = parse(xml);
        let defaults = styles.paragraph_props(None);
        let inherited = styles.paragraph_props(Some("RtlDerived"));
        let overridden = styles.paragraph_props(Some("LtrOverride"));

        assert_eq!(defaults.jc.as_deref(), Some("left"));
        assert_eq!(inherited.bidi, Some(true));
        assert_eq!(inherited.jc.as_deref(), Some("start"));
        assert_eq!(inherited.indent_start_pt, Some(36.0));
        assert_eq!(inherited.indent_end_pt, Some(72.0));
        assert_eq!(overridden.bidi, Some(false));
        assert_eq!(overridden.jc.as_deref(), Some("end"));
    }

    #[test]
    fn resolves_inherited_pagination_controls_and_explicit_off_values() {
        let xml = r#"<w:styles>
            <w:docDefaults><w:pPrDefault><w:pPr><w:widowControl/></w:pPr></w:pPrDefault></w:docDefaults>
            <w:style w:type="paragraph" w:styleId="KeepBase">
                <w:pPr><w:keepNext/><w:keepLines/><w:widowControl w:val="0"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="KeepDerived"><w:basedOn w:val="KeepBase"/>
                <w:pPr><w:keepLines w:val="off"/><w:widowControl/></w:pPr>
            </w:style>
        </w:styles>"#;
        let styles = parse(xml);
        let defaults = styles.paragraph_props(None);
        let base = styles.paragraph_props(Some("KeepBase"));
        let derived = styles.paragraph_props(Some("KeepDerived"));

        assert_eq!(defaults.widow_control, Some(true));
        assert_eq!(base.keep_next, Some(true));
        assert_eq!(base.keep_lines, Some(true));
        assert_eq!(base.widow_control, Some(false));
        assert_eq!(derived.keep_next, Some(true));
        assert_eq!(derived.keep_lines, Some(false));
        assert_eq!(derived.widow_control, Some(true));
    }

    #[test]
    fn default_paragraph_style_applies_only_without_an_explicit_style() {
        let xml = r#"<w:styles>
            <w:docDefaults>
                <w:rPrDefault><w:rPr><w:i/></w:rPr></w:rPrDefault>
                <w:pPrDefault><w:pPr><w:spacing w:before="60"/></w:pPr></w:pPrDefault>
            </w:docDefaults>
            <w:style w:type="paragraph" w:default="1" w:styleId="OldDefault">
                <w:pPr><w:spacing w:before="120"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:default="off" w:styleId="DisabledDefault">
                <w:pPr><w:spacing w:before="180"/></w:pPr>
            </w:style>
            <w:style w:default="true" w:styleId="CurrentDefault">
                <w:pPr><w:spacing w:before="240"/><w:pageBreakBefore/></w:pPr>
                <w:rPr><w:b/></w:rPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="Explicit">
                <w:pPr><w:spacing w:before="360"/></w:pPr>
            </w:style>
        </w:styles>"#;
        let styles = parse(xml);

        let implicit = styles.paragraph_props(None);
        assert_eq!(implicit.layout.spacing().before_pt, Some(12.0));
        assert!(implicit.layout.page_break_before());

        let explicit = styles.paragraph_props(Some("Explicit"));
        assert_eq!(explicit.layout.spacing().before_pt, Some(18.0));
        assert!(!explicit.layout.page_break_before());

        let missing = styles.paragraph_props(Some("Missing"));
        assert_eq!(missing.layout.spacing().before_pt, Some(3.0));
        assert!(!missing.layout.page_break_before());

        let implicit_run = styles.resolved_run_props(None, None);
        assert!(implicit_run.bold.expect("default paragraph run property"));
        assert!(implicit_run.italic.expect("document run default"));
        let explicit_run = styles.resolved_run_props(Some("Explicit"), None);
        assert_eq!(explicit_run.bold, None);
        assert!(explicit_run.italic.expect("document run default"));
    }

    #[test]
    fn resolves_paragraph_layout_only_from_paragraph_property_scope() {
        let xml = r#"<w:styles xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
            <w:docDefaults>
                <w:rPrDefault><w:rPr>
                    <w:spacing w:before="120"/><w:ind w:firstLine="120"/>
                    <w:shd w:val="clear" w:fill="112233"/><w:pageBreakBefore/>
                </w:rPr></w:rPrDefault>
                <w:pPrDefault><w:pPr><w:rPr>
                    <w:spacing w:before="180"/><w:ind w:firstLine="180"/>
                    <w:shd w:val="clear" w:fill="182838"/><w:pageBreakBefore/>
                </w:rPr></w:pPr></w:pPrDefault>
            </w:docDefaults>
            <w:style w:type="character" w:styleId="Character">
                <w:pPr><w:spacing w:before="240"/><w:ind w:firstLine="240"/>
                    <w:shd w:val="clear" w:fill="223344"/><w:pageBreakBefore/>
                </w:pPr>
            </w:style>
            <w:style w:type="table" w:styleId="Table">
                <w:pPr><w:spacing w:before="360"/><w:ind w:firstLine="360"/>
                    <w:shd w:val="clear" w:fill="334455"/><w:pageBreakBefore/>
                </w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="Paragraph">
                <w:rPr><w:spacing w:before="480"/><w:ind w:firstLine="480"/>
                    <w:shd w:val="clear" w:fill="445566"/><w:pageBreakBefore/>
                </w:rPr>
                <w:pPr>
                    <w:tcPr><w:spacing w:before="540"/><w:ind w:firstLine="540"/>
                        <w:shd w:val="clear" w:fill="556677"/><w:pageBreakBefore/>
                    </w:tcPr>
                    <w:numPr><w:spacing w:before="600"/><w:ind w:firstLine="600"/>
                        <w:shd w:val="clear" w:fill="667788"/><w:pageBreakBefore/>
                    </w:numPr>
                    <w:tabs>
                        <mc:AlternateContent>
                            <mc:Choice Requires="w14">
                                <w:tab w:val="right" w:pos="720"/>
                                <w:spacing w:before="720"/><w:ind w:firstLine="720"/>
                                <w:shd w:val="clear" w:fill="8899AA"/><w:pageBreakBefore/>
                            </mc:Choice>
                            <mc:Fallback><w:tab w:val="left" w:pos="1440"/></mc:Fallback>
                        </mc:AlternateContent>
                    </w:tabs>
                </w:pPr>
            </w:style>
            <w:style w:type="table" w:styleId="ConditionalTable">
                <w:tblStylePr w:type="firstRow"><w:pPr>
                    <w:spacing w:before="660"/><w:ind w:firstLine="660"/>
                    <w:shd w:val="clear" w:fill="778899"/><w:pageBreakBefore/>
                </w:pPr></w:tblStylePr>
            </w:style>
        </w:styles>"#;
        let styles = parse(xml);

        for props in [
            styles.paragraph_props(None),
            styles.paragraph_props(Some("Character")),
            styles.paragraph_props(Some("Table")),
            styles.paragraph_props(Some("Paragraph")),
            styles.paragraph_props(Some("ConditionalTable")),
        ] {
            let mut indent = Indent::default();
            props.layout.apply_indent(&mut indent);
            assert_eq!(props.layout.spacing(), Spacing::default());
            assert_eq!(indent.first_line_pt, None);
            assert_eq!(indent.hanging_pt, None);
            assert_eq!(props.layout.shading(), None);
            assert!(!props.layout.page_break_before());
        }
        assert_eq!(
            styles.paragraph_props(Some("Paragraph")).tab_stops,
            vec![TabStop {
                position_pt: 36.0,
                alignment: TabAlignment::Right,
                leader: TabLeader::None,
            }]
        );
    }

    #[test]
    fn paragraph_layout_suppresses_unrepresented_nearer_values() {
        let xml = r#"<w:styles>
            <w:style w:type="paragraph" w:styleId="Base"><w:pPr>
                <w:spacing w:before="120" w:after="240" w:line="360"/>
                <w:ind w:firstLine="200"/>
                <w:shd w:val="clear" w:fill="112233"/>
                <w:pageBreakBefore/>
            </w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="Automatic"><w:basedOn w:val="Base"/><w:pPr>
                <w:spacing w:beforeAutospacing="1" w:afterAutospacing="true"
                           w:line="360" w:lineRule="atLeast"/>
                <w:ind w:hangingChars="100"/>
                <w:shd w:val="nil" w:fill="445566"/>
                <w:pageBreakBefore w:val="false"/>
            </w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="Malformed"><w:basedOn w:val="Base"/><w:pPr>
                <w:spacing w:before="-1" w:after="bad" w:line="NaN"/>
                <w:ind w:firstLine="-20"/>
                <w:shd w:val="clear" w:fill="auto"/>
            </w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="Both"><w:basedOn w:val="Base"/><w:pPr>
                <w:ind w:firstLine="320" w:hanging="440"/>
            </w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="InvalidToggle"><w:basedOn w:val="Base"/><w:pPr>
                <w:pageBreakBefore w:val="TRUE"/>
            </w:pPr></w:style>
        </w:styles>"#;
        let styles = parse(xml);

        for id in ["Automatic", "Malformed"] {
            let props = styles.paragraph_props(Some(id));
            let mut indent = Indent::default();
            props.layout.apply_indent(&mut indent);
            assert_eq!(props.layout.spacing(), Spacing::default(), "{id}");
            assert_eq!(indent.first_line_pt, None, "{id}");
            assert_eq!(indent.hanging_pt, None, "{id}");
            assert_eq!(props.layout.shading(), None, "{id}");
        }
        assert!(!styles
            .paragraph_props(Some("Automatic"))
            .layout
            .page_break_before());
        assert!(styles
            .paragraph_props(Some("Malformed"))
            .layout
            .page_break_before());

        let both = styles.paragraph_props(Some("Both"));
        let mut indent = Indent::default();
        both.layout.apply_indent(&mut indent);
        assert_eq!(indent.first_line_pt, None);
        assert_eq!(indent.hanging_pt, Some(22.0));
        assert!(!styles
            .paragraph_props(Some("InvalidToggle"))
            .layout
            .page_break_before());
    }

    #[test]
    fn tab_stops_preserve_supported_leaders_and_bar_alignment() {
        let styles = parse(
            r#"<w:styles><w:style w:type="paragraph" w:styleId="Tabs"><w:pPr><w:tabs>
                <w:tab w:val="right" w:pos="1440" w:leader="dot"/>
                <w:tab w:val="bar" w:pos="2160"/>
                <w:tab w:val="decimal" w:pos="2880" w:leader="middleDot"/>
            </w:tabs></w:pPr></w:style></w:styles>"#,
        );

        assert_eq!(
            styles.paragraph_props(Some("Tabs")).tab_stops,
            vec![
                TabStop {
                    position_pt: 72.0,
                    alignment: TabAlignment::Right,
                    leader: TabLeader::Dot,
                },
                TabStop {
                    position_pt: 108.0,
                    alignment: TabAlignment::Bar,
                    leader: TabLeader::None,
                },
                TabStop {
                    position_pt: 144.0,
                    alignment: TabAlignment::Decimal,
                    leader: TabLeader::MiddleDot,
                },
            ]
        );
    }

    #[test]
    fn paragraph_layout_cascades_dependent_attributes_before_evaluation() {
        let xml = r#"<w:styles>
            <w:style w:type="paragraph" w:styleId="Automatic"><w:pPr>
                <w:spacing w:before="120" w:after="240"
                    w:beforeAutospacing="1" w:afterAutospacing="true"/>
            </w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="AutomaticManual"><w:basedOn w:val="Automatic"/>
                <w:pPr><w:spacing w:before="480" w:after="600"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="AutomaticOff"><w:basedOn w:val="Automatic"/>
                <w:pPr><w:spacing w:beforeAutospacing="0" w:afterAutospacing="off"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="Exact"><w:pPr>
                <w:spacing w:line="360" w:lineRule="exact"/>
            </w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="ExactLine"><w:basedOn w:val="Exact"/>
                <w:pPr><w:spacing w:line="480"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="ExactAuto"><w:basedOn w:val="Exact"/>
                <w:pPr><w:spacing w:lineRule="auto"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="CharacterIndent"><w:pPr>
                <w:ind w:hangingChars="100"/>
            </w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="CharacterIndentTwips">
                <w:basedOn w:val="CharacterIndent"/>
                <w:pPr><w:ind w:firstLine="240"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="LineUnits"><w:pPr>
                <w:spacing w:before="120" w:after="240"
                    w:beforeLines="100" w:afterLines="100"/>
            </w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="LineUnitsZero">
                <w:basedOn w:val="LineUnits"/>
                <w:pPr><w:spacing w:beforeLines="-0" w:afterLines="+0"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="StandaloneLineUnitsZero"><w:pPr>
                <w:spacing w:beforeLines="0" w:afterLines="0"/>
            </w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="FirstLineChars"><w:pPr>
                <w:ind w:firstLineChars="100"/>
            </w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="FirstLineCharsZero">
                <w:basedOn w:val="FirstLineChars"/>
                <w:pPr><w:ind w:firstLineChars="-0" w:firstLine="240"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="HangingChars"><w:pPr>
                <w:ind w:hangingChars="100"/>
            </w:pPr></w:style>
            <w:style w:type="paragraph" w:styleId="HangingCharsZero">
                <w:basedOn w:val="HangingChars"/>
                <w:pPr><w:ind w:hangingChars="+0" w:hanging="240"/></w:pPr>
            </w:style>
        </w:styles>"#;
        let styles = parse(xml);

        assert_eq!(
            styles
                .paragraph_props(Some("AutomaticManual"))
                .layout
                .spacing(),
            Spacing::default()
        );
        assert_eq!(
            styles
                .paragraph_props(Some("AutomaticOff"))
                .layout
                .spacing(),
            Spacing {
                before_pt: Some(6.0),
                after_pt: Some(12.0),
                line_pct: None,
            }
        );
        assert_eq!(
            styles
                .paragraph_props(Some("ExactLine"))
                .layout
                .spacing()
                .line_pct,
            Some(2.0)
        );
        assert_eq!(
            styles
                .paragraph_props(Some("ExactAuto"))
                .layout
                .spacing()
                .line_pct,
            Some(1.5)
        );
        assert_eq!(
            styles
                .paragraph_props(Some("LineUnitsZero"))
                .layout
                .spacing(),
            Spacing {
                before_pt: Some(6.0),
                after_pt: Some(12.0),
                line_pct: None,
            }
        );
        assert_eq!(
            styles
                .paragraph_props(Some("StandaloneLineUnitsZero"))
                .layout
                .spacing(),
            Spacing {
                before_pt: Some(0.0),
                after_pt: Some(0.0),
                line_pct: None,
            }
        );

        let mut indent = Indent::default();
        styles
            .paragraph_props(Some("CharacterIndentTwips"))
            .layout
            .apply_indent(&mut indent);
        assert_eq!(indent.first_line_pt, None);
        assert_eq!(indent.hanging_pt, None);

        let mut indent = Indent::default();
        styles
            .paragraph_props(Some("FirstLineCharsZero"))
            .layout
            .apply_indent(&mut indent);
        assert_eq!(indent.first_line_pt, Some(12.0));
        assert_eq!(indent.hanging_pt, None);

        let mut indent = Indent::default();
        styles
            .paragraph_props(Some("HangingCharsZero"))
            .layout
            .apply_indent(&mut indent);
        assert_eq!(indent.first_line_pt, None);
        assert_eq!(indent.hanging_pt, Some(12.0));
    }

    #[test]
    fn paragraph_layout_on_off_lexical_forms_are_exact() {
        for (value, expected) in [
            ("1", true),
            ("true", true),
            ("on", true),
            ("0", false),
            ("false", false),
            ("off", false),
        ] {
            assert_eq!(strict_on_off(value), Some(expected), "{value}");
        }
        for value in ["TRUE", "False", "ON", " true", "true ", "\t0"] {
            assert_eq!(strict_on_off(value), None, "{value:?}");
        }
    }

    #[test]
    fn paragraph_layout_rejects_non_schema_numeric_forms() {
        let xml = r#"<w:styles>
            <w:style w:type="paragraph" w:styleId="Extreme"><w:pPr>
                <w:spacing w:before="INF" w:after="3.4028235e38" w:line="1e-9999"/>
                <w:ind w:firstLine="1e999" w:hanging="-INF"/>
            </w:pPr></w:style>
        </w:styles>"#;
        let props = parse(xml).paragraph_props(Some("Extreme"));
        let mut indent = Indent::default();
        props.layout.apply_indent(&mut indent);

        assert_eq!(props.layout.spacing(), Spacing::default());
        assert_eq!(indent.first_line_pt, None);
        assert_eq!(indent.hanging_pt, None);
    }

    #[test]
    fn paragraph_absolute_line_spacing_resolves_twips_after_style_cascade() {
        let styles = parse(
            r#"<w:styles>
                <w:style w:type="paragraph" w:styleId="Exact"><w:pPr>
                    <w:spacing w:line="360" w:lineRule="exact"/>
                </w:pPr></w:style>
                <w:style w:type="paragraph" w:styleId="InheritedAtLeast">
                    <w:basedOn w:val="Exact"/><w:pPr>
                        <w:spacing w:lineRule="atLeast"/>
                    </w:pPr>
                </w:style>
                <w:style w:type="paragraph" w:styleId="DirectLineDefaultsAuto">
                    <w:basedOn w:val="Exact"/><w:pPr>
                        <w:spacing w:line="480"/>
                    </w:pPr>
                </w:style>
                <w:style w:type="paragraph" w:styleId="Malformed"><w:pPr>
                    <w:spacing w:line="-20" w:lineRule="exact"/>
                </w:pPr></w:style>
            </w:styles>"#,
        );

        assert_eq!(
            styles
                .paragraph_props(Some("Exact"))
                .layout
                .line_spacing_hint(),
            Some(crate::model::LineSpacingHint::Exact(18.0))
        );
        assert_eq!(
            styles
                .paragraph_props(Some("InheritedAtLeast"))
                .layout
                .line_spacing_hint(),
            Some(crate::model::LineSpacingHint::AtLeast(18.0))
        );
        assert_eq!(
            styles
                .paragraph_props(Some("DirectLineDefaultsAuto"))
                .layout
                .line_spacing_hint(),
            None
        );
        assert_eq!(
            styles
                .paragraph_props(Some("Malformed"))
                .layout
                .line_spacing_hint(),
            None
        );
    }

    #[test]
    fn paragraph_layout_cycles_are_root_local_and_deterministic() {
        let xml = r#"<w:styles>
            <w:style w:type="paragraph" w:styleId="TwoA"><w:basedOn w:val="TwoB"/>
                <w:pPr><w:spacing w:before="120"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="TwoB"><w:basedOn w:val="TwoA"/>
                <w:pPr><w:spacing w:before="240"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="ThreeA"><w:basedOn w:val="ThreeB"/>
                <w:pPr><w:ind w:firstLine="180"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="ThreeB"><w:basedOn w:val="ThreeC"/>
                <w:pPr><w:ind w:firstLine="300"/></w:pPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="ThreeC"><w:basedOn w:val="ThreeA"/>
                <w:pPr><w:ind w:firstLine="420"/></w:pPr>
            </w:style>
        </w:styles>"#;

        for _ in 0..16 {
            let styles = parse(xml);
            assert_eq!(
                styles
                    .paragraph_props(Some("TwoA"))
                    .layout
                    .spacing()
                    .before_pt,
                Some(6.0)
            );
            assert_eq!(
                styles
                    .paragraph_props(Some("TwoB"))
                    .layout
                    .spacing()
                    .before_pt,
                Some(12.0)
            );
            for (id, expected) in [("ThreeA", 9.0), ("ThreeB", 15.0), ("ThreeC", 21.0)] {
                let mut indent = Indent::default();
                styles
                    .paragraph_props(Some(id))
                    .layout
                    .apply_indent(&mut indent);
                assert_eq!(indent.first_line_pt, Some(expected), "{id}");
            }
        }
    }

    #[test]
    fn utf8_styles_xml_declarations_parse() {
        let document = r#"<w:styles>
            <w:style w:type="paragraph" w:styleId="Body">
                <w:name w:val="Body"/>
            </w:style>
        </w:styles>"#;

        for declaration in [
            r#"<?xml version="1.0"?>"#,
            r#"<?xml version="1.0" encoding="UTF-8"?>"#,
            r#"<?xml version="1.0" encoding="utf-8" standalone="yes"?>"#,
        ] {
            assert_eq!(
                parse(&format!("{declaration}{document}")).name("Body"),
                Some("Body"),
                "{declaration}"
            );
        }
    }

    #[test]
    fn malformed_styles_xml_fails_empty() {
        let prefix = r#"<w:styles>
            <w:docDefaults><w:pPrDefault><w:pPr>
                <w:spacing w:before="120"/><w:pageBreakBefore/>
            </w:pPr></w:pPrDefault></w:docDefaults>
            <w:style w:type="paragraph" w:styleId="Completed"><w:pPr>
                <w:ind w:firstLine="240"/><w:shd w:val="clear" w:fill="112233"/>
            </w:pPr></w:style>"#;
        let assert_empty = |xml: &str, label: &str| {
            let styles = parse(xml);
            for props in [
                styles.paragraph_props(None),
                styles.paragraph_props(Some("Completed")),
            ] {
                let mut indent = Indent::default();
                props.layout.apply_indent(&mut indent);
                assert_eq!(props.layout.spacing(), Spacing::default(), "{label}");
                assert_eq!(indent, Indent::default(), "{label}");
                assert_eq!(props.layout.shading(), None, "{label}");
                assert!(!props.layout.page_break_before(), "{label}");
            }
            assert_eq!(styles.name("Completed"), None, "{label}");
        };
        for suffix in [
            "<w:broken></w:styles>",
            "<w:broken w:value=bad/></w:styles>",
            r#"<w:broken w:value="1" w:value="2"/></w:styles>"#,
            "<w:broken>&bogus;</w:broken></w:styles>",
            "<w:broken>&#1;</w:broken></w:styles>",
            r#"<w:broken w:value="&#1;"/></w:styles>"#,
            "</w:styles><w:styles/>",
            "</w:styles>not-whitespace",
        ] {
            let xml = format!("{prefix}{suffix}");
            assert_empty(&xml, suffix);
        }

        let complete = format!("{prefix}</w:styles>");
        for declaration in [
            r#"<!DOCTYPE w:styles><?xml version="1.0"?>"#,
            r#"<?xml version="1.0" version="1.1"?>"#,
            r#"<?xml version="1.2"?>"#,
            r#"<?xml version="1&#46;0"?>"#,
            r#"<?xml version="1.0" encoding="UT&#70;-8"?>"#,
            r#"<?xml version="1.0" encoding="UTF-16"?>"#,
            r#"<?xml version="1.0" standalone="maybe"?>"#,
            r#"<?xml version="1.0" standalone="y&#101;s"?>"#,
            r#" <?xml version="1.0"?>"#,
        ] {
            let xml = format!("{declaration}{complete}");
            assert_empty(&xml, declaration);
        }
    }

    #[test]
    fn paragraph_layout_ignores_historical_property_changes() {
        let xml = r#"<w:styles>
            <w:style w:type="paragraph" w:styleId="Current"><w:pPr>
                <w:spacing w:before="120"/>
                <w:pPrChange><w:pPr>
                    <w:spacing w:before="480"/>
                    <w:ind w:hanging="600"/>
                    <w:shd w:val="clear" w:fill="AABBCC"/>
                    <w:pageBreakBefore/>
                </w:pPr></w:pPrChange>
                <w:ind w:firstLine="360"/>
                <w:shd w:val="clear" w:fill="112233"/>
                <w:pageBreakBefore w:val="0"/>
            </w:pPr></w:style>
        </w:styles>"#;
        let props = parse(xml).paragraph_props(Some("Current"));
        let mut indent = Indent::default();
        props.layout.apply_indent(&mut indent);

        assert_eq!(props.layout.spacing().before_pt, Some(6.0));
        assert_eq!(props.layout.spacing().after_pt, None);
        assert_eq!(indent.first_line_pt, Some(18.0));
        assert_eq!(indent.hanging_pt, None);
        assert_eq!(props.layout.shading(), Some(Color::rgb(0x11, 0x22, 0x33)));
        assert!(!props.layout.page_break_before());
    }

    #[test]
    fn paragraph_layout_uses_one_alternate_content_branch_and_chain_limit() {
        let mut xml = String::from(
            r#"<w:styles xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
                <w:style w:type="paragraph" w:styleId="Choice"><w:pPr>
                    <mc:AlternateContent>
                        <mc:Choice Requires="w14">
                            <w:spacing w:before="120"/><w:ind w:firstLine="240"/>
                            <w:shd w:val="clear" w:fill="112233"/><w:pageBreakBefore/>
                        </mc:Choice>
                        <mc:Fallback>
                            <w:spacing w:before="360"/><w:ind w:hanging="480"/>
                            <w:shd w:val="clear" w:fill="AABBCC"/>
                            <w:pageBreakBefore w:val="0"/>
                        </mc:Fallback>
                    </mc:AlternateContent>
                </w:pPr></w:style>"#,
        );
        for index in 0..=STYLE_CHAIN_LIMIT {
            let base = if index == STYLE_CHAIN_LIMIT {
                String::new()
            } else {
                format!(r#"<w:basedOn w:val="Depth{}"/>"#, index + 1)
            };
            let layout = if index == STYLE_CHAIN_LIMIT {
                r#"<w:pPr><w:spacing w:after="240"/></w:pPr>"#
            } else {
                ""
            };
            xml.push_str(&format!(
                r#"<w:style w:type="paragraph" w:styleId="Depth{index}">{base}{layout}</w:style>"#
            ));
        }
        xml.push_str("</w:styles>");

        let styles = parse(&xml);
        let choice = styles.paragraph_props(Some("Choice"));
        let mut indent = Indent::default();
        choice.layout.apply_indent(&mut indent);
        assert_eq!(choice.layout.spacing().before_pt, Some(6.0));
        assert_eq!(indent.first_line_pt, Some(12.0));
        assert_eq!(indent.hanging_pt, None);
        assert_eq!(choice.layout.shading(), Some(Color::rgb(0x11, 0x22, 0x33)));
        assert!(choice.layout.page_break_before());

        assert_eq!(
            styles
                .paragraph_props(Some("Depth0"))
                .layout
                .spacing()
                .after_pt,
            None
        );
        assert_eq!(
            styles
                .paragraph_props(Some("Depth1"))
                .layout
                .spacing()
                .after_pt,
            Some(12.0)
        );
    }

    #[test]
    fn resolves_nonconditional_table_row_pagination_through_table_style_chains() {
        let xml = r#"<w:styles xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
            <w:style w:type="table" w:styleId="KeepBase">
                <w:trPr><w:cantSplit/></w:trPr>
            </w:style>
            <w:style w:type="table" w:styleId="KeepDerived">
                <w:basedOn w:val="KeepBase"/>
            </w:style>
            <w:style w:type="table" w:styleId="AllowDerived">
                <w:basedOn w:val="KeepBase"/>
                <w:trPr><w:cantSplit w:val="off"/></w:trPr>
            </w:style>
            <w:style w:type="table" w:styleId="ChoiceOff">
                <w:trPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:cantSplit w:val="0"/></mc:Choice>
                    <mc:Fallback><w:cantSplit/></mc:Fallback>
                </mc:AlternateContent></w:trPr>
            </w:style>
            <w:style w:type="table" w:styleId="ConditionalOnly">
                <w:tblStylePr w:type="firstRow"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
            </w:style>
            <w:style w:type="table" w:styleId="HistoricalOnly">
                <w:trPr><w:trPrChange><w:trPr><w:cantSplit/></w:trPr></w:trPrChange></w:trPr>
            </w:style>
            <w:style w:type="table" w:styleId="CycleA"><w:basedOn w:val="CycleB"/></w:style>
            <w:style w:type="table" w:styleId="CycleB"><w:basedOn w:val="CycleA"/></w:style>
            <w:style w:type="paragraph" w:styleId="WrongKind">
                <w:trPr><w:cantSplit/></w:trPr>
            </w:style>
            <w:style w:type="table" w:styleId="CrossKind">
                <w:basedOn w:val="WrongKind"/>
            </w:style>
        </w:styles>"#;
        let styles = parse(xml);

        assert_eq!(styles.table_row_cant_split(Some("KeepBase")), Some(true));
        assert_eq!(styles.table_row_cant_split(Some("KeepDerived")), Some(true));
        assert_eq!(
            styles.table_row_cant_split(Some("AllowDerived")),
            Some(false)
        );
        assert_eq!(styles.table_row_cant_split(Some("ChoiceOff")), Some(false));
        assert_eq!(styles.table_row_cant_split(Some("ConditionalOnly")), None);
        assert_eq!(styles.table_row_cant_split(Some("HistoricalOnly")), None);
        assert_eq!(styles.table_row_cant_split(Some("CycleA")), None);
        assert_eq!(styles.table_row_cant_split(Some("CrossKind")), None);
        assert_eq!(styles.table_row_cant_split(Some("missing")), None);
        assert_eq!(styles.table_row_cant_split(None), None);
    }

    #[test]
    fn table_row_style_resolution_stops_at_the_chain_limit() {
        let mut xml = String::from("<w:styles>");
        for index in 0..=STYLE_CHAIN_LIMIT {
            xml.push_str(&format!(
                r#"<w:style w:type="table" w:styleId="S{index}"><w:basedOn w:val="S{}"/></w:style>"#,
                index + 1
            ));
        }
        xml.push_str(&format!(
            r#"<w:style w:type="table" w:styleId="S{}"><w:trPr><w:cantSplit/></w:trPr><w:tblStylePr w:type="firstRow"><w:trPr><w:cantSplit/></w:trPr><w:tcPr><w:shd w:fill="123456"/></w:tcPr></w:tblStylePr></w:style></w:styles>"#,
            STYLE_CHAIN_LIMIT + 1
        ));

        let styles = parse(&xml);

        assert_eq!(styles.table_row_cant_split(Some("S0")), None);
        assert_eq!(
            styles.table_row_cant_split(Some(&format!("S{}", STYLE_CHAIN_LIMIT))),
            Some(true)
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(
                Some("S0"),
                TableRowStyleRegions {
                    first_row: true,
                    ..Default::default()
                },
            ),
            None
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(
                Some(&format!("S{}", STYLE_CHAIN_LIMIT)),
                TableRowStyleRegions {
                    first_row: true,
                    ..Default::default()
                },
            ),
            Some(true)
        );
        let first = TableRowStyleRegions {
            first_row: true,
            ..Default::default()
        };
        assert_eq!(
            styles
                .table_cell_presentation_for_regions(Some("S0"), first)
                .defaults
                .shading(),
            None
        );
        assert_eq!(
            styles
                .table_cell_presentation_for_regions(
                    Some(&format!("S{}", STYLE_CHAIN_LIMIT)),
                    first,
                )
                .defaults
                .shading(),
            Some(Color::rgb(0x12, 0x34, 0x56))
        );
    }

    #[test]
    fn resolves_bounded_conditional_table_row_pagination_with_region_precedence() {
        let xml = r#"<w:styles xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
            <w:style w:type="table" w:styleId="ConditionalBase">
                <w:trPr><w:cantSplit/></w:trPr>
                <w:tblStylePr w:type="wholeTable">
                    <w:trPr><w:cantSplit w:val="off"/></w:trPr>
                </w:tblStylePr>
                <w:tblStylePr w:type="firstRow">
                    <w:trPr><w:cantSplit/></w:trPr>
                </w:tblStylePr>
                <w:tblStylePr w:type="lastRow">
                    <w:trPr><w:cantSplit w:val="off"/></w:trPr>
                </w:tblStylePr>
            </w:style>
            <w:style w:type="table" w:styleId="ConditionalDerived">
                <w:basedOn w:val="ConditionalBase"/>
                <w:tblStylePr w:type="lastRow">
                    <w:trPr><w:cantSplit/></w:trPr>
                </w:tblStylePr>
            </w:style>
            <w:style w:type="table" w:styleId="ChoiceOff">
                <w:tblStylePr w:type="firstRow"><mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:trPr><w:cantSplit w:val="0"/></w:trPr>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:trPr><w:cantSplit/></w:trPr>
                    </mc:Fallback>
                </mc:AlternateContent></w:tblStylePr>
            </w:style>
            <w:style w:type="table" w:styleId="DeferredBand">
                <w:tblStylePr w:type="band1Horz">
                    <w:trPr><w:cantSplit/></w:trPr>
                </w:tblStylePr>
            </w:style>
            <w:style w:type="table" w:styleId="HistoricalConditional">
                <w:tblStylePr w:type="firstRow"><w:trPr>
                    <w:trPrChange><w:trPr><w:cantSplit/></w:trPr></w:trPrChange>
                </w:trPr></w:tblStylePr>
            </w:style>
        </w:styles>"#;
        let styles = parse(xml);
        let neither = TableRowStyleRegions::default();
        let first = TableRowStyleRegions {
            first_row: true,
            ..Default::default()
        };
        let last = TableRowStyleRegions {
            last_row: true,
            ..Default::default()
        };
        let both = TableRowStyleRegions {
            first_row: true,
            last_row: true,
            ..Default::default()
        };
        let band1 = TableRowStyleRegions {
            band1_horizontal: true,
            ..Default::default()
        };

        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("ConditionalBase"), neither),
            Some(false)
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("ConditionalBase"), first),
            Some(true)
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("ConditionalBase"), last),
            Some(false)
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("ConditionalBase"), both),
            Some(false)
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("ConditionalDerived"), last),
            Some(true)
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("ChoiceOff"), first),
            Some(false)
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("DeferredBand"), band1),
            Some(true)
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("HistoricalConditional"), first),
            None
        );
    }

    #[test]
    fn conditional_cell_presentation_recovers_after_mce_depth_limit() {
        let mut nested = String::new();
        for _ in 0..STYLE_CHAIN_LIMIT + 2 {
            nested.push_str("<mc:AlternateContent><mc:Choice Requires=\"w\">");
        }
        nested.push_str("<w:tcPr><w:shd w:fill=\"FFFFFF\"/></w:tcPr>");
        for _ in 0..STYLE_CHAIN_LIMIT + 2 {
            nested.push_str(
                "</mc:Choice><mc:Fallback><w:tcPr><w:shd w:fill=\"EEEEEE\"/></w:tcPr></mc:Fallback></mc:AlternateContent>",
            );
        }
        let xml = format!(
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
                <w:style w:type="table" w:styleId="Bounded">
                    <w:tblStylePr w:type="firstRow">{nested}
                        <w:tcPr>
                            <w:shd w:fill="123456"/>
                            <w:vAlign w:val="bottom"/>
                            <w:tcW w:w="2500" w:type="pct"/>
                        </w:tcPr>
                    </w:tblStylePr>
                </w:style>
            </w:styles>"#
        );
        let styles = parse(&xml);
        let presentation = styles.table_cell_presentation_for_regions(
            Some("Bounded"),
            TableRowStyleRegions {
                first_row: true,
                ..Default::default()
            },
        );

        assert_eq!(
            presentation.defaults.shading(),
            Some(Color::rgb(0x12, 0x34, 0x56))
        );
        assert_eq!(
            presentation.defaults.valign(),
            Some(crate::model::VCell::Bottom)
        );
        assert_eq!(presentation.defaults.width_pct(), Some(0.5));
    }

    #[test]
    fn conditional_cell_region_precedence_applies_after_based_on_resolution() {
        let styles = parse(
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="Base">
                    <w:tblStylePr w:type="firstRow"><w:tcPr>
                        <w:shd w:fill="112233"/>
                    </w:tcPr></w:tblStylePr>
                </w:style>
                <w:style w:type="table" w:styleId="Derived">
                    <w:basedOn w:val="Base"/>
                    <w:tblStylePr w:type="wholeTable"><w:tcPr>
                        <w:shd w:fill="AABBCC"/>
                    </w:tcPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );

        let first = styles.table_cell_presentation_for_regions(
            Some("Derived"),
            TableRowStyleRegions {
                first_row: true,
                ..Default::default()
            },
        );
        let ordinary = styles
            .table_cell_presentation_for_regions(Some("Derived"), TableRowStyleRegions::default());
        assert_eq!(first.defaults.shading(), Some(Color::rgb(0x11, 0x22, 0x33)));
        assert_eq!(
            ordinary.defaults.shading(),
            Some(Color::rgb(0xAA, 0xBB, 0xCC))
        );
    }

    #[test]
    fn conditional_cell_declarations_clear_inherited_values_and_bound_percentages() {
        let styles = parse(
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="Base">
                    <w:tblStylePr w:type="wholeTable"><w:tcPr>
                        <w:shd w:fill="112233"/>
                        <w:tcW w:w="2500" w:type="pct"/>
                    </w:tcPr></w:tblStylePr>
                </w:style>
                <w:style w:type="table" w:styleId="Derived">
                    <w:basedOn w:val="Base"/>
                    <w:tblStylePr w:type="band1Horz"><w:tcPr>
                        <w:shd w:val="nil" w:fill="AABBCC"/>
                        <w:tcW w:w="1440" w:type="dxa"/>
                    </w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="band2Horz"><w:tcPr>
                        <w:shd w:fill="auto"/>
                        <w:tcW w:w="NaN" w:type="pct"/>
                    </w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="firstRow"><w:tcPr>
                        <w:tcW w:w="-1" w:type="pct"/>
                    </w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="lastRow"><w:tcPr>
                        <w:tcW w:w="5001" w:type="pct"/>
                    </w:tcPr></w:tblStylePr>
                </w:style>
                <w:style w:type="table" w:styleId="Bounds">
                    <w:tblStylePr w:type="firstRow"><w:tcPr>
                        <w:tcW w:w="0" w:type="pct"/>
                    </w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="lastRow"><w:tcPr>
                        <w:tcW w:w="5000" w:type="pct"/>
                    </w:tcPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );

        for regions in [
            TableRowStyleRegions {
                band1_horizontal: true,
                ..Default::default()
            },
            TableRowStyleRegions {
                band2_horizontal: true,
                ..Default::default()
            },
        ] {
            let presentation = styles.table_cell_presentation_for_regions(Some("Derived"), regions);
            assert_eq!(presentation.defaults.shading(), None);
            assert_eq!(presentation.defaults.width_pct(), None);
        }
        for regions in [
            TableRowStyleRegions {
                first_row: true,
                ..Default::default()
            },
            TableRowStyleRegions {
                last_row: true,
                ..Default::default()
            },
        ] {
            let presentation = styles.table_cell_presentation_for_regions(Some("Derived"), regions);
            assert_eq!(
                presentation.defaults.shading(),
                Some(Color::rgb(0x11, 0x22, 0x33))
            );
            assert_eq!(presentation.defaults.width_pct(), None);
        }

        let both_bounds = styles.table_cell_presentation_for_regions(
            Some("Bounds"),
            TableRowStyleRegions {
                first_row: true,
                last_row: true,
                ..Default::default()
            },
        );
        assert_eq!(both_bounds.defaults.width_pct(), Some(1.0));
    }

    #[test]
    fn conditional_cell_region_order_uses_the_last_matching_region() {
        let styles = parse(
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="Order">
                    <w:tblPr><w:tblCellMar><w:top w:w="10"/></w:tblCellMar></w:tblPr>
                    <w:tblStylePr w:type="wholeTable"><w:tcPr><w:tcMar><w:top w:w="20"/></w:tcMar></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="band1Horz"><w:tcPr><w:tcMar><w:top w:w="30"/></w:tcMar></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="band2Horz"><w:tcPr><w:tcMar><w:top w:w="40"/></w:tcMar></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="firstRow"><w:tcPr><w:tcMar><w:top w:w="50"/></w:tcMar></w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="lastRow"><w:tcPr><w:tcMar><w:top w:w="60"/></w:tcMar></w:tcPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let presentation = styles.table_cell_presentation_for_regions(
            Some("Order"),
            TableRowStyleRegions {
                first_row: true,
                last_row: true,
                band1_horizontal: true,
                band2_horizontal: true,
                ..Default::default()
            },
        );

        assert_eq!(presentation.margins.logical_values().0, Some(60));
    }

    #[test]
    fn nonconditional_table_properties_ignore_history_and_unknown_wrappers() {
        let styles = parse(
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
                <w:style w:type="table" w:styleId="Scoped">
                    <w:tblPr>
                        <w:tblCellMar><w:top w:w="100"/></w:tblCellMar>
                        <w:tblW w:w="2500" w:type="pct"/>
                        <w:tblStyleRowBandSize w:val="2"/>
                        <w:tblPrChange><w:tblPr>
                            <w:tblCellMar><w:top w:w="900"/></w:tblCellMar>
                            <w:tblW w:w="4500" w:type="pct"/>
                            <w:tblStyleRowBandSize w:val="3"/>
                        </w:tblPr></w:tblPrChange>
                        <w:unknown>
                            <w:tblCellMar><w:top w:w="901"/></w:tblCellMar>
                            <w:tblW w:w="4000" w:type="pct"/>
                            <w:tblStyleRowBandSize w:val="1"/>
                        </w:unknown>
                    </w:tblPr>
                    <w:unknown><w:tblPr>
                        <w:tblCellMar><w:top w:w="902"/></w:tblCellMar>
                        <w:tblW w:w="3500" w:type="pct"/>
                        <w:tblStyleRowBandSize w:val="3"/>
                    </w:tblPr></w:unknown>
                </w:style>
            </w:styles>"#,
        );
        let presentation = styles
            .table_cell_presentation_for_regions(Some("Scoped"), TableRowStyleRegions::default());

        assert_eq!(presentation.margins.logical_values().0, Some(100));
        assert_eq!(styles.table_geometry(Some("Scoped")).width_pct, Some(0.5));
        assert_eq!(styles.table_row_band_size(Some("Scoped")), Some(2));
    }

    #[test]
    fn conditional_cell_mce_selects_one_branch_and_recovers_siblings() {
        let styles = parse(
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
                <w:style w:type="table" w:styleId="Mce">
                    <w:tblStylePr w:type="firstRow"><w:tcPr>
                        <mc:AlternateContent>
                            <mc:Choice Requires="w"/>
                            <mc:Fallback><w:shd w:fill="EEEEEE"/></mc:Fallback>
                        </mc:AlternateContent>
                        <w:tcMar><mc:AlternateContent>
                            <mc:Choice Requires="w"><w:top w:w="120"/></mc:Choice>
                            <mc:Fallback><w:top w:w="920"/></mc:Fallback>
                        </mc:AlternateContent><w:bottom w:w="240"/></w:tcMar>
                        <w:shd w:fill="123456"/>
                    </w:tcPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let presentation = styles.table_cell_presentation_for_regions(
            Some("Mce"),
            TableRowStyleRegions {
                first_row: true,
                ..Default::default()
            },
        );

        assert_eq!(presentation.margins.logical_values().0, Some(120));
        assert_eq!(presentation.margins.logical_values().2, Some(240));
        assert_eq!(
            presentation.defaults.shading(),
            Some(Color::rgb(0x12, 0x34, 0x56))
        );
    }

    #[test]
    fn conditional_cell_mce_depth_limits_recover_inside_tcpr_and_tcmar() {
        let mut tcpr_nested = String::new();
        for _ in 0..STYLE_CHAIN_LIMIT + 2 {
            tcpr_nested.push_str("<mc:AlternateContent><mc:Choice Requires=\"w\">");
        }
        tcpr_nested.push_str("<w:shd w:fill=\"EEEEEE\"/>");
        for _ in 0..STYLE_CHAIN_LIMIT + 2 {
            tcpr_nested.push_str(
                "</mc:Choice><mc:Fallback><w:shd w:fill=\"DDDDDD\"/></mc:Fallback></mc:AlternateContent>",
            );
        }

        let mut margin_nested = String::new();
        for _ in 0..132 {
            margin_nested.push_str("<mc:AlternateContent><mc:Choice Requires=\"w\">");
        }
        margin_nested.push_str("<w:top w:w=\"900\"/>");
        for _ in 0..132 {
            margin_nested.push_str(
                "</mc:Choice><mc:Fallback><w:top w:w=\"901\"/></mc:Fallback></mc:AlternateContent>",
            );
        }

        let xml = format!(
            r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"
                xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
                <w:style w:type="table" w:styleId="Deep">
                    <w:tblStylePr w:type="firstRow"><w:tcPr>
                        {tcpr_nested}<w:shd w:fill="123456"/>
                    </w:tcPr></w:tblStylePr>
                    <w:tblStylePr w:type="lastRow"><w:tcPr><w:tcMar>
                        {margin_nested}<w:bottom w:w="240"/>
                    </w:tcMar></w:tcPr></w:tblStylePr>
                </w:style>
            </w:styles>"#
        );
        let styles = parse(&xml);
        let first = styles.table_cell_presentation_for_regions(
            Some("Deep"),
            TableRowStyleRegions {
                first_row: true,
                ..Default::default()
            },
        );
        let last = styles.table_cell_presentation_for_regions(
            Some("Deep"),
            TableRowStyleRegions {
                last_row: true,
                ..Default::default()
            },
        );

        assert_eq!(first.defaults.shading(), Some(Color::rgb(0x12, 0x34, 0x56)));
        assert_eq!(last.margins.logical_values().0, None);
        assert_eq!(last.margins.logical_values().2, Some(240));
    }

    #[test]
    fn resolves_horizontal_table_bands_and_inherited_band_sizes() {
        let xml = r#"<w:styles xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
            <w:style w:type="table" w:styleId="BandBase">
                <w:tblPr><w:tblStyleRowBandSize w:val="2"/></w:tblPr>
                <w:tblStylePr w:type="band1Horz">
                    <w:trPr><w:cantSplit/></w:trPr>
                </w:tblStylePr>
                <w:tblStylePr w:type="band2Horz">
                    <w:trPr><w:cantSplit w:val="off"/></w:trPr>
                </w:tblStylePr>
                <w:tblStylePr w:type="firstRow">
                    <w:trPr><w:cantSplit w:val="off"/></w:trPr>
                </w:tblStylePr>
            </w:style>
            <w:style w:type="table" w:styleId="BandDerived">
                <w:basedOn w:val="BandBase"/>
                <w:tblPr><w:tblStyleRowBandSize w:val="3"/></w:tblPr>
                <w:tblStylePr w:type="band2Horz">
                    <w:trPr><w:cantSplit/></w:trPr>
                </w:tblStylePr>
            </w:style>
            <w:style w:type="table" w:styleId="InvalidKeepsBase">
                <w:basedOn w:val="BandBase"/>
                <w:tblPr><w:tblStyleRowBandSize w:val="4"/></w:tblPr>
            </w:style>
            <w:style w:type="table" w:styleId="ZeroDisables">
                <w:basedOn w:val="BandBase"/>
                <w:tblPr><w:tblStyleRowBandSize w:val="0"/></w:tblPr>
            </w:style>
            <w:style w:type="table" w:styleId="LastValidWins">
                <w:basedOn w:val="BandBase"/>
                <w:tblPr>
                    <w:tblStyleRowBandSize w:val="1"/>
                    <w:tblStyleRowBandSize w:val="invalid"/>
                    <w:tblStyleRowBandSize w:val="3"/>
                    <w:tblStyleRowBandSize w:val="9"/>
                </w:tblPr>
            </w:style>
            <w:style w:type="table" w:styleId="SelectedChoice">
                <w:basedOn w:val="BandBase"/>
                <w:tblPr><mc:AlternateContent>
                    <mc:Choice Requires="w14">
                        <w:tblStyleRowBandSize w:val="1"/>
                    </mc:Choice>
                    <mc:Fallback>
                        <w:tblStyleRowBandSize w:val="3"/>
                    </mc:Fallback>
                </mc:AlternateContent></w:tblPr>
            </w:style>
            <w:style w:type="paragraph" w:styleId="WrongKind">
                <w:tblStyleRowBandSize w:val="3"/>
                <w:tblStylePr w:type="band1Horz">
                    <w:trPr><w:cantSplit/></w:trPr>
                </w:tblStylePr>
            </w:style>
        </w:styles>"#;
        let styles = parse(xml);
        let band1 = TableRowStyleRegions {
            band1_horizontal: true,
            ..Default::default()
        };
        let band2 = TableRowStyleRegions {
            band2_horizontal: true,
            ..Default::default()
        };
        let band1_first = TableRowStyleRegions {
            first_row: true,
            band1_horizontal: true,
            ..Default::default()
        };
        let both_bands = TableRowStyleRegions {
            band1_horizontal: true,
            band2_horizontal: true,
            ..Default::default()
        };

        assert_eq!(styles.table_row_band_size(Some("BandBase")), Some(2));
        assert_eq!(styles.table_row_band_size(Some("BandDerived")), Some(3));
        assert_eq!(
            styles.table_row_band_size(Some("InvalidKeepsBase")),
            Some(2)
        );
        assert_eq!(styles.table_row_band_size(Some("ZeroDisables")), Some(0));
        assert_eq!(styles.table_row_band_size(Some("LastValidWins")), Some(3));
        assert_eq!(styles.table_row_band_size(Some("SelectedChoice")), Some(1));
        assert_eq!(styles.table_row_band_size(Some("WrongKind")), None);
        assert_eq!(styles.table_row_band_size(Some("missing")), None);
        assert_eq!(styles.table_row_band_size(None), None);

        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("BandBase"), band1),
            Some(true)
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("BandBase"), band2),
            Some(false)
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("BandBase"), band1_first),
            Some(false)
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("BandBase"), both_bands),
            Some(false)
        );
        assert_eq!(
            styles.table_row_cant_split_for_regions(Some("BandDerived"), band2),
            Some(true)
        );
    }

    #[test]
    fn resolves_all_conditional_table_row_region_properties() {
        let styles = parse(
            r#"<w:styles>
                <w:style w:type="table" w:styleId="AllRows">
                    <w:tblStylePr w:type="band1Vert"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="band2Vert"><w:trPr><w:cantSplit w:val="off"/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="firstCol"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="lastCol"><w:trPr><w:cantSplit w:val="off"/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="nwCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="neCell"><w:trPr><w:cantSplit w:val="off"/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="swCell"><w:trPr><w:cantSplit/></w:trPr></w:tblStylePr>
                    <w:tblStylePr w:type="seCell"><w:trPr><w:cantSplit w:val="off"/></w:trPr></w:tblStylePr>
                </w:style>
            </w:styles>"#,
        );
        let region = |region: TableRowStyleRegions| {
            styles.table_row_cant_split_for_regions(Some("AllRows"), region)
        };

        assert_eq!(
            region(TableRowStyleRegions {
                band1_vertical: true,
                ..Default::default()
            }),
            Some(true)
        );
        assert_eq!(
            region(TableRowStyleRegions {
                band2_vertical: true,
                ..Default::default()
            }),
            Some(false)
        );
        assert_eq!(
            region(TableRowStyleRegions {
                first_column: true,
                ..Default::default()
            }),
            Some(true)
        );
        assert_eq!(
            region(TableRowStyleRegions {
                last_column: true,
                ..Default::default()
            }),
            Some(false)
        );
        assert_eq!(
            region(TableRowStyleRegions {
                north_west: true,
                ..Default::default()
            }),
            Some(true)
        );
        assert_eq!(
            region(TableRowStyleRegions {
                north_east: true,
                ..Default::default()
            }),
            Some(false)
        );
        assert_eq!(
            region(TableRowStyleRegions {
                south_west: true,
                ..Default::default()
            }),
            Some(true)
        );
        assert_eq!(
            region(TableRowStyleRegions {
                south_east: true,
                ..Default::default()
            }),
            Some(false)
        );
    }

    #[test]
    fn resolves_complete_conditional_cell_regions_and_column_band_sizes() {
        let xml = r#"<w:styles xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">
            <w:style w:type="table" w:styleId="AllRegions">
                <w:tblPr><w:tblStyleColBandSize w:val="2"/></w:tblPr>
                <w:tblStylePr w:type="wholeTable"><w:tcPr><w:shd w:fill="010101"/></w:tcPr></w:tblStylePr>
                <w:tblStylePr w:type="band1Horz"><w:tcPr><w:shd w:fill="110001"/></w:tcPr></w:tblStylePr>
                <w:tblStylePr w:type="band2Horz"><w:tcPr><w:shd w:fill="120002"/></w:tcPr></w:tblStylePr>
                <w:tblStylePr w:type="band1Vert"><w:tcPr><w:shd w:fill="210001"/></w:tcPr></w:tblStylePr>
                <w:tblStylePr w:type="band2Vert"><w:tcPr><w:shd w:fill="220002"/></w:tcPr></w:tblStylePr>
                <w:tblStylePr w:type="firstCol"><w:tcPr><w:shd w:fill="310001"/></w:tcPr></w:tblStylePr>
                <w:tblStylePr w:type="lastCol"><w:tcPr><w:shd w:fill="320002"/></w:tcPr></w:tblStylePr>
                <w:tblStylePr w:type="firstRow"><w:tcPr><w:shd w:fill="410001"/></w:tcPr></w:tblStylePr>
                <w:tblStylePr w:type="lastRow"><w:tcPr><w:shd w:fill="420002"/></w:tcPr></w:tblStylePr>
                <w:tblStylePr w:type="nwCell"><w:tcPr><w:shd w:fill="510001"/></w:tcPr></w:tblStylePr>
                <w:tblStylePr w:type="neCell"><w:tcPr><w:shd w:fill="520002"/></w:tcPr></w:tblStylePr>
                <w:tblStylePr w:type="swCell"><w:tcPr><w:shd w:fill="530003"/></w:tcPr></w:tblStylePr>
                <w:tblStylePr w:type="seCell"><w:tcPr><w:shd w:fill="540004"/></w:tcPr></w:tblStylePr>
            </w:style>
            <w:style w:type="table" w:styleId="Derived">
                <w:basedOn w:val="AllRegions"/>
                <w:tblPr><w:tblStyleColBandSize w:val="3"/></w:tblPr>
                <w:tblStylePr w:type="neCell"><w:tcPr><w:shd w:fill="AABBCC"/></w:tcPr></w:tblStylePr>
            </w:style>
            <w:style w:type="table" w:styleId="InvalidKeepsBase">
                <w:basedOn w:val="AllRegions"/>
                <w:tblPr><w:tblStyleColBandSize w:val="4"/></w:tblPr>
            </w:style>
            <w:style w:type="table" w:styleId="ZeroDisables">
                <w:basedOn w:val="AllRegions"/>
                <w:tblPr><w:tblStyleColBandSize w:val="0"/></w:tblPr>
            </w:style>
            <w:style w:type="table" w:styleId="LastValidWins">
                <w:tblPr>
                    <w:tblStyleColBandSize w:val="1"/>
                    <w:tblStyleColBandSize w:val="invalid"/>
                    <w:tblStyleColBandSize w:val="3"/>
                    <w:tblStyleColBandSize w:val="9"/>
                </w:tblPr>
            </w:style>
            <w:style w:type="table" w:styleId="SelectedChoice">
                <w:tblPr><mc:AlternateContent>
                    <mc:Choice Requires="w14"><w:tblStyleColBandSize w:val="1"/></mc:Choice>
                    <mc:Fallback><w:tblStyleColBandSize w:val="3"/></mc:Fallback>
                </mc:AlternateContent></w:tblPr>
            </w:style>
        </w:styles>"#;
        let styles = parse(xml);
        let shade = |regions| {
            styles
                .table_cell_props(Some("AllRegions"))
                .presentation_for_regions(regions)
                .defaults
                .shading()
        };
        let color = |value: u32| {
            Some(Color::rgb(
                ((value >> 16) & 0xff) as u8,
                ((value >> 8) & 0xff) as u8,
                (value & 0xff) as u8,
            ))
        };

        assert_eq!(shade(TableCellStyleRegions::default()), color(0x010101));
        for (regions, expected) in [
            (
                TableCellStyleRegions {
                    band1_horizontal: true,
                    ..Default::default()
                },
                0x110001,
            ),
            (
                TableCellStyleRegions {
                    band2_horizontal: true,
                    ..Default::default()
                },
                0x120002,
            ),
            (
                TableCellStyleRegions {
                    band1_vertical: true,
                    ..Default::default()
                },
                0x210001,
            ),
            (
                TableCellStyleRegions {
                    band2_vertical: true,
                    ..Default::default()
                },
                0x220002,
            ),
            (
                TableCellStyleRegions {
                    first_column: true,
                    ..Default::default()
                },
                0x310001,
            ),
            (
                TableCellStyleRegions {
                    last_column: true,
                    ..Default::default()
                },
                0x320002,
            ),
            (
                TableCellStyleRegions {
                    first_row: true,
                    ..Default::default()
                },
                0x410001,
            ),
            (
                TableCellStyleRegions {
                    last_row: true,
                    ..Default::default()
                },
                0x420002,
            ),
            (
                TableCellStyleRegions {
                    north_west: true,
                    ..Default::default()
                },
                0x510001,
            ),
            (
                TableCellStyleRegions {
                    north_east: true,
                    ..Default::default()
                },
                0x520002,
            ),
            (
                TableCellStyleRegions {
                    south_west: true,
                    ..Default::default()
                },
                0x530003,
            ),
            (
                TableCellStyleRegions {
                    south_east: true,
                    ..Default::default()
                },
                0x540004,
            ),
        ] {
            assert_eq!(shade(regions), color(expected), "{regions:?}");
        }

        assert_eq!(
            shade(TableCellStyleRegions {
                band2_horizontal: true,
                band1_vertical: true,
                first_column: true,
                last_column: true,
                first_row: true,
                last_row: true,
                north_west: true,
                north_east: true,
                south_west: true,
                south_east: true,
                ..Default::default()
            }),
            color(0x540004)
        );
        assert_eq!(
            styles
                .table_cell_props(Some("Derived"))
                .presentation_for_regions(TableCellStyleRegions {
                    north_east: true,
                    ..Default::default()
                })
                .defaults
                .shading(),
            color(0xAABBCC)
        );

        assert_eq!(styles.table_col_band_size(Some("AllRegions")), Some(2));
        assert_eq!(styles.table_col_band_size(Some("Derived")), Some(3));
        assert_eq!(
            styles.table_col_band_size(Some("InvalidKeepsBase")),
            Some(2)
        );
        assert_eq!(styles.table_col_band_size(Some("ZeroDisables")), Some(0));
        assert_eq!(styles.table_col_band_size(Some("LastValidWins")), Some(3));
        assert_eq!(styles.table_col_band_size(Some("SelectedChoice")), Some(1));
        assert_eq!(styles.table_col_band_size(Some("missing")), None);
        assert_eq!(styles.table_col_band_size(None), None);
    }
}
