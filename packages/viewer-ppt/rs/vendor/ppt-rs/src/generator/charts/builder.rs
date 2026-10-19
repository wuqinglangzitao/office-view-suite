//! Chart builder for fluent API

use super::data::{AxisConfig, Chart, ChartSeries, DataLabelsConfig, LegendConfig};
use super::types::ChartType;
use crate::core::{ElementPlacement, validate_chart};
use crate::exc::Result;

/// Chart builder for fluent API
pub struct ChartBuilder {
    title: String,
    chart_type: ChartType,
    categories: Vec<String>,
    series: Vec<ChartSeries>,
    placement: ElementPlacement,
    legend: Option<LegendConfig>,
    value_axis: Option<AxisConfig>,
    category_axis: Option<AxisConfig>,
    data_labels: Option<DataLabelsConfig>,
}

impl ChartBuilder {
    /// Create a new chart builder
    pub fn new(title: &str, chart_type: ChartType) -> Self {
        ChartBuilder {
            title: title.to_string(),
            chart_type,
            categories: Vec::new(),
            series: Vec::new(),
            placement: ElementPlacement::chart_defaults(),
            legend: None,
            value_axis: None,
            category_axis: None,
            data_labels: None,
        }
    }

    /// Set chart position
    pub fn position(mut self, x: u32, y: u32) -> Self {
        self.placement.set_position(x, y);
        self
    }

    /// Set chart size
    pub fn size(mut self, width: u32, height: u32) -> Self {
        self.placement.set_size(width, height);
        self
    }

    /// Add categories
    pub fn categories(mut self, categories: Vec<&str>) -> Self {
        self.categories = categories.into_iter().map(|c| c.to_string()).collect();
        self
    }

    /// Add a data series
    pub fn add_series(mut self, series: ChartSeries) -> Self {
        self.series.push(series);
        self
    }

    /// Configure the legend. Pass [`LegendConfig::hidden()`] to remove the
    /// legend entirely; pass [`LegendConfig::shown()`] (or any other config)
    /// to show it. Defaults to "shown at right" — same as the pre-existing
    /// emission so calling `.build()` without `.legend()` preserves
    /// backwards-compatible behavior.
    pub fn legend(mut self, legend: LegendConfig) -> Self {
        self.legend = Some(legend);
        self
    }

    /// Configure the value axis (the one that holds the `y` values). Pass an
    /// [`AxisConfig::default()`] to use all auto-scaled defaults.
    pub fn value_axis(mut self, axis: AxisConfig) -> Self {
        self.value_axis = Some(axis);
        self
    }

    /// Configure the category axis (the one that holds the `x` categories).
    pub fn category_axis(mut self, axis: AxisConfig) -> Self {
        self.category_axis = Some(axis);
        self
    }

    /// Configure per-plot data labels. Defaults to "all off" — the
    /// generated chart shows no inline labels.
    pub fn data_labels(mut self, labels: DataLabelsConfig) -> Self {
        self.data_labels = Some(labels);
        self
    }

    /// Build the chart, validating that categories and series are non-empty
    /// and that every series has exactly as many values as there are categories.
    pub fn build(self) -> Result<Chart> {
        let series_names: Vec<&str> = self.series.iter().map(|s| s.name.as_str()).collect();
        let series_lengths: Vec<usize> = self.series.iter().map(|s| s.values.len()).collect();
        validate_chart(&self.categories, &series_names, &series_lengths)?;
        Ok(Chart {
            title: self.title,
            chart_type: self.chart_type,
            categories: self.categories,
            series: self.series,
            x: self.placement.x,
            y: self.placement.y,
            width: self.placement.width,
            height: self.placement.height,
            legend: self.legend.unwrap_or_else(LegendConfig::shown),
            value_axis: self.value_axis.unwrap_or_default(),
            category_axis: self.category_axis.unwrap_or_default(),
            data_labels: self.data_labels.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::charts::data::{LabelPosition, LegendPosition};

    #[test]
    fn test_chart_builder() {
        let chart = ChartBuilder::new("Revenue", ChartType::Bar)
            .categories(vec!["Q1", "Q2", "Q3"])
            .add_series(ChartSeries::new("2023", vec![100.0, 150.0, 200.0]))
            .add_series(ChartSeries::new("2024", vec![120.0, 180.0, 220.0]))
            .position(100000, 200000)
            .size(4000000, 3000000)
            .build()
            .expect("valid chart");

        assert_eq!(chart.title, "Revenue");
        assert_eq!(chart.chart_type, ChartType::Bar);
        assert_eq!(chart.category_count(), 3);
        assert_eq!(chart.series_count(), 2);
        assert_eq!(chart.x, 100000);
        assert_eq!(chart.y, 200000);
        // Default legend shown at right.
        assert!(chart.legend.show);
        assert_eq!(chart.legend.position, LegendPosition::Right);
    }

    #[test]
    fn test_chart_builder_rejects_empty_categories() {
        let res = ChartBuilder::new("Revenue", ChartType::Bar)
            .add_series(ChartSeries::new("2023", vec![]))
            .build();
        assert!(res.is_err());
    }

    #[test]
    fn test_chart_builder_rejects_no_series() {
        let res = ChartBuilder::new("Revenue", ChartType::Bar)
            .categories(vec!["Q1", "Q2"])
            .build();
        assert!(res.is_err());
    }

    #[test]
    fn test_chart_builder_rejects_length_mismatch() {
        let res = ChartBuilder::new("Revenue", ChartType::Bar)
            .categories(vec!["Q1", "Q2", "Q3"])
            .add_series(ChartSeries::new("2023", vec![100.0, 150.0, 200.0]))
            .add_series(ChartSeries::new("2024", vec![120.0, 180.0]))
            .build();
        assert!(res.is_err());
        let msg = format!("{}", res.unwrap_err());
        assert!(msg.contains("'2024'"));
    }

    #[test]
    fn test_chart_builder_legend_hidden() {
        let chart = ChartBuilder::new("Revenue", ChartType::Bar)
            .categories(vec!["Q1", "Q2"])
            .add_series(ChartSeries::new("S", vec![1.0, 2.0]))
            .legend(LegendConfig::hidden())
            .build()
            .unwrap();
        assert!(!chart.legend.show);
    }

    #[test]
    fn test_chart_builder_axis_min_max() {
        let chart = ChartBuilder::new("Revenue", ChartType::Bar)
            .categories(vec!["Q1", "Q2"])
            .add_series(ChartSeries::new("S", vec![1.0, 2.0]))
            .value_axis(
                AxisConfig::default()
                    .with_min(0.0)
                    .with_max(100.0)
                    .with_major_unit(10.0)
                    .with_log_scale(10),
            )
            .build()
            .unwrap();
        assert_eq!(chart.value_axis.min, Some(0.0));
        assert_eq!(chart.value_axis.max, Some(100.0));
        assert_eq!(chart.value_axis.major_unit, Some(10.0));
        assert!(chart.value_axis.log_scale);
    }

    #[test]
    fn test_chart_builder_data_labels_preset() {
        let chart = ChartBuilder::new("Revenue", ChartType::Bar)
            .categories(vec!["Q1"])
            .add_series(ChartSeries::new("S", vec![10.0]))
            .data_labels(DataLabelsConfig::show_values())
            .build()
            .unwrap();
        assert!(chart.data_labels.show_value);
        assert!(!chart.data_labels.show_percentage);
        assert_eq!(chart.data_labels.position, LabelPosition::InsideEnd);
    }
}
