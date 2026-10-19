//! Chart XML generation

use super::data::{AxisConfig, Chart, DataLabelsConfig};
use super::escape_xml;
use super::types::ChartType;

/// Generate chart XML content (for ppt/charts/chartN.xml)
pub fn generate_chart_part_xml(chart: &Chart) -> String {
    match chart.chart_type {
        ChartType::Bar
        | ChartType::BarHorizontal
        | ChartType::BarStacked
        | ChartType::BarStacked100 => generate_bar_chart_xml(chart),
        ChartType::Line | ChartType::LineMarkers | ChartType::LineStacked => {
            generate_line_chart_xml(chart)
        }
        ChartType::Pie => generate_pie_chart_xml(chart),
        ChartType::Doughnut => generate_doughnut_chart_xml(chart),
        ChartType::Area | ChartType::AreaStacked | ChartType::AreaStacked100 => {
            generate_area_chart_xml(chart)
        }
        ChartType::Scatter | ChartType::ScatterLines | ChartType::ScatterSmooth => {
            generate_scatter_chart_xml(chart)
        }
        ChartType::Bubble => generate_bubble_chart_xml(chart),
        ChartType::Radar | ChartType::RadarFilled => generate_radar_chart_xml(chart),
        ChartType::StockHLC | ChartType::StockOHLC => generate_stock_chart_xml(chart),
        ChartType::Combo => generate_combo_chart_xml(chart),
    }
}

/// Generate chart reference XML for slide (p:graphicFrame)
pub fn generate_chart_ref_xml(chart: &Chart, r_id: &str, shape_id: usize) -> String {
    format!(
        r#"<p:graphicFrame>
<p:nvGraphicFramePr>
<p:cNvPr id="{}" name="Chart {}"/>
<p:cNvGraphicFramePr/>
<p:nvPr/>
</p:nvGraphicFramePr>
<p:xfrm>
<a:off x="{}" y="{}"/>
<a:ext cx="{}" cy="{}"/>
</p:xfrm>
<a:graphic>
<a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart">
<c:chart xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:id="{}"/>
</a:graphicData>
</a:graphic>
</p:graphicFrame>"#,
        shape_id, shape_id, chart.x, chart.y, chart.width, chart.height, r_id
    )
}

/// Generate the chart part header
fn chart_part_header(chart: &Chart) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
<c:date1904 val="0"/>
<c:lang val="en-US"/>
<c:roundedCorners val="0"/>
<c:chart>
<c:title>
<c:tx>
<c:rich>
<a:bodyPr/>
<a:lstStyle/>
<a:p>
<a:pPr>
<a:defRPr/>
</a:pPr>
<a:r>
<a:rPr lang="en-US" sz="1800" b="0" i="0" u="none" strike="noStrike">
<a:solidFill>
<a:srgbClr val="595959"/>
</a:solidFill>
<a:latin typeface="Calibri"/>
</a:rPr>
<a:t>{}</a:t>
</a:r>
</a:p>
</c:rich>
</c:tx>
<c:layout/>
<c:overlay val="0"/>
</c:title>
<c:autoTitleDeleted val="0"/>
<c:plotArea>
<c:layout/>"#,
        escape_xml(&chart.title)
    )
}

/// Generate the chart part footer.
///
/// `legend` is the configured legend; when `legend.show == false` we emit no
/// `<c:legend>` block at all (PowerPoint then falls back to layout/master
/// defaults). `data_labels` controls the plot-level data labels that apply
/// to every series unless overridden per-series.
fn chart_part_footer(chart: &Chart) -> String {
    let legend = if chart.legend.show {
        format!(
            r#"
<c:legend>
<c:legendPos val="{}"/>
<c:layout/>
<c:overlay val="0"/>
</c:legend>"#,
            chart.legend.position.as_str()
        )
    } else {
        String::new()
    };

    // Plot-level data labels. Only emit when at least one flag is on.
    let plot_dlbls = generate_data_labels_block(&chart.data_labels);

    format!(
        r#"</c:plotArea>{legend}{plot_dlbls}
<c:plotVisOnly val="1"/>
<c:dispBlanksAs val="gap"/>
<c:showDLblsOverMax val="0"/>
</c:chart>
<c:externalData r:id="rId1">
<c:autoUpdate val="0"/>
</c:externalData>
</c:chartSpace>"#,
    )
}

