//! Chart data structures

use super::types::ChartType;

/// Chart data series
#[derive(Clone, Debug)]
pub struct ChartSeries {
    pub name: String,
    pub values: Vec<f64>,
}

impl ChartSeries {
    /// Create a new chart series
    pub fn new(name: &str, values: Vec<f64>) -> Self {
        ChartSeries {
            name: name.to_string(),
            values,
        }
    }

    /// Get the number of data points
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if series is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Position of the chart legend relative to the plot area.
///
/// Maps to OOXML's `c:legendPos val="..."` enum (`"b"` / `"l"` / `"r"` / `"t"` /
/// `"tr"`). Default is [`LegendPosition::Right`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum LegendPosition {
    #[default]
    Right,
    Left,
    Top,
    TopRight,
    Bottom,
}

impl LegendPosition {
    /// OOXML `c:legendPos val` attribute value.
    pub fn as_str(self) -> &'static str {
        match self {
            LegendPosition::Right => "r",
            LegendPosition::Left => "l",
            LegendPosition::Top => "t",
            LegendPosition::TopRight => "tr",
            LegendPosition::Bottom => "b",
        }
    }
}

/// Position of data labels relative to their data point.
///
/// Maps to OOXML's `c:dLblPos val="..."` enum (`"bestFit"` / `"ctr"` / `"inEnd"`
/// / `"inBase"` / `"outEnd"` / `"l"` / `"r"` / `"t"` / `"b"`). Default is
/// [`LabelPosition::BestFit`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum LabelPosition {
    #[default]
    BestFit,
    Center,
    InsideBase,
    InsideEnd,
    OutsideEnd,
    Left,
    Right,
    Top,
    Bottom,
}

impl LabelPosition {
    /// OOXML `c:dLblPos val` attribute value.
    pub fn as_str(self) -> &'static str {
        match self {
            LabelPosition::BestFit => "bestFit",
            LabelPosition::Center => "ctr",
            LabelPosition::InsideBase => "inBase",
            LabelPosition::InsideEnd => "inEnd",
            LabelPosition::OutsideEnd => "outEnd",
            LabelPosition::Left => "l",
            LabelPosition::Right => "r",
            LabelPosition::Top => "t",
            LabelPosition::Bottom => "b",
        }
    }
}

/// Legend configuration.
#[derive(Clone, Debug, Default)]
pub struct LegendConfig {
    /// Whether the legend is shown. OOXML emits no `<c:legend>` block when
    /// `false` (PowerPoint then uses the layout/master defaults).
    pub show: bool,
    /// Position around the plot area.
    pub position: LegendPosition,
}

impl LegendConfig {
    /// Default legend: shown, at the right of the plot area (matches
    /// ppt-rs's pre-existing emission).
    pub fn shown() -> Self {
        Self {
            show: true,
            position: LegendPosition::Right,
        }
    }

    /// Hide the legend entirely.
    pub fn hidden() -> Self {
        Self {
            show: false,
            position: LegendPosition::Right,
        }
    }

    pub fn with_position(mut self, position: LegendPosition) -> Self {
        self.position = position;
        self
    }

    pub fn with_show(mut self, show: bool) -> Self {
        self.show = show;
        self
    }
}

/// Configuration for a chart axis (value or category).
#[derive(Clone, Debug, Default)]
pub struct AxisConfig {
    /// Force the axis minimum (OOXML `c:min val="…"`). `None` → auto.
    pub min: Option<f64>,
    /// Force the axis maximum (OOXML `c:max val="…"`). `None` → auto.
    pub max: Option<f64>,
    /// Major tick interval (OOXML `c:majorUnit val="…"`). `None` → auto.
    pub major_unit: Option<f64>,
    /// Minor tick interval (OOXML `c:minorUnit val="…"`). `None` → auto.
    pub minor_unit: Option<f64>,
    /// Use logarithmic scale (OOXML `c:logScale base="10"`). `false` → linear.
    pub log_scale: bool,
    /// Number format string for axis tick labels (OOXML `c:numFmt
    /// formatCode="…"`). `None` → `General` (the default we already emit).
    pub number_format: Option<String>,
    /// Tick label font size in points. `None` → default.
    pub tick_label_size_pt: Option<u32>,
    /// Tick label bold. `false` → default.
    pub tick_label_bold: bool,
    /// Whether category labels rotate by 45°. Convenience flag; for more
    /// granular control use `tick_label_rotation`.
    pub tick_label_rotation_deg: Option<i32>,
}

