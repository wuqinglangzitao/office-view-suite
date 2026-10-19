# Corpus attribution & licenses

## synthetic/

Generated from scratch by `scripts/gen_public_corpus.py` in this repository. No third-party
content; these files are part of rwml and covered by the repository's own license.

| File | Features exercised |
|------|--------------------|
| `kitchen_sink.docx` | tracked changes (`w:ins`/`w:del`), content control (`w:sdt`), text box (`mc:AlternateContent`/`w:txbxContent`), footnote, comment, table, inline PNG, header/footer |
| `comments.docx` | two comments with body ranges and comment-reference markers |
| `revisions.docx` | insertion, deletion, move-from, move-to, and paragraph-property tracked-change markers |
| `fields.docx` | simple and complex fields: `PAGE`, `TOC`, `REF`, `HYPERLINK`, unknown `CUSTOM`, and `FILENAME` |
| `hyperlinks.docx` | relationship-backed `w:hyperlink` with an external target |
| `unsupported_objects.docx` | floating shape, alternate content, chart payload/reference, OLE object marker, WMF/EMF media |
| `floating_altcontent_anchor.docx` | floating `wp:anchor` inside `mc:AlternateContent`, with fallback branch present to assert single-branch recovery |
| `floating_z_order_pair.docx` | two floating anchors with `behindDoc` and `relativeHeight` metadata |
| `floating_wrap_policy.docx` | floating anchor and wrap distances plus `wrapTight` polygon metadata |
| `floating_text_bearing.docx` | text-bearing floating shape with containing-block anchor text |
| `table-cell-lists.docx` | numbering start values across body, direct table cells, and a recursively nested cell; bullet fallback; bidi paragraphs; visual RTL table order |

## benchmark/ - generated legacy fixtures

The `.doc` files are generated artifacts derived only from this repository's
synthetic inputs and are covered by the repository license. They were exported
with LibreOffice 26.2.3.2 using the `MS Word 97` filter on 2026-07-15. The
`sample-poi` text files were produced by Apache POI 5.2.3
`WordExtractor.getText()`; the `sample-lo` text files were produced by
LibreOffice's `Text` filter.

| Legacy fixture | Repository-owned source |
|------|-------|
| `floating_text_bearing.doc` | `synthetic/floating_text_bearing.docx` |
| `floating_wrap_policy.doc` | `synthetic/floating_wrap_policy.docx` |
| `nested_tables.doc` | `synthetic/nested_tables.docx` |

## vendored/

Real-producer files copied verbatim from permissively-licensed upstreams. Only **MIT** /
**CC0** sources are vendored; each file's origin and license is listed below.
The expected report baselines for these files are included in `MANIFEST.tsv`.

### `vendored/python-docx/` — MIT

From [python-openxml/python-docx](https://github.com/python-openxml/python-docx) `tests/test_files/`
(MIT License, © python-docx contributors). Real Word-produced `.docx` exercising common structures.

| File | Notes |
|------|-------|
| `test.docx` | general document (paragraphs, styles, sections) |
| `having-images.docx` | inline images |
| `blk-inner-content.docx` | block-level content (tables, etc.) |
| `sct-inner-content.docx` | section content |

> openpreserve/format-corpus (CC0) was evaluated but its office corpus contains no `.docx`
> (legacy `.doc`/`.odt` only), so nothing was taken from it.
>
> Excluded by policy: GovDocs1 (redistribution license unconfirmed; almost no `.docx`), the
> maintainer's private real-world corpus (not redistributable), and Apache-POI test files copied
> verbatim (per-file provenance not individually cleared). The legacy binary `.doc` reader is
> validated only against that private corpus — see the scope note in [README.md](README.md).