/// Generate the `<c:dLbls>` block for plot-level (chart-wide) data labels.
///
/// Returns the empty string when all flags are off — the chart then hides
/// labels by default (matching the previous emission). When any flag is on,
/// emits a `<c:dLbls>` block with the requested fields plus `<c:dLblPos>`.
fn generate_data_labels_block(cfg: &DataLabelsConfig) -> String {
    if !cfg.show_value
        && !cfg.show_category_name
        && !cfg.show_percentage
        && !cfg.show_series_name
    {
        return String::new();
    }
    format!(
        r#"
<c:dLbls>
<c:spPr><a:noFill/><a:ln><a:noFill/></a:ln></c:spPr>
<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr sz="900" b="0" i="0" u="none" strike="noStrike" baseline="0"><a:solidFill><a:srgbClr val="595959"/></a:solidFill><a:latin typeface="Calibri"/></a:defRPr></a:pPr><a:endParaRPr lang="en-US"/></a:p></c:txPr>
<c:dLblPos val="{pos}"/>
<c:showLegendKey val="0"/>
<c:showVal val="{v}"/>
<c:showCatName val="{c}"/>
<c:showSerName val="{s}"/>
<c:showPercent val="{p}"/>
<c:showBubbleSize val="0"/>
</c:dLbls>"#,
        pos = cfg.position.as_str(),
        v = bool_to_flag(cfg.show_value),
        c = bool_to_flag(cfg.show_category_name),
        s = bool_to_flag(cfg.show_series_name),
        p = bool_to_flag(cfg.show_percentage),
    )
}

fn bool_to_flag(b: bool) -> &'static str {
    if b { "1" } else { "0" }
}

/// Build the `<c:scaling>` block for a configured axis.
///
/// Auto-scaled axes (no min/max/unit) get plain `<c:orientation val="minMax"/>`.
/// Forced min/max emit `<c:min>` / `<c:max>` with the user's values. `logScale`
/// emits `<c:logScale base="10"/>` (OOXML default base).
fn scaling_block(cfg: &AxisConfig) -> String {
    let mut s = String::from("<c:scaling>\n<c:orientation val=\"minMax\"/>");
    if let Some(min) = cfg.min {
        s.push_str(&format!("\n<c:min val=\"{min}\"/>"));
    }
    if let Some(max) = cfg.max {
        s.push_str(&format!("\n<c:max val=\"{max}\"/>"));
    }
    if cfg.log_scale {
        s.push_str("\n<c:logScale base=\"10\"/>");
    }
    s.push_str("\n</c:scaling>");
    s
}

/// Build the unit config block — the `<c:majorUnit>` / `<c:minorUnit>` elements
/// go after `<c:scaling>` but before `<c:numFmt>` in the OOXML order.
fn unit_block(cfg: &AxisConfig) -> String {
    let mut s = String::new();
    if let Some(major) = cfg.major_unit {
        s.push_str(&format!("\n<c:majorUnit val=\"{major}\"/>"));
    }
    if let Some(minor) = cfg.minor_unit {
        s.push_str(&format!("\n<c:minorUnit val=\"{minor}\"/>"));
    }
    s
}

/// Build the `<c:numFmt>` block — `formatCode` overrides the default `General`.
fn numfmt_block(cfg: &AxisConfig) -> String {
    let fmt = cfg.number_format.as_deref().unwrap_or("General");
    let source_linked = if cfg.number_format.is_some() { "0" } else { "1" };
    format!(
        "\n<c:numFmt formatCode=\"{fmt}\" sourceLinked=\"{source_linked}\"/>"
    )
}

/// Build the `<c:txPr>` block (tick label font).
    ///
    /// Returns the empty string when no font overrides are set — the chart then
    /// inherits the default tick label font from the master.
