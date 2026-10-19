use std::collections::{BTreeMap, BTreeSet};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use super::xml_text::resolve_reference;
use super::{attr_local_trimmed, local};
use crate::model::{Chart, ChartKind, ChartSeries, ChartShape};

const MAX_CHART_DEPTH: usize = 64;
const MAX_CHART_SERIES: usize = 256;
const MAX_CHART_POINTS: usize = 65_536;
const MAX_TOTAL_CHART_POINTS: usize = 1 << 20;
const MAX_CHART_TEXT_BYTES: usize = 8 << 20;
const C_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const CX_NS: &str = "http://schemas.microsoft.com/office/drawing/2014/chartex";

/// Decode the bounded literal-cache shapes emitted by the native writer. Other
/// Office chart grammars intentionally return `None` so the package layer keeps
/// them opaque and diagnostics continue to report them as unsupported.
pub(crate) fn parse(xml: &str) -> Option<Chart> {
    let mut reader = Reader::from_str(xml);
    let chart = loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                break match start.name().as_ref() {
                    b"c:chartSpace" if has_exact_attr(&start, b"xmlns:c", C_NS) => {
                        parse_core(&mut reader)
                    }
                    b"cx:chartSpace" if has_exact_attr(&start, b"xmlns:cx", CX_NS) => {
                        parse_extended(&mut reader)
                    }
                    _ => None,
                }?;
            }
            Ok(Event::Decl(_) | Event::Comment(_) | Event::PI(_)) => {}
            Ok(Event::Text(text)) if text.decode().ok()?.trim().is_empty() => {}
            Ok(Event::Eof) | Err(_) => return None,
            _ => return None,
        }
    };
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => return Some(chart),
            Ok(Event::Comment(_) | Event::PI(_)) => {}
            Ok(Event::Text(text)) if text.decode().ok()?.trim().is_empty() => {}
            Err(_) => return None,
            _ => return None,
        }
    }
}