impl AxisConfig {
    pub fn with_min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }
    pub fn with_max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }
    pub fn with_major_unit(mut self, unit: f64) -> Self {
        self.major_unit = Some(unit);
        self
    }
    pub fn with_minor_unit(mut self, unit: f64) -> Self {
        self.minor_unit = Some(unit);
        self
    }
    pub fn with_log_scale(mut self, base: u32) -> Self {
        // Clamp to OOXML-allowed range (2..=99).
        self.log_scale = (2..=99).contains(&base);
        self
    }
    pub fn with_number_format(mut self, format: impl Into<String>) -> Self {
        self.number_format = Some(format.into());
        self
    }
    pub fn with_tick_label_size_pt(mut self, pts: u32) -> Self {
        self.tick_label_size_pt = Some(pts.max(8));
        self
    }
    pub fn with_tick_label_bold(mut self, bold: bool) -> Self {
        self.tick_label_bold = bold;
        self
    }
    pub fn with_tick_label_rotation(mut self, deg: i32) -> Self {
        // OOXML accepts -360..=360; clamp for safety.
        let d = deg.rem_euclid(360);
        self.tick_label_rotation_deg = Some(if d > 180 { d - 360 } else { d });
        self
    }
}

/// Configuration for per-plot data labels.
///
/// Controls whether each data point shows its value, category name, and
/// percentage, and where the labels sit relative to the marker. All fields
/// default to "off" — the generated chart shows nothing beside the bars /
/// slices unless the user opts in.
#[derive(Clone, Debug, Default)]
pub struct DataLabelsConfig {
    pub show_value: bool,
    pub show_category_name: bool,
    pub show_percentage: bool,
    pub show_series_name: bool,
    pub position: LabelPosition,
}

impl DataLabelsConfig {
    pub fn with_show_value(mut self, show: bool) -> Self {
        self.show_value = show;
        self
    }
    pub fn with_show_category_name(mut self, show: bool) -> Self {
        self.show_category_name = show;
        self
    }
    pub fn with_show_percentage(mut self, show: bool) -> Self {
        self.show_percentage = show;
        self
    }
    pub fn with_show_series_name(mut self, show: bool) -> Self {
        self.show_series_name = show;
        self
    }
    pub fn with_position(mut self, position: LabelPosition) -> Self {
        self.position = position;
        self
    }

    /// Quick preset: show the value at the inside-end of each marker.
    /// Common pattern for bar charts where the data sits on top of the bar.
    pub fn show_values() -> Self {
        Self {
            show_value: true,
            position: LabelPosition::InsideEnd,
            ..Default::default()
        }
    }

    /// Quick preset: show the percentage outside each slice. For pie/doughnut.
    pub fn show_percentages_outside() -> Self {
        Self {
            show_percentage: true,
            position: LabelPosition::OutsideEnd,
            ..Default::default()
        }
    }
}

/// Chart definition
#[derive(Clone, Debug)]
pub struct Chart {
    pub title: String,
    pub chart_type: ChartType,
    pub categories: Vec<String>,
    pub series: Vec<ChartSeries>,
    pub x: u32,      // Position X in EMU
    pub y: u32,      // Position Y in EMU
    pub width: u32,  // Width in EMU
    pub height: u32, // Height in EMU
    pub legend: LegendConfig,
    pub value_axis: AxisConfig,
    pub category_axis: AxisConfig,
    pub data_labels: DataLabelsConfig,
}