fn tick_label_txpr_block(cfg: &AxisConfig) -> String {
    if cfg.tick_label_size_pt.is_none() && !cfg.tick_label_bold {
        return String::new();
    }
    let sz = cfg.tick_label_size_pt.unwrap_or(900);
    let b = if cfg.tick_label_bold { "1" } else { "0" };
    format!(
        r#"<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr sz="{sz}" b="{b}" i="0" u="none" strike="noStrike" baseline="0"><a:solidFill><a:srgbClr val="595959"/></a:solidFill><a:latin typeface="Calibri"/></a:defRPr></a:pPr><a:endParaRPr lang="en-US"/></a:p></c:txPr>"#
    )
}

/// Generate series data XML
fn generate_series_data(chart: &Chart, idx: usize, series_name: &str, values: &[f64]) -> String {
    // Per-series data labels: when plot-level config has any flag on, override
    // the per-series defaults (which were always `showVal=0`) so the series
    // emit `<c:dLbls>` matching the user's request.
    let per_series_dlbls = generate_data_labels_block(&chart.data_labels);

    let mut xml = format!(
        r#"
<c:ser>
<c:idx val="{}"/>
<c:order val="{}"/>
<c:title>
<c:tx>
<c:rich>
<a:bodyPr/>
<a:lstStyle/>
<a:p>
<a:r>
<a:rPr lang="en-US" sz="1000"/>
<a:t>{}</a:t>
</a:r>
</a:p>
</c:rich>
</c:tx>
</c:title>{per_series_dlbls}
<c:val>
<c:numRef>
<c:f>Sheet1!$B${}:$B${}</c:f>
<c:numCache>
<c:formatCode>General</c:formatCode>"#,
        idx,
        idx,
        escape_xml(series_name),
        2 + idx,
        2 + idx + values.len()
    );

    for value in values {
        xml.push_str(&format!(
            r#"
<c:pt idx="0">
<c:v>{}</c:v>
</c:pt>"#,
            value
        ));
    }

    xml.push_str(
        r#"
</c:numCache>
</c:numRef>
</c:val>
</c:ser>"#,
    );

    xml
}

/// Generate category axis XML
fn generate_category_axis(chart: &Chart, ax_pos: &str) -> String {
    let cfg = &chart.category_axis;
    // When the user asks for rotated category labels, emit the trio of
    // child elements that LibreOffice / PowerPoint need to actually rotate
    // the labels: `<c:lblAlgn>` (center), `<c:lblOffset>`, `<c:tickLblSkip>`.
    // They live between `<c:crosses>` and the data refs, per OOXML schema.
    let rotation_block = if cfg.tick_label_rotation_deg.is_some() {
        r#"<c:lblAlgn val="ctr"/><c:lblOffset val="100"/><c:tickLblSkip val="1"/>"#
    } else {
        ""
    };
    let mut xml = format!(
        r#"
<c:catAx>
<c:axId val="1"/>{}<c:delete val="0"/>
<c:axPos val="{}"/>{}<c:majorGridlines/>{}<c:tickLblPos val="low"/>{}<c:crossAx val="2"/>
<c:crosses val="autoZero"/>{}
<c:strRef>
<c:f>Sheet1!$A$2:$A${}</c:f>
<c:strCache>
<c:ptCount val="{}"/>"#,
        scaling_block(cfg),
        ax_pos,
        unit_block(cfg),
        numfmt_block(cfg),
        tick_label_txpr_block(cfg),
        rotation_block,
        1 + chart.category_count(),
        chart.category_count()
    );

    for (idx, cat) in chart.categories.iter().enumerate() {
        xml.push_str(&format!(
            r#"
<c:pt idx="{}">
<c:v>{}</c:v>
</c:pt>"#,
            idx,
            escape_xml(cat)
        ));
    }

    xml.push_str(
        r#"
</c:strCache>
</c:strRef>
</c:catAx>"#,
    );

    xml
}

