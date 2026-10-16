# Public synthetic corpus provenance

Every file below is generated from repository-owned raw OOXML by
[`scripts/gen_public_corpus.py`](../../scripts/gen_public_corpus.py). The generator
uses fixed ZIP member order and timestamps. No third-party or private document
content is included.

| Path | Purpose |
|---|---|
| `synthetic/kitchen_sink.docx` | Package-preservation coverage across revisions, controls, notes, comments, headers, tables, and raster media. |
| `synthetic/comments.docx` | Comment ranges and bodies. |
| `synthetic/revisions.docx` | Accepted-current tracked-change handling. |
| `synthetic/fields.docx` | Supported and cached field reporting. |
| `synthetic/hyperlinks.docx` | Relationship-backed external hyperlink handling. |
| `synthetic/nested_tables.docx` | Nested table parsing and preservation. |
| `synthetic/unsupported_objects.docx` | Deterministic warnings for preserved charts, OLE, shapes, and metafiles. |
| `synthetic/floating_altcontent_anchor.docx` | AlternateContent choice selection for a floating text shape. |
| `synthetic/floating_z_order_pair.docx` | Relative-height and behind-document floating-shape ordering. |
| `synthetic/floating_wrap_policy.docx` | Tight-wrap metadata and polygon preservation. |
| `synthetic/floating_text_bearing.docx` | Text-bearing floating shape recovery. |
| `synthetic/style-hidden-tabs-table.docx` | Run paint, case transforms, vertical alignment, hidden text, paragraph geometry, explicit tabs, cell paint/margins/alignment, and visual RTL table order. |
| `synthetic/pagination-keep.docx` | Direct and style-inherited keep/widow controls on bounded page geometry. |
| `synthetic/two-columns.docx` | Equal-width section flow across column and page boundaries. |
| `synthetic/rtl-table.docx` | Mixed Arabic/Hebrew paragraph direction, run isolation, and RTL table cells. |
| `synthetic/table-cell-lists.docx` | Numbered and bullet paragraphs across body, direct cells, and a recursively nested cell in source-logical visual-RTL table order. |
| `synthetic/wrap-top-bottom.docx` | One bounded top-and-bottom floating-shape exclusion band. |