impl Chart {
    /// Create a new chart with default legend (shown, right) and axes.
    pub fn new(
        title: &str,
        chart_type: ChartType,
        categories: Vec<String>,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Self {
        Chart {
            title: title.to_string(),
            chart_type,
            categories,
            series: Vec::new(),
            x,
            y,
            width,
            height,
            legend: LegendConfig::shown(),
            value_axis: AxisConfig::default(),
            category_axis: AxisConfig::default(),
            data_labels: DataLabelsConfig::default(),
        }
    }

    /// Add a data series
    pub fn add_series(mut self, series: ChartSeries) -> Self {
        self.series.push(series);
        self
    }

    /// Get number of categories
    pub fn category_count(&self) -> usize {
        self.categories.len()
    }

    /// Get number of series
    pub fn series_count(&self) -> usize {
        self.series.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chart_series() {
        let series = ChartSeries::new("Sales", vec![10.0, 20.0, 30.0]);
        assert_eq!(series.name, "Sales");
        assert_eq!(series.len(), 3);
        assert!(!series.is_empty());
    }

    #[test]
    fn test_chart_add_series() {
        let chart = Chart::new(
            "Test",
            ChartType::Pie,
            vec!["A".to_string()],
            0,
            0,
            1000000,
            1000000,
        )
        .add_series(ChartSeries::new("Data", vec![50.0]));

        assert_eq!(chart.series_count(), 1);
    }

    #[test]
    fn test_legend_position_str() {
        assert_eq!(LegendPosition::Bottom.as_str(), "b");
        assert_eq!(LegendPosition::Top.as_str(), "t");
        assert_eq!(LegendPosition::Right.as_str(), "r");
        assert_eq!(LegendPosition::Left.as_str(), "l");
        assert_eq!(LegendPosition::TopRight.as_str(), "tr");
    }

    #[test]
    fn test_label_position_str() {
        assert_eq!(LabelPosition::BestFit.as_str(), "bestFit");
        assert_eq!(LabelPosition::Center.as_str(), "ctr");
        assert_eq!(LabelPosition::InsideEnd.as_str(), "inEnd");
        assert_eq!(LabelPosition::OutsideEnd.as_str(), "outEnd");
    }

    #[test]
    fn test_axis_log_scale_rejects_out_of_range() {
        // OOXML accepts 2..=99 — anything else falls back to linear.
        let cfg = AxisConfig::default().with_log_scale(1);
        assert!(!cfg.log_scale);
        let cfg = AxisConfig::default().with_log_scale(10);
        assert!(cfg.log_scale);
        let cfg = AxisConfig::default().with_log_scale(100);
        assert!(!cfg.log_scale);
    }

    #[test]
    fn test_axis_tick_label_rotation_wraps() {
        let cfg = AxisConfig::default().with_tick_label_rotation(720);
        // 720° → 0°; the wrap is `-180..=180` so it normalizes to 0.
        assert_eq!(cfg.tick_label_rotation_deg, Some(0));
        let cfg = AxisConfig::default().with_tick_label_rotation(225);
        assert_eq!(cfg.tick_label_rotation_deg, Some(-135));
    }

    #[test]
    fn test_data_labels_presets() {
        let v = DataLabelsConfig::show_values();
        assert!(v.show_value);
        assert!(!v.show_percentage);
        assert_eq!(v.position, LabelPosition::InsideEnd);
        let p = DataLabelsConfig::show_percentages_outside();
        assert!(p.show_percentage);
        assert_eq!(p.position, LabelPosition::OutsideEnd);
    }

    #[test]
    fn test_chart_default_legend_is_shown_right() {
        let chart = Chart::new(
            "Test",
            ChartType::Bar,
            vec!["A".to_string()],
            0,
            0,
            1000000,
            1000000,
        );
        assert!(chart.legend.show);
        assert_eq!(chart.legend.position, LegendPosition::Right);
    }
}