/// Generate value axis XML
fn generate_value_axis(chart: &Chart, ax_pos: &str) -> String {
    let cfg = &chart.value_axis;
    format!(
        r#"
<c:valAx>
<c:axId val="2"/>{}<c:delete val="0"/>
<c:axPos val="{}"/>{}<c:majorGridlines/>{}{}<c:tickLblPos val="low"/>{}<c:crossAx val="1"/>
<c:crosses val="autoZero"/>
</c:valAx>"#,
        scaling_block(cfg),
        ax_pos,
        unit_block(cfg),
        numfmt_block(cfg),
        tick_label_txpr_block(cfg),
        ax_pos
    )
}

/// Generate bar chart XML
fn generate_bar_chart_xml(chart: &Chart) -> String {
    let mut xml = chart_part_header(chart);

    xml.push_str(
        r#"<c:barChart>
<c:barDir val="bar"/>
<c:grouping val="clustered"/>"#,
    );

    for (idx, series) in chart.series.iter().enumerate() {
        xml.push_str(&generate_series_data(
            chart,
            idx,
            &series.name,
            &series.values,
        ));
    }

    xml.push_str(&generate_category_axis(chart, "l"));
    xml.push_str(&generate_value_axis(chart, "b"));
    xml.push_str("</c:barChart>");
    xml.push_str(&chart_part_footer(chart));

    xml
}

/// Generate line chart XML
fn generate_line_chart_xml(chart: &Chart) -> String {
    let mut xml = chart_part_header(chart);

    xml.push_str(
        r#"<c:lineChart>
<c:grouping val="lineMarkers"/>"#,
    );

    for (idx, series) in chart.series.iter().enumerate() {
        xml.push_str(&generate_series_data(
            chart,
            idx,
            &series.name,
            &series.values,
        ));
    }

    xml.push_str(&generate_category_axis(chart, "b"));
    xml.push_str(&generate_value_axis(chart, "l"));
    xml.push_str("</c:lineChart>");
    xml.push_str(&chart_part_footer(chart));

    xml
}

/// Generate pie chart XML
fn generate_pie_chart_xml(chart: &Chart) -> String {
    let mut xml = chart_part_header(chart);

    xml.push_str(
        r#"<c:pieChart>
<c:varyColors val="1"/>"#,
    );

    // Pie chart uses first series only
    if let Some(series) = chart.series.first() {
        xml.push_str(&format!(
            r#"
<c:ser>
<c:idx val="0"/>
<c:order val="0"/>
<c:title>
<c:tx>
<c:rich>
<a:bodyPr/>
<a:lstStyle/>
<a:p>
<a:r>
<a:rPr lang="en-US" sz="1000"/>
<a:t>{}</a:t>
</a:r>
</a:p>
</c:rich>
</c:tx>
</c:title>
<c:dLbls>
<c:showCatName val="1"/>
<c:showPercent val="1"/>
</c:dLbls>
<c:val>
<c:numRef>
<c:f>Sheet1!$B$2:$B${}</c:f>
<c:numCache>
<c:formatCode>General</c:formatCode>"#,
            escape_xml(&series.name),
            1 + series.values.len()
        ));

        for (idx, value) in series.values.iter().enumerate() {
            xml.push_str(&format!(
                r#"
<c:pt idx="{}">
<c:v>{}</c:v>
</c:pt>"#,
                idx, value
            ));
        }

        xml.push_str(&format!(
            r#"
</c:numCache>
</c:numRef>
</c:val>
<c:cat>
<c:strRef>
<c:f>Sheet1!$A$2:$A${}</c:f>
<c:strCache>
<c:ptCount val="{}"/>"#,
            1 + chart.category_count(),
            chart.category_count()
        ));

        for (idx, cat) in chart.categories.iter().enumerate() {
            xml.push_str(&format!(
                r#"
<c:pt idx="{}">
<c:v>{}</c:v>
</c:pt>"#,
                idx,
                escape_xml(cat)
            ));
        }

        xml.push_str(
            r#"
</c:strCache>
</c:strRef>
</c:cat>
</c:ser>"#,
        );
    }

    xml.push_str("</c:pieChart>");
    xml.push_str(&chart_part_footer(chart));

    xml
}