fn has_exact_attr(element: &BytesStart<'_>, key: &[u8], value: &str) -> bool {
    element.attributes().flatten().any(|attribute| {
        attribute.key.as_ref() == key
            && attribute
                .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, element.decoder())
                .ok()
                .is_some_and(|actual| actual == value)
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CoreFamily {
    Bar,
    Bar3D,
    Line,
    Line3D,
    Area,
    Area3D,
    Radar,
    Scatter,
    Bubble,
    Pie,
    Pie3D,
    OfPie,
    Doughnut,
    Surface,
    Surface3D,
    Stock,
}

#[derive(Default)]
struct SeriesAccumulator {
    name: String,
    categories: BTreeMap<usize, String>,
    x_values: BTreeMap<usize, f64>,
    values: BTreeMap<usize, f64>,
    bubble_sizes: BTreeMap<usize, f64>,
    saw_categories_literal: bool,
    saw_x_values_literal: bool,
    saw_values_literal: bool,
    saw_bubble_sizes_literal: bool,
}

#[derive(Clone, Copy)]
enum CorePointKind {
    Category,
    XValue,
    Value,
    BubbleSize,
}

struct CorePointAccumulator {
    index: usize,
    kind: CorePointKind,
    value: String,
}

struct CoreState {
    family: Option<CoreFamily>,
    grouping: Option<String>,
    bar_direction: Option<String>,
    marker: Option<String>,
    smooth: bool,
    radar_style: Option<String>,
    scatter_style: Option<String>,
    bubble_3d: bool,
    exploded: bool,
    of_pie_type: Option<String>,
    wireframe: bool,
    shape: ChartShape,
    stock_up_down_bars: bool,
    title: String,
    categories: Option<Vec<String>>,
    series: Vec<ChartSeries>,
    current_series: Option<SeriesAccumulator>,
    current_point: Option<CorePointAccumulator>,
    total_points: usize,
    text_bytes: usize,
}

impl Default for CoreState {
    fn default() -> Self {
        Self {
            family: None,
            grouping: None,
            bar_direction: None,
            marker: None,
            smooth: false,
            radar_style: None,
            scatter_style: None,
            bubble_3d: false,
            exploded: false,
            of_pie_type: None,
            wireframe: false,
            shape: ChartShape::Box,
            stock_up_down_bars: false,
            title: String::new(),
            categories: None,
            series: Vec::new(),
            current_series: None,
            current_point: None,
            total_points: 0,
            text_bytes: 0,
        }
    }
}

fn parse_core(reader: &mut Reader<&[u8]>) -> Option<Chart> {
    let mut path = vec![b"chartSpace".to_vec()];
    let mut state = CoreState::default();
    let mut closed = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if path.len() >= MAX_CHART_DEPTH {
                    return None;
                }
                let name = local(start.name().as_ref()).to_vec();
                path.push(name.clone());
                state.start(&name, &start, &path)?;
            }
            Ok(Event::Empty(empty)) => {
                if path.len() >= MAX_CHART_DEPTH {
                    return None;
                }
                let name = local(empty.name().as_ref()).to_vec();
                path.push(name.clone());
                state.start(&name, &empty, &path)?;
                state.end(&name)?;
                path.pop();
            }
            Ok(Event::Text(text)) => {
                let value = text.decode().ok()?.into_owned();
                state.text(&path, &value)?;
            }
            Ok(Event::GeneralRef(reference)) => {
                state.text(&path, &resolve_reference(&reference)?)?;
            }
            Ok(Event::End(end)) => {
                let name = local(end.name().as_ref()).to_vec();
                if path.last().map(Vec::as_slice) != Some(name.as_slice()) {
                    return None;
                }
                if name == b"chartSpace" && path.len() == 1 {
                    closed = true;
                    break;
                }
                state.end(&name)?;
                path.pop()?;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    if !closed || state.current_series.is_some() {
        return None;
    }
    state.finish()
}

impl CoreState {
    fn start(&mut self, name: &[u8], element: &BytesStart<'_>, path: &[Vec<u8>]) -> Option<()> {
        if let Some(family) = core_family(name) {
            if self.family.replace(family).is_some() {
                return None;
            }
        }
        match name {
            b"ser" => {
                if self.current_series.is_some() || self.series.len() >= MAX_CHART_SERIES {
                    return None;
                }
                self.current_series = Some(SeriesAccumulator::default());
            }
            b"pt" if self.current_series.is_some() => {
                if self.current_point.is_some() {
                    return None;
                }
                let kind = core_point_kind(path)?;
                let index = attr_local_trimmed(element, b"idx")
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|index| *index < MAX_CHART_POINTS)?;
                self.current_point = Some(CorePointAccumulator {
                    index,
                    kind,
                    value: String::new(),
                });
            }
            b"strLit" if self.current_series.is_some() && path_contains(path, b"cat") => {
                let series = self.current_series.as_mut()?;
                if std::mem::replace(&mut series.saw_categories_literal, true) {
                    return None;
                }
            }
            b"numLit" if self.current_series.is_some() => {
                let series = self.current_series.as_mut()?;
                let flag = if path_contains(path, b"bubbleSize") {
                    &mut series.saw_bubble_sizes_literal
                } else if path_contains(path, b"xVal") {
                    &mut series.saw_x_values_literal
                } else if path_contains(path, b"val") || path_contains(path, b"yVal") {
                    &mut series.saw_values_literal
                } else {
                    return Some(());
                };
                if std::mem::replace(flag, true) {
                    return None;
                }
            }
            b"grouping" => self.grouping = attr_local_trimmed(element, b"val"),
            b"barDir" => self.bar_direction = attr_local_trimmed(element, b"val"),
            b"symbol" if self.marker.is_none() => self.marker = attr_local_trimmed(element, b"val"),
            b"smooth" => self.smooth = on_value(element),
            b"radarStyle" => self.radar_style = attr_local_trimmed(element, b"val"),
            b"scatterStyle" => self.scatter_style = attr_local_trimmed(element, b"val"),
            b"bubble3D" => self.bubble_3d = on_value(element),
            b"explosion" => self.exploded = true,
            b"ofPieType" => self.of_pie_type = attr_local_trimmed(element, b"val"),
            b"wireframe" => self.wireframe = on_value(element),
            b"shape" => {
                self.shape = attr_local_trimmed(element, b"val")
                    .as_deref()
                    .and_then(chart_shape)
                    .unwrap_or(ChartShape::Box)
            }
            b"upDownBars" => self.stock_up_down_bars = true,
            _ => {}
        }
        Some(())
    }

    fn end(&mut self, name: &[u8]) -> Option<()> {
        match name {
            b"pt" => self.finish_point()?,
            b"ser" => self.finish_series()?,
            _ => {}
        }
        Some(())
    }

    fn text(&mut self, path: &[Vec<u8>], value: &str) -> Option<()> {
        self.text_bytes = self.text_bytes.checked_add(value.len())?;
        if self.text_bytes > MAX_CHART_TEXT_BYTES {
            return None;
        }
        if let Some(series) = self.current_series.as_mut() {
            if path_ends(path, &[b"ser", b"tx", b"v"]) {
                series.name.push_str(value);
                return Some(());
            }
            if path.last().is_some_and(|name| name.as_slice() == b"v") {
                if let Some(point) = self.current_point.as_mut() {
                    point.value.push_str(value);
                }
            }
        } else if path.last().is_some_and(|name| name.as_slice() == b"t")
            && path_contains(path, b"title")
        {
            self.title.push_str(value);
        }
        Some(())
    }

    fn finish_point(&mut self) -> Option<()> {
        let Some(point) = self.current_point.take() else {
            return Some(());
        };
        let series = self.current_series.as_mut()?;
        let inserted = match point.kind {
            CorePointKind::Category => series.categories.insert(point.index, point.value).is_none(),
            CorePointKind::XValue => series
                .x_values
                .insert(point.index, finite_number(&point.value)?)
                .is_none(),
            CorePointKind::Value => series
                .values
                .insert(point.index, finite_number(&point.value)?)
                .is_none(),
            CorePointKind::BubbleSize => series
                .bubble_sizes
                .insert(point.index, finite_number(&point.value)?)
                .is_none(),
        };
        if !inserted {
            return None;
        }
        self.total_points = self.total_points.checked_add(1)?;
        (self.total_points <= MAX_TOTAL_CHART_POINTS).then_some(())
    }

    fn finish_series(&mut self) -> Option<()> {
        let series = self.current_series.take()?;
        let family = self.family?;
        match family {
            CoreFamily::Scatter => {
                if !series.saw_x_values_literal || !series.saw_values_literal {
                    return None;
                }
            }
            CoreFamily::Bubble => {
                if !series.saw_x_values_literal
                    || !series.saw_values_literal
                    || !series.saw_bubble_sizes_literal
                {
                    return None;
                }
            }
            _ => {
                if !series.saw_categories_literal || !series.saw_values_literal {
                    return None;
                }
            }
        }
        let categories = dense_strings(series.categories)?;
        let _x_values = dense_numbers(series.x_values)?;
        if !categories.is_empty() {
            match &self.categories {
                Some(existing) if existing != &categories => return None,
                Some(_) => {}
                None => self.categories = Some(categories),
            }
        }
        self.series.push(ChartSeries {
            name: series.name,
            values: dense_numbers(series.values)?,
            bubble_sizes: dense_numbers(series.bubble_sizes)?,
        });
        Some(())
    }

    fn finish(self) -> Option<Chart> {
        let kind = match self.family? {
            CoreFamily::Bar => bar_kind(
                self.bar_direction.as_deref()?,
                self.grouping.as_deref()?,
                false,
            )?,
            CoreFamily::Bar3D => bar_kind(
                self.bar_direction.as_deref()?,
                self.grouping.as_deref()?,
                true,
            )?,
            CoreFamily::Line => match self.grouping.as_deref()? {
                "stacked" => ChartKind::StackedLine,
                "percentStacked" => ChartKind::PercentStackedLine,
                "standard" if self.smooth => ChartKind::SmoothLine,
                "standard" if self.marker.as_deref() == Some("none") => ChartKind::LineNoMarkers,
                "standard" => ChartKind::Line,
                _ => return None,
            },
            CoreFamily::Line3D => ChartKind::Line3D,
            CoreFamily::Area => area_kind(self.grouping.as_deref()?, false)?,
            CoreFamily::Area3D => area_kind(self.grouping.as_deref()?, true)?,
            CoreFamily::Radar => match self.radar_style.as_deref()? {
                "standard" => ChartKind::Radar,
                "marker" => ChartKind::RadarWithMarkers,
                "filled" => ChartKind::FilledRadar,
                _ => return None,
            },
            CoreFamily::Scatter => match self.scatter_style.as_deref()? {
                "lineMarker" => ChartKind::Scatter,
                "marker" => ChartKind::ScatterMarkers,
                "line" => ChartKind::ScatterLines,
                "smoothMarker" => ChartKind::ScatterSmooth,
                "smooth" => ChartKind::ScatterSmoothNoMarkers,
                _ => return None,
            },
            CoreFamily::Bubble if self.bubble_3d => ChartKind::Bubble3D,
            CoreFamily::Bubble => ChartKind::Bubble,
            CoreFamily::Pie if self.exploded => ChartKind::ExplodedPie,
            CoreFamily::Pie => ChartKind::Pie,
            CoreFamily::Pie3D if self.exploded => ChartKind::ExplodedPie3D,
            CoreFamily::Pie3D => ChartKind::Pie3D,
            CoreFamily::OfPie if self.of_pie_type.as_deref() == Some("pie") => ChartKind::PieOfPie,
            CoreFamily::OfPie if self.of_pie_type.as_deref() == Some("bar") => ChartKind::BarOfPie,
            CoreFamily::OfPie => return None,
            CoreFamily::Doughnut if self.exploded => ChartKind::ExplodedDoughnut,
            CoreFamily::Doughnut => ChartKind::Doughnut,
            CoreFamily::Surface => ChartKind::Surface,
            CoreFamily::Surface3D => ChartKind::Surface3D,
            CoreFamily::Stock if self.stock_up_down_bars => ChartKind::Stock,
            CoreFamily::Stock => ChartKind::StockHighLowClose,
        };
        Some(Chart {
            kind,
            title: non_empty(self.title),
            categories: self.categories.unwrap_or_default(),
            series: self.series,
            width_px: None,
            height_px: None,
            alt: None,
            wireframe: self.wireframe,
            shape: self.shape,
        })
    }
}

fn core_point_kind(path: &[Vec<u8>]) -> Option<CorePointKind> {
    if path_contains(path, b"cat") && path_contains(path, b"strLit") {
        Some(CorePointKind::Category)
    } else if path_contains(path, b"xVal") && path_contains(path, b"numLit") {
        Some(CorePointKind::XValue)
    } else if (path_contains(path, b"val") || path_contains(path, b"yVal"))
        && path_contains(path, b"numLit")
    {
        Some(CorePointKind::Value)
    } else if path_contains(path, b"bubbleSize") && path_contains(path, b"numLit") {
        Some(CorePointKind::BubbleSize)
    } else {
        None
    }
}

fn core_family(name: &[u8]) -> Option<CoreFamily> {
    match name {
        b"barChart" => Some(CoreFamily::Bar),
        b"bar3DChart" => Some(CoreFamily::Bar3D),
        b"lineChart" => Some(CoreFamily::Line),
        b"line3DChart" => Some(CoreFamily::Line3D),
        b"areaChart" => Some(CoreFamily::Area),
        b"area3DChart" => Some(CoreFamily::Area3D),
        b"radarChart" => Some(CoreFamily::Radar),
        b"scatterChart" => Some(CoreFamily::Scatter),
        b"bubbleChart" => Some(CoreFamily::Bubble),
        b"pieChart" => Some(CoreFamily::Pie),
        b"pie3DChart" => Some(CoreFamily::Pie3D),
        b"ofPieChart" => Some(CoreFamily::OfPie),
        b"doughnutChart" => Some(CoreFamily::Doughnut),
        b"surfaceChart" => Some(CoreFamily::Surface),
        b"surface3DChart" => Some(CoreFamily::Surface3D),
        b"stockChart" => Some(CoreFamily::Stock),
        _ => None,
    }
}

fn bar_kind(direction: &str, grouping: &str, three_d: bool) -> Option<ChartKind> {
    Some(match (direction, grouping, three_d) {
        ("bar", "clustered", false) => ChartKind::Bar,
        ("bar", "stacked", false) => ChartKind::StackedBar,
        ("bar", "percentStacked", false) => ChartKind::PercentStackedBar,
        ("bar", "clustered", true) => ChartKind::Bar3D,
        ("bar", "stacked", true) => ChartKind::StackedBar3D,
        ("bar", "percentStacked", true) => ChartKind::PercentStackedBar3D,
        ("col", "clustered", false) => ChartKind::Column,
        ("col", "stacked", false) => ChartKind::StackedColumn,
        ("col", "percentStacked", false) => ChartKind::PercentStackedColumn,
        ("col", "clustered", true) => ChartKind::Column3D,
        ("col", "stacked", true) => ChartKind::StackedColumn3D,
        ("col", "percentStacked", true) => ChartKind::PercentStackedColumn3D,
        _ => return None,
    })
}

fn area_kind(grouping: &str, three_d: bool) -> Option<ChartKind> {
    Some(match (grouping, three_d) {
        ("standard", false) => ChartKind::Area,
        ("stacked", false) => ChartKind::StackedArea,
        ("percentStacked", false) => ChartKind::PercentStackedArea,
        ("standard", true) => ChartKind::Area3D,
        ("stacked", true) => ChartKind::StackedArea3D,
        ("percentStacked", true) => ChartKind::PercentStackedArea3D,
        _ => return None,
    })
}

#[derive(Default)]
struct ExtendedDataAccumulator {
    id: String,
    categories: BTreeMap<usize, String>,
    values: BTreeMap<usize, f64>,
    saw_categories: bool,
    saw_values: bool,
}

#[derive(Default)]
struct ExtendedSeriesAccumulator {
    name: String,
    data_id: Option<String>,
    kind: Option<ChartKind>,
}

#[derive(Clone, Copy)]
enum ExtendedDimension {
    Category,
    Value,
}

struct ExtendedPointAccumulator {
    index: usize,
    dimension: ExtendedDimension,
    value: String,
}

#[derive(Default)]
struct ExtendedState {
    data: BTreeMap<String, (Vec<String>, Vec<f64>)>,
    series: Vec<ExtendedSeriesAccumulator>,
    current_data: Option<ExtendedDataAccumulator>,
    current_series: Option<ExtendedSeriesAccumulator>,
    current_dimension: Option<ExtendedDimension>,
    current_point: Option<ExtendedPointAccumulator>,
    title: String,
    total_points: usize,
    text_bytes: usize,
}

fn parse_extended(reader: &mut Reader<&[u8]>) -> Option<Chart> {
    let mut path = vec![b"chartSpace".to_vec()];
    let mut state = ExtendedState::default();
    let mut closed = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(start)) => {
                if path.len() >= MAX_CHART_DEPTH {
                    return None;
                }
                let name = local(start.name().as_ref()).to_vec();
                path.push(name.clone());
                state.start(&name, &start)?;
            }
            Ok(Event::Empty(empty)) => {
                if path.len() >= MAX_CHART_DEPTH {
                    return None;
                }
                let name = local(empty.name().as_ref()).to_vec();
                path.push(name.clone());
                state.start(&name, &empty)?;
                state.end(&name)?;
                path.pop();
            }
            Ok(Event::Text(text)) => {
                let value = text.decode().ok()?.into_owned();
                state.text(&path, &value)?;
            }
            Ok(Event::GeneralRef(reference)) => {
                state.text(&path, &resolve_reference(&reference)?)?;
            }
            Ok(Event::End(end)) => {
                let name = local(end.name().as_ref()).to_vec();
                if path.last().map(Vec::as_slice) != Some(name.as_slice()) {
                    return None;
                }
                if name == b"chartSpace" && path.len() == 1 {
                    closed = true;
                    break;
                }
                state.end(&name)?;
                path.pop()?;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    if !closed || state.current_data.is_some() || state.current_series.is_some() {
        return None;
    }
    state.finish()
}

impl ExtendedState {
    fn start(&mut self, name: &[u8], element: &BytesStart<'_>) -> Option<()> {
        match name {
            b"data" => {
                if self.current_data.is_some()
                    || self.current_series.is_some()
                    || self.data.len() >= MAX_CHART_SERIES
                {
                    return None;
                }
                self.current_data = Some(ExtendedDataAccumulator {
                    id: attr_local_trimmed(element, b"id")?,
                    ..ExtendedDataAccumulator::default()
                });
            }
            b"series" => {
                if self.current_series.is_some()
                    || self.current_data.is_some()
                    || self.series.len() >= MAX_CHART_SERIES
                {
                    return None;
                }
                self.current_series = Some(ExtendedSeriesAccumulator {
                    kind: attr_local_trimmed(element, b"layoutId")
                        .as_deref()
                        .and_then(extended_kind),
                    ..ExtendedSeriesAccumulator::default()
                });
                self.current_series.as_ref()?.kind?;
            }
            b"strDim" if self.current_data.is_some() => {
                if self.current_dimension.is_some()
                    || attr_local_trimmed(element, b"type").as_deref() != Some("cat")
                {
                    return None;
                }
                let data = self.current_data.as_mut()?;
                if std::mem::replace(&mut data.saw_categories, true) {
                    return None;
                }
                self.current_dimension = Some(ExtendedDimension::Category);
            }
            b"numDim" if self.current_data.is_some() => {
                if self.current_dimension.is_some()
                    || attr_local_trimmed(element, b"type").as_deref() != Some("val")
                {
                    return None;
                }
                let data = self.current_data.as_mut()?;
                if std::mem::replace(&mut data.saw_values, true) {
                    return None;
                }
                self.current_dimension = Some(ExtendedDimension::Value);
            }
            b"dataId" if self.current_series.is_some() => {
                let series = self.current_series.as_mut()?;
                if series.data_id.is_some() {
                    return None;
                }
                series.data_id = Some(attr_local_trimmed(element, b"val")?);
            }
            b"pt" if self.current_data.is_some() => {
                if self.current_point.is_some() {
                    return None;
                }
                let index = attr_local_trimmed(element, b"idx")
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|index| *index < MAX_CHART_POINTS)?;
                self.current_point = Some(ExtendedPointAccumulator {
                    index,
                    dimension: self.current_dimension?,
                    value: String::new(),
                });
            }
            _ => {}
        }
        Some(())
    }

    fn end(&mut self, name: &[u8]) -> Option<()> {
        match name {
            b"pt" => self.finish_point()?,
            b"strDim" | b"numDim" if self.current_data.is_some() => {
                if self.current_point.is_some() {
                    return None;
                }
                self.current_dimension = None;
            }
            b"data" => {
                let data = self.current_data.take()?;
                if !data.saw_categories || !data.saw_values {
                    return None;
                }
                let values = (dense_strings(data.categories)?, dense_numbers(data.values)?);
                if self.data.insert(data.id, values).is_some() {
                    return None;
                }
            }
            b"series" => self.series.push(self.current_series.take()?),
            _ => {}
        }
        Some(())
    }

    fn text(&mut self, path: &[Vec<u8>], value: &str) -> Option<()> {
        self.text_bytes = self.text_bytes.checked_add(value.len())?;
        if self.text_bytes > MAX_CHART_TEXT_BYTES {
            return None;
        }
        if self.current_data.is_some() {
            if path.last().is_some_and(|name| name.as_slice() == b"v") {
                if let Some(point) = self.current_point.as_mut() {
                    point.value.push_str(value);
                }
            }
        } else if let Some(series) = self.current_series.as_mut() {
            if path.last().is_some_and(|name| name.as_slice() == b"v")
                && path_contains(path, b"txData")
            {
                series.name.push_str(value);
            }
        } else if path.last().is_some_and(|name| name.as_slice() == b"t")
            && path_contains(path, b"title")
        {
            self.title.push_str(value);
        }
        Some(())
    }

    fn finish_point(&mut self) -> Option<()> {
        let Some(point) = self.current_point.take() else {
            return Some(());
        };
        let data = self.current_data.as_mut()?;
        let inserted = match point.dimension {
            ExtendedDimension::Category => {
                data.categories.insert(point.index, point.value).is_none()
            }
            ExtendedDimension::Value => data
                .values
                .insert(point.index, finite_number(&point.value)?)
                .is_none(),
        };
        if !inserted {
            return None;
        }
        self.total_points = self.total_points.checked_add(1)?;
        (self.total_points <= MAX_TOTAL_CHART_POINTS).then_some(())
    }

    fn finish(self) -> Option<Chart> {
        let mut kind = None;
        let mut categories = None;
        let mut series_out = Vec::with_capacity(self.series.len());
        let mut used_data = BTreeSet::new();
        for series in self.series {
            let series_kind = series.kind?;
            if kind
                .replace(series_kind)
                .is_some_and(|old| old != series_kind)
            {
                return None;
            }
            let data_id = series.data_id?;
            if !used_data.insert(data_id.clone()) {
                return None;
            }
            let (series_categories, values) = self.data.get(&data_id)?;
            match &categories {
                Some(existing) if existing != series_categories => return None,
                Some(_) => {}
                None => categories = Some(series_categories.clone()),
            }
            series_out.push(ChartSeries {
                name: series.name,
                values: values.clone(),
                bubble_sizes: Vec::new(),
            });
        }
        if used_data.len() != self.data.len() {
            return None;
        }
        Some(Chart {
            kind: kind?,
            title: non_empty(self.title),
            categories: categories.unwrap_or_default(),
            series: series_out,
            width_px: None,
            height_px: None,
            alt: None,
            wireframe: false,
            shape: ChartShape::Box,
        })
    }
}

