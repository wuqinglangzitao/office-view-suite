//! Parse PowerPoint comment parts (`commentAuthors.xml`, `commentN.xml`).

use crate::generator::{Comment, CommentAuthor, CommentAuthorList, SlideComments};
use crate::oxml::xmlchemy::XmlParser;

/// Parse `ppt/commentAuthors.xml` into a [`CommentAuthorList`].
pub fn parse_comment_authors_xml(xml: &str) -> CommentAuthorList {
    let mut list = CommentAuthorList::new();
    let Ok(root) = XmlParser::parse_str(xml) else {
        return list;
    };
    for author in root.find_all_descendants("cmAuthor") {
        let id = author.attr("id").and_then(|v| v.parse().ok()).unwrap_or(0);
        let name = author.attr("name").unwrap_or("").to_string();
        let initials = author.attr("initials").unwrap_or("").to_string();
        let color_index = author
            .attr("clrIdx")
            .and_then(|v| v.parse().ok())
            .unwrap_or(id);
        list.import_author(CommentAuthor {
            id,
            name,
            initials,
            color_index,
        });
    }
    list
}

/// Parse `ppt/comments/commentN.xml` into [`SlideComments`].
pub fn parse_slide_comments_xml(xml: &str) -> SlideComments {
    let mut comments = SlideComments::new();
    let Ok(root) = XmlParser::parse_str(xml) else {
        return comments;
    };
    for cm in root.find_all_descendants("cm") {
        let author_id = cm
            .attr("authorId")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let date = cm.attr("dt").unwrap_or("").to_string();
        let index = cm.attr("idx").and_then(|v| v.parse().ok()).unwrap_or(1);
        let pos = cm.find_descendant("pos");
        let x = pos
            .and_then(|p| p.attr("x"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let y = pos
            .and_then(|p| p.attr("y"))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let text = cm
            .find_descendant("text")
            .map(|t| t.text_content())
            .unwrap_or_default();
        comments.add(
            Comment::new(author_id, &text)
                .position(x, y)
                .index(index)
                .date(&date),
        );
    }
    comments
}

/// Resolve an OPC relationship target relative to a base part path.
pub fn normalize_opc_path(base_part: &str, target: &str) -> String {
    if let Some(stripped) = target.strip_prefix('/') {
        return stripped.to_string();
    }
    let base_dir = base_part.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
    let combined = if base_dir.is_empty() {
        target.to_string()
    } else {
        format!("{base_dir}/{target}")
    };
    let mut stack: Vec<&str> = Vec::new();
    for segment in combined.split('/') {
        match segment {
            ".." => {
                stack.pop();
            }
            "." | "" => {}
            other => stack.push(other),
        }
    }
    stack.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_comment_authors_from_xml() {
        let xml = r#"<?xml version="1.0"?>
<p:cmAuthorLst xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cmAuthor id="0" name="Alice Reviewer" initials="AR" clrIdx="0"/>
  <p:cmAuthor id="1" name="Bob Author" initials="BA" clrIdx="1"/>
</p:cmAuthorLst>"#;
        let authors = parse_comment_authors_xml(xml);
        assert_eq!(authors.len(), 2);
        assert_eq!(authors.get_by_id(0).unwrap().name, "Alice Reviewer");
    }

    #[test]
    fn parse_slide_comments_from_xml() {
        let xml = r#"<?xml version="1.0"?>
<p:cmLst xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cm authorId="0" dt="2025-01-01T00:00:00.000" idx="1">
    <p:pos x="100" y="200"/>
    <p:text>Fix wording</p:text>
  </p:cm>
</p:cmLst>"#;
        let comments = parse_slide_comments_xml(xml);
        assert_eq!(comments.len(), 1);
        let comment = &comments.comments()[0];
        assert_eq!(comment.text, "Fix wording");
        assert_eq!(comment.x, 100);
        assert_eq!(comment.y, 200);
    }

    #[test]
    fn normalize_opc_path_resolves_parent_segments() {
        assert_eq!(
            normalize_opc_path("ppt/slides/slide1.xml", "../comments/comment1.xml"),
            "ppt/comments/comment1.xml"
        );
    }
}