/// Generate doughnut chart XML
fn generate_doughnut_chart_xml(chart: &Chart) -> String {
    let mut xml = chart_part_header(chart);

    xml.push_str(
        r#"<c:doughnutChart>
<c:varyColors val="1"/>
<c:holeSize val="50"/>"#,
    );

    // Doughnut chart uses first series only (like pie)
    if let Some(series) = chart.series.first() {
        xml.push_str(&format!(
            r#"
<c:ser>
<c:idx val="0"/>
<c:order val="0"/>
<c:tx>
<c:strRef>
<c:f>Sheet1!$B$1</c:f>
<c:strCache>
<c:ptCount val="1"/>
<c:pt idx="0"><c:v>{}</c:v></c:pt>
</c:strCache>
</c:strRef>
</c:tx>
<c:dLbls>
<c:showCatName val="1"/>
<c:showPercent val="1"/>
</c:dLbls>
<c:val>
<c:numRef>
<c:f>Sheet1!$B$2:$B${}</c:f>
<c:numCache>
<c:formatCode>General</c:formatCode>"#,
            escape_xml(&series.name),
            1 + series.values.len()
        ));

        for (idx, value) in series.values.iter().enumerate() {
            xml.push_str(&format!(
                r#"
<c:pt idx="{}">
<c:v>{}</c:v>
</c:pt>"#,
                idx, value
            ));
        }

        xml.push_str(&format!(
            r#"
</c:numCache>
</c:numRef>
</c:val>
<c:cat>
<c:strRef>
<c:f>Sheet1!$A$2:$A${}</c:f>
<c:strCache>
<c:ptCount val="{}"/>"#,
            1 + chart.category_count(),
            chart.category_count()
        ));

        for (idx, cat) in chart.categories.iter().enumerate() {
            xml.push_str(&format!(
                r#"
<c:pt idx="{}">
<c:v>{}</c:v>
</c:pt>"#,
                idx,
                escape_xml(cat)
            ));
        }

        xml.push_str(
            r#"
</c:strCache>
</c:strRef>
</c:cat>
</c:ser>"#,
        );
    }

    xml.push_str("</c:doughnutChart>");
    xml.push_str(&chart_part_footer(chart));

    xml
}

/// Generate area chart XML
fn generate_area_chart_xml(chart: &Chart) -> String {
    let mut xml = chart_part_header(chart);

    let grouping = chart.chart_type.grouping().unwrap_or("standard");
    xml.push_str(&format!(
        r#"<c:areaChart>
<c:grouping val="{}"/>"#,
        grouping
    ));

    for (idx, series) in chart.series.iter().enumerate() {
        xml.push_str(&generate_series_data(
            chart,
            idx,
            &series.name,
            &series.values,
        ));
    }

    xml.push_str(&generate_category_axis(chart, "b"));
    xml.push_str(&generate_value_axis(chart, "l"));
    xml.push_str("</c:areaChart>");
    xml.push_str(&chart_part_footer(chart));

    xml
}

