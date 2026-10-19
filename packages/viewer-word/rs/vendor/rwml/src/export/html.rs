//! `.doc` → semantic HTML exporter.
//!
//! Emits a fragment (no `<html>`/`<head>` wrapper) so it composes into previews:
//! `<h1>`–`<h6>`, `<strong>`/`<em>`/`<u>`/`<s>`, `<a href>`, nested `<ol>`/`<ul>`,
//! and `<table>` with `colspan`/`rowspan`. Hidden (`fVanish`) runs are omitted.

use crate::model::{Align, Block, Cell, DocModel, FieldRole, Paragraph, Run, Table};

pub(crate) fn render(doc: &DocModel) -> String {
    let mut out = String::new();
    let mut stack: Vec<bool> = Vec::new(); // ordered flag per open list level
    for block in &doc.blocks {
        if matches!(block, Block::Paragraph(p) if p.props.page_break_before) && !out.is_empty() {
            close_lists(&mut out, &mut stack);
            out.push_str("<hr class=\"page-break\">");
        }
        match block {
            Block::Paragraph(p) if p.props.list.is_some() => {
                let list = p.props.list.as_ref().expect("list checked");
                adjust_lists(&mut out, &mut stack, list.level as usize, list.ordered);
                out.push_str("<li>");
                out.push_str(&render_runs(&p.runs));
                out.push_str("</li>");
            }
            other => {
                close_lists(&mut out, &mut stack);
                out.push_str(&render_block(other));
            }
        }
    }
    close_lists(&mut out, &mut stack);
    out
}

fn adjust_lists(out: &mut String, stack: &mut Vec<bool>, level: usize, ordered: bool) {
    while stack.len() > level + 1 {
        let o = stack.pop().unwrap_or(false);
        out.push_str(if o { "</ol>" } else { "</ul>" });
    }
    while stack.len() < level + 1 {
        stack.push(ordered);
        out.push_str(if ordered { "<ol>" } else { "<ul>" });
    }
    if stack.last().copied() != Some(ordered) {
        let o = stack.pop().unwrap_or(false);
        out.push_str(if o { "</ol>" } else { "</ul>" });
        stack.push(ordered);
        out.push_str(if ordered { "<ol>" } else { "<ul>" });
    }
}

fn close_lists(out: &mut String, stack: &mut Vec<bool>) {
    while let Some(o) = stack.pop() {
        out.push_str(if o { "</ol>" } else { "</ul>" });
    }
}

fn render_block(block: &Block) -> String {
    match block {
        Block::Paragraph(p) => render_paragraph(p),
        Block::Table(t) => render_table_fragment(t),
        Block::Image(img) => format!("<img alt=\"{}\">", escaped_image_alt(img)),
        Block::Chart(chart) => format!(
            "<figure class=\"chart\" aria-label=\"{}\"></figure>",
            escape_attr(
                chart
                    .alt
                    .as_deref()
                    .or(chart.title.as_deref())
                    .unwrap_or("chart")
            )
        ),
        Block::PageBreak => "<hr class=\"page-break\">".to_string(),
        Block::SectionBreak(_) => "<hr class=\"section-break\">".to_string(),
    }
}

fn render_paragraph(p: &Paragraph) -> String {
    let inline = render_runs(&p.runs);
    let style = align_style(p.props.align);
    if let Some(level) = p.props.heading_level {
        let n = level.clamp(1, 6);
        return format!("<h{n}{style}>{inline}</h{n}>");
    }
    format!("<p{style}>{inline}</p>")
}

fn align_style(align: Align) -> &'static str {
    match align {
        Align::Left => "",
        Align::Center => " style=\"text-align:center\"",
        Align::Right => " style=\"text-align:right\"",
        Align::Justify => " style=\"text-align:justify\"",
    }
}

fn render_runs(runs: &[Run]) -> String {
    let mut out = String::new();
    for run in runs {
        if let Some(img) = &run.image {
            let alt = escaped_image_alt(img);
            match crate::image::data_uri(img) {
                Some(src) => out.push_str(&format!("<img src=\"{src}\" alt=\"{alt}\">")),
                None => out.push_str(&format!("<img alt=\"{alt}\">")),
            }
            continue;
        }
        if run.props.hidden {
            continue;
        }
        let mut t = escape(&run.text);
        if t.is_empty() {
            continue;
        }
        if run.props.strike {
            t = format!("<s>{t}</s>");
        }
        if run.props.underline {
            t = format!("<u>{t}</u>");
        }
        if run.props.italic {
            t = format!("<em>{t}</em>");
        }
        if run.props.bold {
            t = format!("<strong>{t}</strong>");
        }
        match &run.field {
            FieldRole::Hyperlink { url } => {
                out.push_str(&format!("<a href=\"{}\">{t}</a>", escape_attr(url)))
            }
            _ => out.push_str(&t),
        }
    }
    out
}