fn extended_kind(value: &str) -> Option<ChartKind> {
    match value {
        "waterfall" => Some(ChartKind::Waterfall),
        "treemap" => Some(ChartKind::Treemap),
        "sunburst" => Some(ChartKind::Sunburst),
        "histogram" => Some(ChartKind::Histogram),
        "boxWhisker" => Some(ChartKind::BoxWhisker),
        "funnel" => Some(ChartKind::Funnel),
        _ => None,
    }
}

fn on_value(element: &BytesStart<'_>) -> bool {
    !matches!(
        attr_local_trimmed(element, b"val").as_deref(),
        Some("0" | "false" | "off" | "no")
    )
}

fn chart_shape(value: &str) -> Option<ChartShape> {
    match value {
        "box" => Some(ChartShape::Box),
        "cylinder" => Some(ChartShape::Cylinder),
        "cone" => Some(ChartShape::Cone),
        "coneToMax" => Some(ChartShape::ConeToMax),
        "pyramid" => Some(ChartShape::Pyramid),
        "pyramidToMax" => Some(ChartShape::PyramidToMax),
        _ => None,
    }
}

fn finite_number(value: &str) -> Option<f64> {
    value
        .trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn dense_strings(values: BTreeMap<usize, String>) -> Option<Vec<String>> {
    dense(values)
}

fn dense_numbers(values: BTreeMap<usize, f64>) -> Option<Vec<f64>> {
    dense(values)
}

fn dense<T>(values: BTreeMap<usize, T>) -> Option<Vec<T>> {
    if values.keys().copied().eq(0..values.len()) {
        Some(values.into_values().collect())
    } else {
        None
    }
}

fn path_contains(path: &[Vec<u8>], name: &[u8]) -> bool {
    path.iter().any(|part| part.as_slice() == name)
}

fn path_ends(path: &[Vec<u8>], suffix: &[&[u8]]) -> bool {
    path.len() >= suffix.len()
        && path[path.len() - suffix.len()..]
            .iter()
            .zip(suffix)
            .all(|(actual, expected)| actual.as_slice() == *expected)
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core_chart(series: &str) -> String {
        format!(
            r#"<c:chartSpace xmlns:c="{C_NS}"><c:chart><c:plotArea><c:barChart><c:barDir val="col"/><c:grouping val="clustered"/>{series}</c:barChart></c:plotArea></c:chart></c:chartSpace>"#
        )
    }

    fn literal_series() -> &'static str {
        r#"<c:ser><c:tx><c:v>Series</c:v></c:tx><c:cat><c:strLit><c:ptCount val="2"/><c:pt idx="0"><c:v></c:v></c:pt><c:pt idx="1"><c:v>B</c:v></c:pt></c:strLit></c:cat><c:val><c:numLit><c:formatCode>General</c:formatCode><c:ptCount val="2"/><c:pt idx="0"><c:v>1.5</c:v></c:pt><c:pt idx="1"><c:v>2</c:v></c:pt></c:numLit></c:val></c:ser>"#
    }

    fn extended_chart(data: &str, series: &str) -> String {
        format!(
            r#"<cx:chartSpace xmlns:cx="{CX_NS}"><cx:chartData>{data}</cx:chartData><cx:chart><cx:plotArea><cx:plotAreaRegion>{series}</cx:plotAreaRegion></cx:plotArea></cx:chart></cx:chartSpace>"#
        )
    }

    fn extended_data(id: &str) -> String {
        format!(
            r#"<cx:data id="{id}"><cx:strDim type="cat"><cx:lvl><cx:pt idx="0"><cx:v></cx:v></cx:pt></cx:lvl></cx:strDim><cx:numDim type="val"><cx:lvl><cx:pt idx="0"><cx:v>3</cx:v></cx:pt></cx:lvl></cx:numDim></cx:data>"#
        )
    }

    fn extended_series(id: &str) -> String {
        format!(r#"<cx:series layoutId="waterfall"><cx:dataId val="{id}"/></cx:series>"#)
    }

    #[test]
    fn parses_empty_literal_category_points() {
        let chart = parse(&core_chart(literal_series())).expect("literal chart parses");
        assert_eq!(chart.kind, ChartKind::Column);
        assert_eq!(chart.categories, ["", "B"]);
        assert_eq!(chart.series[0].values, [1.5, 2.0]);

        let chart = parse(&extended_chart(
            &extended_data("d1"),
            &extended_series("d1"),
        ))
        .expect("literal chartEx parses");
        assert_eq!(chart.categories, [""]);
        assert_eq!(chart.series[0].values, [3.0]);
    }

    #[test]
    fn rejects_formula_caches_and_multiple_chart_families() {
        let formula_series = r#"<c:ser><c:cat><c:strRef><c:f>Sheet1!A1</c:f><c:strCache><c:pt idx="0"><c:v>A</c:v></c:pt></c:strCache></c:strRef></c:cat><c:val><c:numRef><c:f>Sheet1!B1</c:f><c:numCache><c:pt idx="0"><c:v>1</c:v></c:pt></c:numCache></c:numRef></c:val></c:ser>"#;
        assert!(parse(&core_chart(formula_series)).is_none());

        let multiple = core_chart("").replace(
            "</c:plotArea>",
            r#"<c:lineChart><c:grouping val="standard"/></c:lineChart></c:plotArea>"#,
        );
        assert!(parse(&multiple).is_none());
    }

    #[test]
    fn rejects_sparse_duplicate_and_non_finite_literal_points() {
        let sparse = core_chart(literal_series()).replace(r#"idx="1""#, r#"idx="2""#);
        assert!(parse(&sparse).is_none());

        let duplicate = core_chart(literal_series()).replace(r#"idx="1""#, r#"idx="0""#);
        assert!(parse(&duplicate).is_none());

        let non_finite = core_chart(literal_series()).replace(">1.5<", ">NaN<");
        assert!(parse(&non_finite).is_none());
    }

    #[test]
    fn rejects_malformed_trailing_wrong_namespace_and_over_deep_xml() {
        let valid = core_chart(literal_series());
        assert!(parse(valid.trim_end_matches("</c:chartSpace>")).is_none());
        assert!(parse(&format!("{valid}<extra/>")).is_none());
        assert!(parse(&valid.replace(C_NS, "urn:not-a-chart")).is_none());

        let opens = "<x>".repeat(MAX_CHART_DEPTH);
        let closes = "</x>".repeat(MAX_CHART_DEPTH);
        let deep = format!(r#"<c:chartSpace xmlns:c="{C_NS}">{opens}{closes}</c:chartSpace>"#);
        assert!(parse(&deep).is_none());
    }

    #[test]
    fn chart_ex_requires_complete_unique_literal_data() {
        let data = extended_data("d1");
        let missing_values = data.replace(
            r#"<cx:numDim type="val"><cx:lvl><cx:pt idx="0"><cx:v>3</cx:v></cx:pt></cx:lvl></cx:numDim>"#,
            "",
        );
        assert!(parse(&extended_chart(&missing_values, &extended_series("d1"))).is_none());

        let duplicate_reference = format!("{}{}", extended_series("d1"), extended_series("d1"));
        assert!(parse(&extended_chart(&data, &duplicate_reference)).is_none());

        let unused_data = format!("{}{}", data, extended_data("d2"));
        assert!(parse(&extended_chart(&unused_data, &extended_series("d1"))).is_none());
    }
}