/// Generate scatter chart XML
fn generate_scatter_chart_xml(chart: &Chart) -> String {
    let mut xml = chart_part_header(chart);

    let scatter_style = chart.chart_type.scatter_style().unwrap_or("lineMarker");
    xml.push_str(&format!(
        r#"<c:scatterChart>
<c:scatterStyle val="{}"/>"#,
        scatter_style
    ));

    for (idx, series) in chart.series.iter().enumerate() {
        xml.push_str(&format!(
            r#"
<c:ser>
<c:idx val="{}"/>
<c:order val="{}"/>
<c:tx>
<c:strRef>
<c:f>Sheet1!$B$1</c:f>
<c:strCache>
<c:ptCount val="1"/>
<c:pt idx="0"><c:v>{}</c:v></c:pt>
</c:strCache>
</c:strRef>
</c:tx>
<c:xVal>
<c:numRef>
<c:f>Sheet1!$A$2:$A${}</c:f>
<c:numCache>
<c:formatCode>General</c:formatCode>"#,
            idx,
            idx,
            escape_xml(&series.name),
            1 + series.values.len()
        ));

        // X values (use index as X)
        for (i, _) in series.values.iter().enumerate() {
            xml.push_str(&format!(
                r#"
<c:pt idx="{}">
<c:v>{}</c:v>
</c:pt>"#,
                i,
                i + 1
            ));
        }

        xml.push_str(&format!(
            r#"
</c:numCache>
</c:numRef>
</c:xVal>
<c:yVal>
<c:numRef>
<c:f>Sheet1!$B$2:$B${}</c:f>
<c:numCache>
<c:formatCode>General</c:formatCode>"#,
            1 + series.values.len()
        ));

        for (i, value) in series.values.iter().enumerate() {
            xml.push_str(&format!(
                r#"
<c:pt idx="{}">
<c:v>{}</c:v>
</c:pt>"#,
                i, value
            ));
        }

        xml.push_str(
            r#"
</c:numCache>
</c:numRef>
</c:yVal>
</c:ser>"#,
        );
    }

    xml.push_str(&generate_value_axis(chart, "b"));
    xml.push_str(&generate_value_axis(chart, "l"));
    xml.push_str("</c:scatterChart>");
    xml.push_str(&chart_part_footer(chart));

    xml
}

/// Generate bubble chart XML
fn generate_bubble_chart_xml(chart: &Chart) -> String {
    let mut xml = chart_part_header(chart);

    xml.push_str(
        r#"<c:bubbleChart>
<c:varyColors val="0"/>
<c:bubbleScale val="100"/>"#,
    );

    for (idx, series) in chart.series.iter().enumerate() {
        xml.push_str(&format!(
            r#"
<c:ser>
<c:idx val="{}"/>
<c:order val="{}"/>
<c:tx>
<c:strRef>
<c:f>Sheet1!$B$1</c:f>
<c:strCache>
<c:ptCount val="1"/>
<c:pt idx="0"><c:v>{}</c:v></c:pt>
</c:strCache>
</c:strRef>
</c:tx>
<c:xVal>
<c:numRef>
<c:f>Sheet1!$A$2:$A${}</c:f>
<c:numCache>
<c:formatCode>General</c:formatCode>"#,
            idx,
            idx,
            escape_xml(&series.name),
            1 + series.values.len()
        ));

        for (i, _) in series.values.iter().enumerate() {
            xml.push_str(&format!(
                r#"
<c:pt idx="{}">
<c:v>{}</c:v>
</c:pt>"#,
                i,
                i + 1
            ));
        }

        xml.push_str(&format!(
            r#"
</c:numCache>
</c:numRef>
</c:xVal>
<c:yVal>
<c:numRef>
<c:f>Sheet1!$B$2:$B${}</c:f>
<c:numCache>
<c:formatCode>General</c:formatCode>"#,
            1 + series.values.len()
        ));

        for (i, value) in series.values.iter().enumerate() {
            xml.push_str(&format!(
                r#"
<c:pt idx="{}">
<c:v>{}</c:v>
</c:pt>"#,
                i, value
            ));
        }

        xml.push_str(&format!(
            r#"
</c:numCache>
</c:numRef>
</c:yVal>
<c:bubbleSize>
<c:numRef>
<c:f>Sheet1!$C$2:$C${}</c:f>
<c:numCache>
<c:formatCode>General</c:formatCode>"#,
            1 + series.values.len()
        ));

        // Bubble sizes (use values as sizes)
        for (i, value) in series.values.iter().enumerate() {
            xml.push_str(&format!(
                r#"
<c:pt idx="{}">
<c:v>{}</c:v>
</c:pt>"#,
                i,
                value.abs()
            ));
        }

        xml.push_str(
            r#"
</c:numCache>
</c:numRef>
</c:bubbleSize>
</c:ser>"#,
        );
    }

    xml.push_str(&generate_value_axis(chart, "b"));
    xml.push_str(&generate_value_axis(chart, "l"));
    xml.push_str("</c:bubbleChart>");
    xml.push_str(&chart_part_footer(chart));

    xml
}