fn escaped_image_alt(image: &crate::model::Image) -> String {
    escape_attr(image.alt.as_deref().unwrap_or("image"))
}

/// Render a table as an HTML `<table>` fragment. Used both by the HTML exporter
/// and as the Markdown fallback for tables GFM cannot express.
pub(crate) fn render_table_fragment(t: &Table) -> String {
    let mut out = String::from("<table>");
    for (i, row) in t.rows.iter().enumerate() {
        let header = i < t.header_rows;
        if i == 0 && t.header_rows > 0 {
            out.push_str("<thead>");
        }
        if i == t.header_rows {
            out.push_str(if t.header_rows > 0 {
                "</thead><tbody>"
            } else {
                "<tbody>"
            });
        }
        out.push_str("<tr>");
        for cell in &row.cells {
            out.push_str(&render_cell(cell, header || cell.is_header));
        }
        out.push_str("</tr>");
    }
    if t.rows.len() <= t.header_rows && t.header_rows > 0 {
        out.push_str("</thead>");
    } else if !t.rows.is_empty() {
        out.push_str("</tbody>");
    }
    out.push_str("</table>");
    out
}

fn render_cell(cell: &Cell, header: bool) -> String {
    let tag = if header { "th" } else { "td" };
    let mut attrs = String::new();
    if cell.col_span > 1 {
        attrs.push_str(&format!(" colspan=\"{}\"", cell.col_span));
    }
    if cell.row_span > 1 {
        attrs.push_str(&format!(" rowspan=\"{}\"", cell.row_span));
    }
    let inner: String = cell.blocks.iter().map(render_block).collect();
    format!("<{tag}{attrs}>{inner}</{tag}>")
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_attr(s: &str) -> String {
    escape(s).replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::*;

    fn run(text: &str) -> Run {
        Run {
            text: text.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn nested_lists() {
        let item = |text: &str, level: u8, ordered: bool| {
            Block::Paragraph(Paragraph {
                props: ParaProps {
                    list: Some(ListInfo {
                        level,
                        ordered,
                        label: String::new(),
                        marker_font: None,
                    }),
                    ..Default::default()
                },
                runs: vec![run(text)],
            })
        };
        let doc = DocModel {
            blocks: vec![item("a", 0, false), item("b", 1, true), item("c", 0, false)],
            meta: DocMeta::default(),
            ..Default::default()
        };
        assert_eq!(
            render(&doc),
            "<ul><li>a</li><ol><li>b</li></ol><li>c</li></ul>"
        );
    }

    #[test]
    fn table_with_colspan_and_escaping() {
        let mut wide = Cell {
            blocks: vec![Block::Paragraph(Paragraph {
                props: ParaProps::default(),
                runs: vec![run("a<b>")],
            })],
            ..Default::default()
        };
        wide.col_span = 2;
        let t = Table {
            rows: vec![Row { cells: vec![wide] }],
            header_rows: 0,
            ..Default::default()
        };
        assert_eq!(
            render_table_fragment(&t),
            "<table><tbody><tr><td colspan=\"2\"><p>a&lt;b&gt;</p></td></tr></tbody></table>"
        );
    }

    #[test]
    fn paragraph_page_break_before_emits_page_break_marker() {
        let doc = DocModel {
            blocks: vec![
                Block::Paragraph(Paragraph {
                    props: ParaProps::default(),
                    runs: vec![run("Cover")],
                }),
                Block::Paragraph(Paragraph {
                    props: ParaProps {
                        page_break_before: true,
                        ..Default::default()
                    },
                    runs: vec![run("Detail")],
                }),
            ],
            meta: DocMeta::default(),
            ..Default::default()
        };
        assert_eq!(
            render(&doc),
            "<p>Cover</p><hr class=\"page-break\"><p>Detail</p>"
        );
    }

    #[test]
    fn block_and_inline_images_use_escaped_alt_text() {
        let doc = DocModel {
            blocks: vec![
                Block::Image(Image {
                    alt: Some("Block \"chart\"".to_string()),
                    ..Image::default()
                }),
                Block::Paragraph(Paragraph {
                    runs: vec![Run {
                        image: Some(Image {
                            alt: Some("Inline <icon>".to_string()),
                            ..Image::default()
                        }),
                        ..Run::default()
                    }],
                    ..Paragraph::default()
                }),
            ],
            ..DocModel::default()
        };

        assert_eq!(
            render(&doc),
            "<img alt=\"Block &quot;chart&quot;\"><p><img alt=\"Inline &lt;icon&gt;\"></p>"
        );
    }
}