/// Generate radar chart XML
fn generate_radar_chart_xml(chart: &Chart) -> String {
    let mut xml = chart_part_header(chart);

    let radar_style = chart.chart_type.radar_style().unwrap_or("marker");
    xml.push_str(&format!(
        r#"<c:radarChart>
<c:radarStyle val="{}"/>"#,
        radar_style
    ));

    for (idx, series) in chart.series.iter().enumerate() {
        xml.push_str(&generate_series_data(
            chart,
            idx,
            &series.name,
            &series.values,
        ));
    }

    xml.push_str(&generate_category_axis(chart, "b"));
    xml.push_str(&generate_value_axis(chart, "l"));
    xml.push_str("</c:radarChart>");
    xml.push_str(&chart_part_footer(chart));

    xml
}

/// Generate stock chart XML
fn generate_stock_chart_xml(chart: &Chart) -> String {
    let mut xml = chart_part_header(chart);

    xml.push_str(r#"<c:stockChart>"#);

    // Stock charts need High, Low, Close (and optionally Open) series
    for (idx, series) in chart.series.iter().enumerate() {
        xml.push_str(&generate_series_data(
            chart,
            idx,
            &series.name,
            &series.values,
        ));
    }

    xml.push_str(&generate_category_axis(chart, "b"));
    xml.push_str(&generate_value_axis(chart, "l"));
    xml.push_str("</c:stockChart>");
    xml.push_str(&chart_part_footer(chart));

    xml
}

/// Generate combo chart XML (bar + line)
fn generate_combo_chart_xml(chart: &Chart) -> String {
    let mut xml = chart_part_header(chart);

    // First half of series as bars
    xml.push_str(
        r#"<c:barChart>
<c:barDir val="col"/>
<c:grouping val="clustered"/>"#,
    );

    let mid = chart.series.len() / 2;
    for (idx, series) in chart.series.iter().take(mid.max(1)).enumerate() {
        xml.push_str(&generate_series_data(
            chart,
            idx,
            &series.name,
            &series.values,
        ));
    }

    xml.push_str(&generate_category_axis(chart, "b"));
    xml.push_str(&generate_value_axis(chart, "l"));
    xml.push_str("</c:barChart>");

    // Second half as lines
    if chart.series.len() > 1 {
        xml.push_str(
            r#"<c:lineChart>
<c:grouping val="standard"/>"#,
        );

        for (idx, series) in chart.series.iter().skip(mid.max(1)).enumerate() {
            xml.push_str(&generate_series_data(
                chart,
                mid + idx,
                &series.name,
                &series.values,
            ));
        }

        xml.push_str("</c:lineChart>");
    }

    xml.push_str(&chart_part_footer(chart));

    xml
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::charts::ChartSeries;

    #[test]
    fn test_generate_bar_chart_xml() {
        let chart = Chart::new(
            "Sales",
            ChartType::Bar,
            vec!["Q1".to_string(), "Q2".to_string()],
            0,
            0,
            5000000,
            3750000,
        )
        .add_series(ChartSeries::new("2024", vec![100.0, 150.0]));

        let xml = generate_bar_chart_xml(&chart);
        assert!(xml.contains("barChart"));
        assert!(xml.contains("Sales"));
    }

    #[test]
    fn test_generate_line_chart_xml() {
        let chart = Chart::new(
            "Trend",
            ChartType::Line,
            vec!["Jan".to_string(), "Feb".to_string()],
            0,
            0,
            5000000,
            3750000,
        )
        .add_series(ChartSeries::new("Revenue", vec![1000.0, 1200.0]));

        let xml = generate_line_chart_xml(&chart);
        assert!(xml.contains("lineChart"));
    }

    #[test]
    fn test_generate_pie_chart_xml() {
        let chart = Chart::new(
            "Distribution",
            ChartType::Pie,
            vec!["A".to_string(), "B".to_string()],
            0,
            0,
            5000000,
            3750000,
        )
        .add_series(ChartSeries::new("Data", vec![30.0, 70.0]));

        let xml = generate_pie_chart_xml(&chart);
        assert!(xml.contains("pieChart"));
    }
}
