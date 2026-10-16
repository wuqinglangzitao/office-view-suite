# Changelog

All notable changes to `rwml` are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.4] - 2026-08-29

### Changed

- The `rwml` CLI now automatically registers the deterministic OFL font subsets
  when built with `bundled-fonts`, so `rwml to-pdf` uses the bundled Korean,
  Arabic, and Hebrew coverage without a separate library call.
- The opt-in renderer now uses `parley` 0.11.1, including corrected emoji
  variation-selector font fallback and refreshed Fontations/HarfRust shaping
  dependencies. The render MSRV remains 1.92 because `krilla` 0.8.2 sets the
  current render-stack floor.
- Public edit and legacy-oracle release validation now use authoritative
  recursive manifests and fail closed on empty, missing, or partial evidence.
- Public contribution guidance now uses a right-sized issue/PR workflow, with
  optional BMad planning kept external and all review decisions recorded in the
  public repository.

### Security

- `quick-xml` is upgraded from 0.36.2 to 0.41.0 to address reachable CPU-
  exhaustion (RUSTSEC-2026-0194) and namespace-allocation (RUSTSEC-2026-0195)
  denial-of-service paths, with regression coverage.
- OPC metadata XML is capped at 256 namespace declarations per element and 128
  nested elements. Over-depth metadata remains read-only and byte-preserved on
  no-op saves.

## [0.1.3] - 2026-08-24

### Added

- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty text and `\p` relative-position `STYLEREF` fields
  with one modeled result run in exact real footnote and endnote payloads when
  the nearest prior, otherwise first later, paragraph-style target is in the same
  note and contains exactly one stable, nonempty plain run. Style ids and resolved
  names, supported text formats, normalized instructions, source order, modeled
  field formatting, and recursive table placement survive native reopen and
  deterministic reconversion; fresh output emits a deduplicated minimal source
  style id/name definition when no valid authored or generated definition exists.
  Numbered results, character-style targets, cross-note matches, multi-run or
  field-backed targets, missing or empty targets, malformed, dirty, mismatched-
  result, or split-result cases still trigger note-local plain-text fallback. This
  does not add field updates, relationships, or page/layout-aware lookup.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty `LISTNUM` fields with an explicit nonnegative `\s`
  start and one modeled result run in exact real footnote and endnote payloads.
  Optional `NumberDefault`/`LegalDefault` names, level one, supported number/text
  formats, normalized instructions, deterministic counter assignment, source
  order, and modeled formatting survive at the top level and through recursively
  nested table cells; one-result-run complex source fields normalize to non-dirty
  `w:fldSimple`. Ordinary LISTNUM increments, levels above one, custom list names,
  AUTONUM-family fields, cross-note numbering state, malformed, dirty, mismatched-
  result, or split-result cases still trigger note-local plain-text fallback. This
  does not add field updates, relationships, or layout behavior.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty visible `SEQ` fields with an explicit nonnegative
  `\r` reset and one modeled result run in exact real footnote and endnote
  payloads. Supported number/text formats, normalized instructions,
  deterministic counter assignment, source order, and modeled formatting survive
  at the top level and through recursively nested table cells; one-result-run
  complex source fields normalize to non-dirty `w:fldSimple`. Ordinary increment/
  current fields, heading resets, hidden output, cross-note sequence state,
  malformed, dirty, mismatched-result, or split-result cases still trigger note-
  local plain-text fallback. This does not add field updates, ordinary source-
  order numbering transport, relationships, or layout behavior.
- Opened DOCX models and fresh `Document::to_docx()` conversion now retain all 13
  supported nonempty `docProps/core.xml` values. Subject, description, keywords,
  category, content status, last-modified-by, created/modified/last-printed
  timestamps, revision, and version now join title and creator in `DocSetup` and
  survive deterministic native reopen and reconversion without changing the
  public model or generated-core-property writer contract.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty core-property and custom-string `DOCPROPERTY`
  fields with one modeled result run in exact real footnote and endnote payloads.
  Direct core-property and `INFO` aliases, supported text/date pictures,
  normalized instructions, deterministic computed text, source order, and
  modeled formatting survive at the top level and through recursively nested
  table cells; one-result-run complex source fields normalize to non-dirty
  `w:fldSimple`. Referenced properties must exist, and core values must already
  match the fresh writer's nonempty trim normalization. Extended application
  properties, document variables, `FILESIZE`, volatile date/user fields,
  `REVNUM`, malformed or dirty instructions, and split results still trigger
  note-local plain-text fallback. This does not copy app/settings parts or add
  field updates, user context, relationships, or layout behavior.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty `REVNUM` fields with one modeled result run in
  exact real footnote and endnote payloads when the core revision exists and
  already matches the fresh writer's nonempty trim normalization. Normalized
  instructions, deterministic current results, supported text-format switches,
  source order, and modeled formatting survive at the top level and through
  recursively nested table cells; one-result-run complex source fields normalize
  to non-dirty `w:fldSimple`. Missing, empty, trim-unstable, malformed, dirty, or
  split-result cases still trigger note-local plain-text fallback. This does not
  add revision tracking, field updates, relationships, or layout behavior.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty explicit bookmark-text `REF` fields with one modeled
  result run in exact real footnote and endnote payloads when the target is exactly
  one stable, nonempty, non-field run in the same note. Neutral hyperlink/lock
  switches, supported text and integer number formats, numeric pictures, target
  bookmarks, source order, and modeled formatting survive at the top level and
  through recursively nested table cells; one-result-run complex source fields
  normalize to non-dirty `w:fldSimple`. Note-mark, relative-position, paragraph/
  full/relative-context numbering, sequence-separator, cross-note, missing, empty,
  multi-run, field-backed, malformed, dirty, mismatched-result, or split-result
  cases still trigger note-local plain-text fallback. This does not add global
  bookmark lookup, field updates, relationships, or layout behavior.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  context-free, non-dirty `EQ` and `ADVANCE` fields with one modeled result run in
  exact real footnote and endnote payloads. Supported EQ expressions retain their
  normalized instructions, deterministic plain-text approximations, source order,
  and modeled formatting; parser-valid ADVANCE instructions retain their empty
  current result and formatted result run. Top-level and recursively nested table-
  cell fields survive, and one-result-run complex source fields normalize to non-
  dirty `w:fldSimple`. Malformed or unsupported display syntax, split results, and
  action fields still trigger note-local plain-text fallback. This transports
  instructions and current results without adding native PDF equation layout,
  ADVANCE displacement, action execution, relationships, or field updates.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty `FILLIN` fields with an explicit literal default and
  one modeled result run in exact real footnote and endnote payloads. Quoted and
  bounded multi-token defaults, neutral `\o`, supported text-format switches,
  normalized instructions, deterministic current results, source order, and
  modeled formatting survive at the top level and through recursively nested
  table cells; one-result-run complex source fields normalize to non-dirty
  `w:fldSimple`. Default-less or malformed prompts, split results, and stateful
  `ASK`, `SET`, or merge-control fields still trigger note-local plain-text
  fallback. This transports the current result and instruction without adding
  interactive prompting, field-update, relationship, or layout behavior.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  context-free, non-dirty formula (`=`) fields with one modeled result run in
  exact real footnote and endnote payloads, plus one independently verified
  table-dependent formula per qualifying formula-owning table. Finite literal
  arithmetic, scalar functions, direct/range and positional table references,
  supported numeric pictures, normalized instructions, deterministic computed
  text, source order, and modeled formatting survive; qualifying tables are
  span-free, left-to-right, non-header grids whose cells each contain one stable
  nonempty run, and may themselves be nested under other supported note tables.
  One-result-run complex source fields normalize to non-dirty `w:fldSimple`.
  Bookmark- or `SET`-backed expressions, non-table-local `DEFINED` state, multiple
  or dependent table formulas, spans, RTL/header grids, non-plain operands,
  malformed or nonfinite formulas, mismatched results, and split results still
  trigger note-local plain-text fallback. This does not broaden prompt/control,
  relationship, field-update, or layout behavior.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  context-free, non-dirty `IF` and `COMPARE` fields with one modeled result run
  in exact real footnote and endnote payloads. Quoted-text or finite-number
  operands, compact comparisons, wildcard equality, optional false branches,
  normalized instructions, deterministic computed text, supported text-format
  switches, source order, and modeled formatting survive at the top level and
  through recursively nested table cells; one-result-run complex source fields
  normalize to non-dirty `w:fldSimple`. Bookmark- or `SET`-backed, malformed or
  nonfinite, and split-result comparisons still trigger note-local plain-text
  fallback. This does not broaden prompt, merge-control, relationship, field-
  update, or layout behavior.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty literal `QUOTE` fields with one modeled result run
  in exact real footnote and endnote payloads. Normalized instructions, computed
  text and supported text-format switches, source order, and modeled formatting
  survive at the top level and through recursively nested table cells; one-result-
  run complex source fields normalize to non-dirty `w:fldSimple`, and stale
  cached text stays replaced. Malformed or split-result `QUOTE` fields and other
  dynamic fields still trigger note-local plain-text fallback; this does not
  broaden prompt, control, bookmark, formula, conditional, relationship, or
  field-update behavior.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty `SYMBOL` fields whose instructions deterministically
  compute one character in exact real footnote and endnote payloads with nonempty
  visible text. Normalized instructions, computed characters, source order, and
  modeled formatting survive at the top level and through recursively nested
  table cells; one-result-run complex source fields normalize to non-dirty
  `w:fldSimple`, and stale cached characters stay replaced. Malformed or unmapped
  symbols and other display fields still trigger note-local plain-text fallback;
  this does not broaden `EQ`, `ADVANCE`, action, relationship, or field-update
  behavior.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty `TC` hidden table-of-contents entry markers in
  exact real footnote and endnote payloads with nonempty visible text. Normalized
  instructions, empty computed results, source order, and modeled formatting
  survive at the top level and through recursively nested table cells; one-
  result-run complex source fields normalize to non-dirty `w:fldSimple`. Stale
  marker caches stay hidden, and malformed `TC` or generated `TOC` siblings still
  trigger note-local plain-text fallback. This transports marker instructions
  only: it does not generate or update a TOC, aggregate markers across package
  parts, or cover marker-only notes, comments, tracked revisions, nested notes,
  floating placement, page-bottom composition, or Word-exact pagination.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty `RD`, `TA`, and `XE` hidden marker fields in exact
  real footnote and endnote payloads with nonempty visible text. Normalized
  instructions, empty computed results, source order, and modeled formatting
  survive at the top level and through recursively nested table cells; one-
  result-run complex source fields normalize to non-dirty `w:fldSimple`. Both
  note families, deterministic bytes, source-model immutability, native reopen/
  reconversion, standalone note exclusion, stale-cache removal, and malformed or
  generated-index sibling fallback are covered. This does not generate or update
  indexes, tables of authorities, or master-document references. Marker-only
  notes, `TC`, generated `BIBLIOGRAPHY`/`CITATION`/`INDEX`/`TOA` fields, dirty or
  malformed markers, comments, tracked revisions, nested notes, floating
  placement, page-bottom composition, and Word-exact pagination remain outside
  this bounded path.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty `FILENAME` cached result runs in exact real footnote
  and endnote payloads. Bounded path-display and text-format switches survive
  with normalized instructions and modeled formatting at the top level and
  through recursively nested table cells; one-result-run complex source fields
  normalize to non-dirty `w:fldSimple`. Both note families, deterministic bytes,
  source-model immutability, native reopen/reconversion, standalone note
  exclusion, and malformed/unknown sibling fallback are covered. No filesystem
  path is read and no destination filename is inferred. Source field grouping and
  update state are not preserved; dirty modeled runs, malformed filename syntax,
  unknown and context-sensitive/computed fields, comments, tracked revisions,
  nested notes, floating placement, page-bottom composition, and Word-exact
  pagination remain outside this bounded path.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-validated, non-dirty `MERGEFIELD` cached result runs in exact real
  footnote and endnote payloads. Valid names and bounded before/after and text-
  format switches survive with normalized instructions and modeled formatting at
  the top level and through recursively nested table cells; one-result-run complex
  source fields normalize to non-dirty `w:fldSimple`. Both note families,
  deterministic bytes, source-model immutability, native reopen/reconversion,
  standalone note exclusion, and malformed/unknown sibling fallback are covered.
  No external merge data source is copied or evaluated, and source field grouping
  and update state are not preserved. Dirty modeled runs, malformed merge syntax,
  unknown and context-sensitive/computed fields, comments, tracked revisions,
  nested notes, floating placement, page-bottom composition, and Word-exact
  pagination remain outside this bounded path.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-normalized cached result runs for nonempty compatibility/private,
  inserted-content, mail-merge-helper, and barcode fields in exact real footnote
  and endnote payloads when the reader reports `NoComputedResult`. They serialize
  as non-dirty `w:fldSimple` fields with normalized instructions and modeled run
  formatting at the top level and through recursively nested table cells.
  Complex source fields normalize to the same simple form; source simple/complex
  grouping and source update state are not preserved. Both note families,
  deterministic bytes, source-model immutability, native reopen/reconversion,
  standalone note exclusion, and per-note atomic fallback are covered. Dirty or
  malformed fields, unknown and context-sensitive/computed fields,
  comments, tracked revisions, nested notes, floating placement, page-bottom
  composition, and Word-exact pagination remain outside this bounded path.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-modeled run content controls in exact real footnote and endnote payloads
  at the top level and through recursively nested table cells. Nonempty alias/tag
  metadata and complete XPath/store-item data-binding pairs retain trimmed values,
  formatting, deterministic bytes, source-model immutability, native reopen and
  reconversion, and standalone note exclusion without adding relationships or
  package parts. One-sided bindings and all-empty modeled metadata reject only
  that note before rendering, preserving atomic text fallback. Property-empty
  source wrappers are normalized out by the reader and are not claimed; comments,
  tracked revisions, other fields, nested notes, floating placement, page-bottom
  composition, and Word-exact pagination remain outside this bounded path.
- Fresh DOCX writers now serialize validator-approved `#bookmark` hyperlink
  targets as `w:hyperlink w:anchor` without external relationships, while
  preserving the existing relationship-backed path for external targets.
  Model-authored body links and native reopen are covered. Exact real footnote
  and endnote payloads from opened DOCX inputs now also retain internal anchors
  and paired run bookmarks in top-level paragraphs and recursively nested table
  cells, with globally unique bookmark IDs and no note relationship parts.
  Both note families, body- and note-local targets, deterministic reconversion,
  source-model immutability, standalone note exclusion, malformed-name rejection,
  and per-note atomic fallback without orphan anchor/bookmark XML are covered.
  Malformed model-authored internal targets serialize as visible plain text;
  arbitrary fields/annotations, nested notes, floating placement, page-bottom
  composition, and Word-exact pagination remain outside this bounded path.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  parser-modeled native literal-cache core and ChartEx chart blocks in exact real
  footnote and endnote payloads that also contain visible text. Note-part-local
  chart relationships coexist with globally unique body/note chart and drawing
  IDs; core charts receive regenerated embedded XLSX workbooks and chart
  relationship parts, while ChartEx charts retain native literal data. Both note
  families, top-level and recursively nested table-cell charts, escaped alt text,
  dimensions, titles/data, conditional `c`/`cx` namespaces, deterministic bytes,
  native reopen/reconversion, source-model immutability, standalone model writing,
  and atomic fallback without orphan charts, workbooks, or relationships are
  covered. Chart-only notes, arbitrary Office chart grammars and source part
  bytes, themes/styling, formulas, external data, exact floating placement,
  page-bottom composition, and Word-exact pagination remain outside this bounded
  normalization path.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  complete extracted inline PNG, JPEG, GIF, BMP, TIFF, and WebP runs in exact
  real footnote and endnote payloads. Note-local image relationships coexist with
  optional external hyperlink wrappers, while media names and drawing IDs remain
  globally unique across body and both note parts. Top-level and recursively
  nested table-cell images, escaped alt text, intrinsic dimensions, rotation,
  exact media bytes/content types, deterministic bytes, native reopen/
  reconversion, source-model immutability, standalone model writing, and atomic
  fallback without orphan media or relationships are covered. Missing or empty
  bytes, unknown or raw-RGBA MIME data, floating and block images, image-only
  notes, vector metafiles, page-bottom placement, and Word-exact
  pagination remain outside this bounded path.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now retains
  relationship-backed external hyperlinks in exact real footnote and endnote
  payloads. Each generated note part owns an independent relationship file, IDs
  remain unique across multiple notes in that part, and links survive in ordinary
  top-level paragraphs and recursively nested supported table cells without
  leaking into `document.xml.rels`. Both note families, escaped targets,
  deterministic bytes, native reopen/reconversion, source-model immutability,
  standalone-writer exclusion, and all-or-nothing fallback without orphan
  relationships are covered. Simple HYPERLINK field syntax may normalize to a
  `w:hyperlink` element. Unsupported raster classes, other fields, annotations,
  nested notes, page-bottom placement, and Word-exact pagination remain outside
  this bounded path.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs retains typed
  manual page breaks in direct and recursively nested body and running-surface
  table cells through the existing ordered public block tree. Evidence covers
  two body-table depths plus three running-table depths across all default/first/
  even header and footer variants, default-surface inheritance, local
  relationships, row no-split and cell keep/widow controls, exact/minimum line
  rules, tabs/leaders, manual column breaks, deterministic bytes, native reopen
  against the established normalized model, source-model immutability,
  standalone model writing, and malformed unrelated hint isolation. This is a
  transport guarantee only; preview table fragmentation, page-bottom note
  composition, legacy note breaks, and Word-exact pagination remain unsupported.
- Fresh `Document::to_docx()` conversion of opened legacy DOC inputs now
  preserves visible manual column breaks in ordinary top-level paragraphs across
  all default/first/even headers and footers. A paragraph-aligned source-story
  sidecar maps all six variants per section with first-owner and unindexed-header
  fallback semantics, then reuses the existing section/variant/leaf-validating
  writer. Two sections and all twelve running stories, body projection,
  deterministic bytes, native reopen and reconversion, public-model immutability,
  model-only writer exclusion, and malformed writer hints are covered. Legacy
  manual breaks in running-table cells and nested running tables, legacy note
  breaks, and PDF distinction between running line and column breaks remain
  outside this bounded path.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now preserves
  mixed ordinary paragraphs and supported table trees rooted at top-level tables
  in real footnote and endnote bodies when the exact body-reference
  contract succeeds. Surviving cells may recursively contain ordinary paragraphs
  or further tables under the same constraint. An ID-keyed, run-aligned private
  payload retains block order, modeled table properties and horizontal/vertical
  merges, modeled paragraph and character formatting, row no-split and cell keep/
  widow controls, exact/minimum line rules, explicit tabs and leaders, and visible
  manual column breaks at every table depth. Parsed manual page breaks between
  top-level paragraph fragments and inside table cells at every selected depth
  are retained through the same ordered payload. An unsupported or malformed
  note body falls back independently to one normalized text paragraph while
  supported siblings retain their payload; body anchor projection remains all-
  or-nothing. Both note families, accepted-current paragraph wrappers, nested
  table pagination metadata, page-break/tab-slot alignment, malformed alignment,
  table-component and break-leaf isolation, deterministic bytes, native reopen
  and reconversion, public-model equality, and standalone-writer exclusion are
  covered. Relationship-backed external hyperlink runs are retained with
  note-part-local relationships. Complete encoded inline raster runs use the same
  boundary and retain note-local image relationships. Parser-modeled native
  literal-cache core and ChartEx blocks retain note-local chart relationships and
  fresh chart package parts. Missing/empty, unknown-MIME, raw-RGBA, floating,
  block, or image-only raster cases, chart-only notes, arbitrary Office chart
  grammars, malformed internal-anchor or bookmark names, other fields,
  annotations, nested notes, custom marks, source IDs/numbering and separators,
  page-bottom placement, and Word-exact pagination remain unsupported by this
  bounded path.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now preserves
  visible manual column breaks in ordinary top-level paragraphs across all
  default/first/even headers and footers. The private section-aligned six-variant
  offsets follow default-surface inheritance, retain header/footer-local
  relationships across run wrappers, and validate section, variant, and paragraph
  leaves independently through the existing run-aware writer. Two sections,
  inherited defaults, multiple runs, accepted-current wrappers, a local
  hyperlink, malformed outer/variant/leaf data, coexistence with running-table
  breaks, deterministic bytes, native reopen, and standalone-writer exclusion are
  covered. Legacy running-table cell and nested running-table breaks, note
  paragraph properties and breaks, and PDF distinction between running line and
  column breaks remain outside this bounded conversion path.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now preserves
  visible manual column breaks in direct paragraphs of top-level running-table
  cells and recursively preserves row no-split state, cell-paragraph keep/widow
  controls, exact/minimum line rules, explicit tabs, and visible column breaks in
  nested running-table descendants. A private section-aligned aggregate covers
  all default/first/even headers and footers, follows default-surface inheritance,
  retains header/footer-local relationships, and validates each table slot and
  recursive component independently. Two nested levels, accepted-current
  wrappers, vertical-merge continuation exclusion, all six variants, inherited
  defaults, malformed outer/slot/child data, malformed break leaves,
  deterministic bytes, native reopen, parser depth limits, and standalone-writer
  exclusion are covered. Legacy nested running tables and their manual breaks,
  legacy running-table cell breaks, note paragraph properties, preview
  pagination of typed table-cell page breaks, and PDF consumption of nested
  running-table layout hints remain outside this bounded conversion path.
- Fresh `Document::to_docx()` conversion of opened DOCX inputs now recursively
  preserves source-only row no-split state, direct cell-paragraph keep/widow
  controls, exact/minimum line rules, explicit tab stops, and visible manual
  column breaks in nested descendants of top-level body tables. The existing
  recursive reader tree is retained in default-feature state and each table
  node validates row, cell, block, line, tab, break, and child-tree components
  independently before reusing the established writers. Two-level accepted-
  current wrappers, vertical-merge continuation exclusion, malformed component
  isolation, malformed break leaves, deterministic bytes, native reopen, parser
  depth limits, standalone-writer exclusion, and unchanged running-surface
  behavior are covered. Legacy nested-table recovery, nested running tables,
  notes, preview pagination of typed table-cell page breaks, and other PDF
  behavior remain outside this bounded conversion path.
- Fresh `Document::to_docx()` conversion now preserves visible manual column
  breaks in direct paragraph blocks of surviving cells in top-level body tables
  from opened DOCX and legacy DOC inputs. A private block/row/surviving-cell/
  paragraph tree follows DOCX vertical merges and legacy horizontal/vertical
  merge ownership, validates structure before use, and validates each strictly
  increasing newline-backed offset leaf independently through the existing run-
  aware column-break writer. Multiple cell paragraphs, accepted-current run
  wrappers, hidden controls, malformed tree and leaf isolation, deterministic
  bytes, native reopen, and unchanged public-model/standalone-writer behavior are
  covered. Typed table-cell page-break transport is separately covered above;
  nested tables, notes, running surfaces, and PDF pagination remain outside this
  bounded column-break path.
- Fresh legacy `Document::to_docx()` conversion now excludes a HeaderFooter
  source region from `word/document.xml` only when assembly actually promoted
  that exact nonempty block range into a selected section running slot. The
  DOCX-only projection validates nonoverlapping exact region ranges and every
  block-aligned writer sidecar before applying one shared keep mask, then remaps
  later source regions and statistics. Valid single- and two-section running
  stories no longer duplicate into the body; malformed/excess or second-section
  stories left unselected by first-owner fallback remain flattened instead of
  being dropped, and footnote, annotation, endnote, and text-box fallback content
  retains source order. Public model extraction and standalone model writing are
  unchanged.
- Fresh `Document::to_docx()` conversion now preserves effective `keepNext`,
  `keepLines`, and widow-off state on direct top-level paragraphs and direct
  paragraphs in surviving cells of top-level tables across all six selected
  running header/footer variants, plus effective no-split state for aligned
  top-level table rows, from opened DOCX and legacy DOC inputs. A private
  section-aligned aggregate validates complete block, row, surviving-cell, and
  cell-block shapes before emitting any pagination property; default-surface
  inheritance, malformed aggregate rejection, deterministic bytes, native
  reopen, public-model exclusion, and unchanged standalone model writing are
  covered. Legacy assembly also removes captured running pagination and
  exact/minimum line rules from body-aligned source hints so those properties are
  emitted only in their running parts. Nested running tables, notes, manual
  breaks, and Word-exact pagination remain outside this bounded path.
- Fresh `Document::to_docx()` conversion now promotes normalized legacy `.doc`
  footnotes and endnotes into real references and note parts when nonempty
  `PlcffndRef`/`PlcfendRef` tables, recovered records, and visible Main-story
  markers match one-to-one at ordinary top-level paragraph offsets. The exact
  block/id/offset contract also preserves that normalized subset when the
  generated DOCX is reopened and freshly converted again, without changing the
  public model or standalone writer. Missing, truncated, duplicate, mismatched,
  or unsupported anchors keep the complete historical flattened fallback. Rich
  note formatting, tables, media, source numbering and separators, custom marks,
  table-cell/manual-break anchors, and page-bottom placement remain unsupported.
- Opened legacy `.doc` Footnote and Endnote source stories now apply resolved
  common custom tabs to ordinary top-level paragraphs and direct paragraph
  blocks in surviving cells of top-level note tables during PDF preview. A
  render-only full-model overlay requires exact block alignment and leaves every
  writer-facing note tab slot blank. Footnote/endnote and direct/table isolation,
  exact private sidecars, deterministic and distinct PDF bytes, public-model
  equality, unchanged fresh DOCX bytes, render-without-DOCX compilation, and
  non-note isolation are covered. Nested note tables, fresh note conversion,
  settings-defined legacy default-tab intervals, post-tab field containment,
  page-bottom composition, and Word-exact reflow remain outside this path.
- Opened legacy `.doc` direct top-level paragraphs and direct paragraph blocks
  in surviving cells of top-level tables across all six running header/footer
  variants now retain bounded STSH/PAPX custom tabs in deterministic PDF output
  and fresh DOCX conversion. A short-lived parser sidecar folds each MS-DOC
  story into section-aligned private hints without leaking running properties
  into body blocks. Six-variant and two-section mapping, unindexed default-header
  fallback, first-owner precedence, direct/table isolation, deterministic bytes,
  native DOCX reopen, and unchanged public-model/standalone-writer behavior are
  covered. Legacy notes, nested running tables, settings-defined default-tab
  intervals, implicit list tabs, negative stops, post-tab field containment,
  the separate running-pagination bridge, and Word-exact reflow remain outside
  this tab path.
- Opened legacy `.doc` ordinary Main-story top-level paragraphs and direct
  paragraph blocks in surviving cells of top-level tables now resolve bounded
  custom tabs through paragraph-style STSH inheritance followed by direct PAPX
  `sprmPChgTabsPapx` and `sprmPChgTabs` changes. Common alignments and leaders,
  both deletion forms, source-order replacement, style-reset semantics,
  malformed-input isolation, merge-owner alignment, deterministic PDF output,
  and fresh DOCX conversion/reopen are covered. The public model and standalone
  writer remain unchanged; running stories, notes, nested legacy tables,
  default-tab intervals, implicit list tabs, negative stops, and Word-exact
  reflow remain outside this bounded path.
- Preserves visible manual column breaks when `Document::to_docx()` freshly
  converts aligned top-level body paragraphs from opened DOCX and legacy DOC
  inputs. The source-only bridge validates strict block and character-offset
  alignment, emits only newline-backed positions as `w:br w:type="column"`, and
  keeps each break inside its existing run formatting and semantic wrappers.
  Ordinary and hidden-source line breaks remain ordinary, malformed hints fail
  independently, deterministic bytes and native reopen are covered, and
  standalone model writing remains unchanged. Note, running-surface, and nested-
  content manual breaks remain outside this bounded path; direct table-cell
  column breaks use the separately validated bridge above.
- Preserves reader-resolved explicit paragraph tab stops when
  `Document::to_docx()` freshly converts direct top-level paragraphs in selected
  default/first/even headers and footers from an opened DOCX. A private
  section-aligned six-variant sidecar follows effective default-surface
  inheritance, validates each complete running block stream independently, and
  emits bounded left, center, right, decimal, and bar stops with supported
  leaders through the established paragraph-property formatter. Deterministic
  bytes, native reopen/model equality, nonparagraph slots, local relationships,
  generated page-number fields, and malformed section/variant isolation are
  covered. Standalone model writing remains unchanged; legacy-DOC running
  stories, settings-defined default-tab intervals, and fresh note conversion
  remain outside this bridge.
- Preserves reader-resolved explicit paragraph tab stops when
  `Document::to_docx()` freshly converts direct paragraph blocks in surviving
  cells of top-level tables in selected default/first/even headers and footers
  from an opened DOCX. A section-aligned six-variant sidecar follows effective
  default-surface inheritance and validates complete running block streams plus
  each table's row, surviving-cell, and cell-block tree through the established
  bounded tab formatter. Vertical-merge ownership, all six variants, malformed
  section/variant/table isolation, coexistence with direct running-paragraph
  tabs and distances, local relationships, generated page numbering,
  deterministic bytes, and native reopen/model equality are covered. Standalone
  model writing remains unchanged; nested running tables, legacy-DOC running
  stories, settings-defined default-tab intervals, and fresh note conversion
  remain outside this bridge.
- Preserves reader-resolved exact and minimum line spacing when
  `Document::to_docx()` freshly converts direct top-level paragraphs in selected
  default/first/even headers and footers from an opened DOCX. The existing
  twelve-vector running-surface sidecar is now retained by default builds,
  requires exact section and per-variant block alignment, and emits bounded
  positive `exact`/`atLeast` rules through the established paragraph formatter.
  All six variants, both section positions, malformed section/variant and
  numeric-value isolation, coexistence with tabs, table-cell tabs, distances,
  local relationships, generated page numbering, deterministic bytes, and
  native reopen/model equality are covered. Standalone model writing remains
  unchanged; legacy-DOC conversion uses the separate bounded bridge below.
  Running pagination controls and fresh note conversion remain outside this
  direct-paragraph bridge.
- Preserves reader-resolved exact and minimum line spacing when
  `Document::to_docx()` freshly converts direct paragraph blocks in surviving
  cells of top-level tables in selected default/first/even headers and footers
  from an opened DOCX. The existing six-variant aggregate now passes an
  independently block-aligned table tree through the established row,
  surviving-cell, and complete cell-block validator plus bounded paragraph
  formatter. Both section positions, all six variants, vertical-merge
  reconstruction, malformed section/variant/table and numeric-value isolation,
  sibling tables, coexistence with direct spacing and both tab bridges,
  distances, local relationships, generated page numbering, deterministic
  bytes, and native reopen/model equality are covered. Standalone model writing
  remains unchanged, while legacy-DOC conversion uses the separate bounded
  bridge below; nested running tables, the separate running-pagination bridge,
  and fresh note conversion remain outside this line-rule bridge.
- Preserves parsed legacy-DOC exact and minimum line spacing when
  `Document::to_docx()` freshly converts direct top-level paragraphs and direct
  paragraph blocks in surviving cells of top-level tables in selected
  default/first/even headers and footers. The existing renderer sidecar is now
  retained by default DOCX builds, keeps its section-aligned six-story mapping,
  and reuses the independently validated direct-paragraph and table-cell writer
  paths. All six variants, exact/minimum isolation, two-section mapping,
  unindexed default-header fallback, deterministic bytes, public-model
  exclusion, native reopen of the private rules, and unchanged standalone model
  writing are covered. Legacy running-story tabs, nested running tables, the
  separate running-pagination bridge, fresh note conversion, and the
  legacy-only build remain unchanged.
- Applies opened-DOCX table-cell tab stops in top-level real footnote and endnote
  tables through the existing table shaper. A private block/row/surviving-cell/
  paragraph tree follows visible note blocks in footnote-then-endnote order,
  skips boilerplate note records before capture, and preserves vertical-merge
  owner indexing. Supported leaders and a positive `w:defaultTabStop` interval
  remain source-only and deterministic. Nested note tables, legacy-note tabs,
  fresh note conversion, post-tab field containment, page-bottom note
  composition, and Word-exact tab reflow and pagination remain outside this path.
- Applies reader-resolved explicit paragraph tab stops to ordinary top-level
  real footnote and endnote paragraphs when an opened DOCX is rendered to PDF.
  A private block-aligned vector is concatenated in the same footnote-then-endnote
  order as the visible note blocks, skips boilerplate note records before
  capture, and preserves empty slots for non-paragraph blocks. Supported leaders
  and a positive `w:defaultTabStop` interval reuse the established paragraph
  shaper without changing the public model. Legacy-note tabs, fresh note
  conversion, post-tab field containment, page-bottom note composition, and
  Word-exact tab reflow and pagination remain outside this path.
- Applies reader-resolved explicit paragraph tab stops to ordinary top-level
  paragraphs in selected default/first/even running headers and footers when an
  opened DOCX is rendered to PDF. A private six-story sidecar follows
  section-local default-surface inheritance and page-variant selection while
  preserving the public model; supported leaders and a positive
  `w:defaultTabStop` interval use the established paragraph shaper. Running
  table-cell tab hints remain independently aligned. Legacy-DOC running stories,
  post-tab field containment, and Word-exact tab reflow and pagination remain
  outside this bounded path.
- Preserves reader-resolved explicit DOCX paragraph tab stops when
  `Document::to_docx()` freshly converts aligned top-level body paragraphs and
  direct paragraph blocks in surviving cells of top-level tables. The bounded
  source-only writer bridge emits validated left, center, right, decimal, and
  bar stops with supported dot, hyphen, underscore, heavy, and middle-dot
  leaders after the reader has applied style/direct/clear precedence. Strict
  block, row, cell, and cell-block alignment, rounded-position ordering, the
  256-stop and 31,680-twip limits, vertical-merge ownership, deterministic
  bytes, and native reopen are covered. Standalone model writing remains
  unchanged; nested-table descendants, notes, and settings-defined default-tab
  intervals remain outside this body/table tab bridge. Direct top-level
  opened-DOCX running paragraphs use the separate section-aligned bridge above.
- Extends source-aware body layout conversion to DOCX-backed
  `Document::to_docx()`. Default builds now retain the reader-resolved
  exact/minimum line rules and effective `keepNext`, `keepLines`, and widow-off
  state for aligned top-level body paragraphs and direct paragraph blocks in
  surviving cells of top-level tables, plus effective no-split state for aligned
  top-level table rows. The existing validated writer bridge preserves those
  five source-only families deterministically through native reopen while
  standalone model writing remains unchanged. Nested-table descendants, notes,
  nested running-table pagination and line rules, package-
  preserving `save()`, and the public model remain outside those layout
  families; direct opened-DOCX running paragraphs and direct cells of top-level
  running tables use the separate bounded line-rule bridges above, while top-
  level body manual column breaks use the separate bridge above.
- Preserves resolved legacy-DOC `keepNext`, `keepLines`, and widow-off semantics
  when `Document::to_docx()` freshly converts aligned direct paragraph blocks in
  surviving cells of top-level tables. The source-only bridge validates block,
  row, cell, and cell-block alignment independently from row and line-spacing
  sidecars, rejects non-paragraph hints, and preserves modeled page breaks in
  CT_PPr order. Direct/style precedence, deterministic bytes, native reopen,
  merge-owner alignment, malformed-sidecar isolation, and unchanged standalone,
  nested-table, and non-body writing are covered; DOCX-backed inputs now use the
  bounded bridge above.
- Preserves resolved legacy-DOC exact and minimum line spacing when
  `Document::to_docx()` freshly converts aligned direct paragraph blocks in
  surviving cells of top-level tables. The source-only writer bridge validates
  block, row, cell, and cell-block alignment as a table-local unit, rejects a
  non-paragraph hint, and reuses the bounded positive `exact`/`atLeast` formatter
  while preserving modeled before/after spacing and proportional fallback.
  Deterministic bytes, native reopen, merge-owner alignment, and unchanged
  standalone, nested-table, and non-body writing are covered; DOCX-backed inputs
  now use the bounded bridge above.
- Preserves resolved legacy-DOC table-row no-split semantics when
  `Document::to_docx()` freshly converts aligned top-level tables. Modern
  `sprmTFCantSplit` values override compatibility `sprmTFCantSplit90` values in
  source order, effective true emits `w:cantSplit`, and effective false remains
  omitted. The source-only writer bridge rejects misaligned block or row vectors,
  keeps `w:cantSplit` before `w:tblHeader` in CT_TrPr order, and leaves standalone
  model writing, nested tables, and non-body tables unchanged. DOCX-backed inputs
  now use the bounded bridge above; deterministic bytes and native reopen are
  covered.
- Preserves resolved legacy-DOC `keepNext`, `keepLines`, and default-on
  `widowControl` semantics when `Document::to_docx()` freshly converts aligned
  top-level paragraphs. The source-only writer bridge rejects a misaligned
  pagination vector, emits effective keep controls, writes widow-off explicitly,
  and keeps the existing modeled `pageBreakBefore` in CT_PPr schema order before
  numbering. Style/direct on/off precedence, deterministic bytes, native reopen,
  and unchanged standalone model writing are covered. Nested-table descendants,
  non-body stories, and the public model remain unchanged; DOCX-backed inputs now
  use the bounded bridge above.
- Preserves resolved legacy-DOC exact and minimum line spacing when
  `Document::to_docx()` freshly converts aligned top-level paragraphs. The
  existing private block sidecar now emits bounded positive `w:line` values with
  `exact` or `atLeast` while preserving public before/after spacing and leaving
  proportional rules unchanged. Style/direct precedence, direct-zero clearing,
  deterministic bytes, native reopen, and strict misalignment fallback are
  covered. Standalone model writing, nested-table-descendant and running-surface
  conversion, and the public model remain unchanged; DOCX-backed inputs now use
  the bounded bridge above.
- Preserves source-defined section-local running header and footer distances
  when `Document::to_docx()` freshly converts an opened DOCX or legacy `.doc`.
  The shared writer bridge consumes strictly section-aligned private hints,
  accepts finite values from zero through 31,680 twips, and falls back each
  missing or rejected edge independently to 708 twips. Accepted-current Markup
  Compatibility selection, malformed-section isolation, deterministic package
  bytes, and native reopen are covered for intermediate and final sections.
  Standalone `write_docx(&DocModel)` remains model-only, and public distance
  authoring, package-preserving `save()`, and Word-exact overlap are unchanged.
- Extends source-aware section-column conversion to DOCX-backed
  `Document::to_docx()`. A default-build capture now retains only the aligned
  equal gaps, complete unequal layouts, separator flags, and section direction
  already resolved by the bounded accepted-current reader; renderer pagination
  sidecars remain feature-gated. Intermediate and final sections emit through
  the validated writer bridge, including selected Markup Compatibility
  branches, while malformed custom geometry stays isolated. Standalone
  `write_docx(&DocModel)` remains model-only and package-preserving `save()` is
  unchanged.
- Preserves validated section-column semantics when `Document::to_docx()`
  converts an opened legacy `.doc`. Section-aligned equal gaps, complete
  unequal widths and following spaces, separator rules, and right-to-left
  population now emit bounded `w:cols`/`w:col` and `w:bidi` properties in the
  fresh package and survive native reopen. Invalid or misaligned private
  geometry falls back to the modeled count and any valid equal gap without
  affecting neighboring sections. Standalone model authoring through
  `write_docx` remains model-only for these private hints; exact per-column
  rewrapping and Word-exact pagination remain outside this conversion bridge.
- Populates opened DOCX and legacy `.doc` section columns from right to left in
  PDF previews when `w:sectPr/w:bidi` or `sprmSFBiDi` is
  active. DOCX on/off values follow accepted-current Markup Compatibility
  selection; legacy Bool8 values retain the last valid source-order state and
  malformed SEPX sections remain isolated. Private section-aligned flags keep
  physical equal and unequal track geometry unchanged while the paginator
  starts at the rightmost track, advances left for overflow and manual column
  breaks, and resets to the rightmost track on each new page. Paragraph/run
  direction, table ordering, public models, and page maps remain independent.
- Paints section column-separator rules in opened DOCX and legacy `.doc` PDF
  previews from `w:cols/@w:sep` and `sprmSLBetween`, respectively. DOCX on/off
  values follow accepted-current Markup Compatibility selection; legacy Bool8
  values retain the last valid source-order state and malformed SEPX sections
  remain isolated. Private section-aligned flags reuse each equal, fitting,
  scaled, or fallback column layout and center a thin rule in every active gap;
  one-column sections remain paint-inert, and pagination and `LayoutPages` stay
  unchanged.
- Applies bounded explicit unequal-width `w:cols` geometry from opened DOCX
  sections to PDF and `LayoutPages` previews through a private section-aligned
  sidecar. An explicit false `w:equalWidth` accepts one through 64 direct
  `w:col` children with positive widths and nonnegative following spaces; child
  count replaces the ignored parent count, selected Markup Compatibility
  branches remain source-stable, and malformed definitions fall back without
  exposing a public API. Fitting dimensions retain their declared origins and
  widths; over-wide sets scale uniformly only while every resulting column
  remains at least the renderer minimum, otherwise the established equal-column
  fallback wins. Pagination places content in each active width, while shaping
  remains conservatively bounded to the narrowest section column. DOCX-backed
  `Document::to_docx()` conversion uses the source-aware bridge above; deferred
  per-column rewrapping and Word-exact pagination remain outside this bounded
  path.
- Applies complete explicit unequal-column geometry from opened legacy `.doc`
  sections to PDF and `LayoutPages` previews through the same private
  section-aligned path. A false `sprmSFEvenlySpaced` selector accepts two
  through 44 indexed `sprmSDxaColWidth` operands from 718 through 31,680 twips
  and optional `sprmSDxaColSpacing` operands from zero through 31,680 twips;
  omitted spacing defaults to zero, later valid indexed values replace earlier
  values, and incomplete geometry retains the conservative fallback. The
  validated count reaches the shared section model while fitting, scaled, and
  minimum-width fallback behavior reuses the DOCX geometry renderer. Malformed
  SEPX sections remain isolated. Opened-legacy `Document::to_docx()` conversion
  of validated geometry uses the source-aware bridge above; deferred per-column
  rewrapping and Word-exact pagination remain outside this bounded path.
- Applies validated legacy `.doc` `sprmSDyaHdrTop` and
  `sprmSDyaHdrBottom` section distances to PDF running surfaces through the
  existing private section-aligned renderer sidecar. Unsigned twip operands
  from zero through 31,680 retain source-order precedence; invalid later values
  leave the last valid value intact, malformed section data remains isolated,
  and absent values preserve the fixed preview fallback. Header and footer
  placement reuses the opened-DOCX physical-edge anchoring, body-margin clamp,
  and deterministic non-overlap policy without changing the public model or
  body page count. Public distance authoring,
  installation-language default reconstruction, positive-margin auto-growth,
  negative fixed-margin semantics, conversion round-trip of this private hint,
  and Word-exact overlap/pagination remain outside this bounded path.
- Emits model-authored core and chart-ex drawings in fresh DOCX
  default/first/even headers and footers, including charts nested through
  ordinary running-table cells. Each running part owns its chart relationship;
  chart payloads, embedded core-chart workbooks, chart-package relationships,
  global chart/drawing ids, extents, and alternative text reuse the body writer.
  Roots add only the required `c`/`cx` chart namespace, document relationship
  ids remain isolated, and all six variants survive native reopen into the
  existing running-surface vector PDF painter. A zero-series running chart keeps
  the previous escaped visible fallback and emits no orphan chart relationship
  or part. Arbitrary opened Office charts, formula/reference-backed or
  combination import, theme/style/data-label fidelity, external refresh, and
  Word-exact chart layout remain outside this bounded path.
- Reconstructs fresh rwml-generated core and chart-ex DOCX payloads as modeled
  `Chart` blocks on native reopen instead of preserving them as opaque chart
  parts. All 52 currently authored kinds retain their kind, title, serialized
  categories, series, bubble sizes, dimensions, alternative text, wireframe,
  and 3-D shape state across top-level and ordinary-table-cell reopen, then use
  the existing vector PDF painter. The decoder accepts only the bounded,
  namespace-correct dense literal-cache structures emitted by rwml, with finite
  values and explicit depth/series/point/text limits; malformed, sparse,
  duplicate, formula/reference-backed, combination, unrelated, and otherwise
  arbitrary Office chart payloads remain preserved and are reported as
  unsupported. Scatter/bubble numeric x-values intentionally normalize the
  public string `categories` vector to empty on reopen.
- Emits present raster image bytes in fresh DOCX default/first/even headers and
  footers with relationships scoped to each owning running part. Top-level,
  paragraph-run, ordinary-table-cell, and nested-table-cell images retain
  deterministic media parts, DrawingML extents, rotation, alternative text,
  inline/floating placement, and image hyperlinks across native reopen; the
  reader now preserves an enclosing hyperlink on drawing runs, so reopened
  linked running images remain actionable in PDF output. Images without bytes
  keep escaped visible placeholders and emit no media relationship. Generated
  crop/flip/effects, image hyperlink annotation geometry, and Word-exact
  floating wrap remain outside this bounded path.
- Emits full table structures in fresh DOCX default/first/even headers and
  footers instead of flattening rows to delimiter-joined text. Running tables
  reuse body-table grid-span, vertical-merge, repeated-header, width/fixed
  layout, alignment/indentation, visual-RTL, border, column-grid, cell
  width/shading/margin/vertical-alignment, paragraph, run-formatting, and nested-
  table serialization. Cell hyperlinks use the owning header/footer
  relationship file and survive native reopen into clickable PDF annotations;
  raster images and model-authored charts use the same part-local relationship
  scope. Empty cells and nested-table terminal paragraphs remain schema-valid.
  Arbitrary opened chart reconstruction, Word-exact table autofit, and
  Word-exact pagination remain outside this bounded path.
- Applies explicit unsigned opened-DOCX `w:pgMar` header and footer distances
  through a private section-aligned renderer sidecar. Headers start at the
  source offset from the physical top edge; footer content and generated page
  numbers are bottom-aligned to the source offset from the physical bottom
  edge. Both bands clamp at the body margin, preserving the renderer's bounded
  non-overlap policy without changing the public model or body pagination.
  Missing, malformed, negative, overflowing, revised-away, and unselected
  markup-compatibility values retain the existing fixed preview bands. Legacy
  `.doc` distances, public distance authoring, automatic margin-conflict
  resolution, and Word-exact overlap remain outside this source-only path.
- Paints decoded block and reader-style inline raster images plus non-empty
  model-authored charts inside ordinary modeled table cells in body content and
  selected default/first/even running headers and footers. Supported cell media
  stays in source order, is centered and proportionally reduced without
  upscaling to the cell's inner width and active page content height, contributes
  to row height and vertical alignment, and remains atomic across row splits and
  repeated headers. Missing or undecodable rasters and empty charts are skipped;
  nested tables retain the same bounded flattened-content behavior. Inline
  baseline placement, relationship display extents, crop/flip/effects, arbitrary
  opened Office chart parts, nested-grid geometry, and Word-exact pagination
  remain outside this bounded path.
- Applies opened legacy-DOC positive non-multiple minimum and negative encoded
  exact LSPD inside modeled table cells in section-linked even/default/first
  running headers and footers. The existing block-aligned table-cell sidecar
  now follows the same six-story, section, and unindexed default-header mapping
  as top-level running paragraphs, then reaches the unchanged shared table
  shaper for row measurement, vertical alignment, and visible-fragment clipping.
  All six placements preserve public-model equality, page count, and
  deterministic PDF bytes. Legacy running-table tabs/media, shared-model line
  rules, fresh conversion, nested-grid geometry, and Word-exact overlap remain
  outside this bounded path.
- Applies opened-DOCX table-cell tab stops in selected default/first/even
  running headers and footers through the existing table shaper. Resolved
  explicit stops retain the supported LTR/RTL alignment, reachability, leader,
  and bar-tab behavior, while cells without a reachable explicit stop use the
  document's finite positive `w:defaultTabStop` interval. Per-block vectors
  follow default-surface inheritance and page-variant selection without entering
  the public model or changing body pagination. Implicit list-marker tabs,
  post-tab field containment, Word-exact custom-tab reflow/pagination, legacy
  running-table tabs, fresh conversion, media, and nested-grid geometry remain
  outside this bounded path.
- Applies opened-DOCX `exact` and `atLeast` paragraph line rules inside modeled
  table cells in selected default/first/even running headers and footers. The
  reader-retained per-block sidecar follows default-surface inheritance and
  variant selection into the existing table layout, so exact/minimum boxes
  participate in row measurement, vertical alignment, and visible row clipping
  without changing the public model, body pagination, or running-band policy.
  Missing or malformed hints retain natural line height; model-authored rules,
  fresh conversion, running-table tabs/media, nested-grid geometry, and
  Word-exact overlap remain outside this bounded path.
- Emits clickable PDF annotations for hyperlink runs in selected
  default/first/even running header and footer paragraphs and modeled table
  cells, including targets resolved from referenced DOCX parts. Candidate
  rectangles are intersected with the physical page and visible margin-band
  line or row fragment; nonfinite, empty, and fully clipped targets are dropped,
  so an over-tall clipped row cannot leave an active link over body content.
  Link order, running cursor movement, diagnostics, body annotations, and page
  counts remain unchanged. Fresh DOCX running-table hyperlinks now retain their
  part-local relationships; Word-exact header/footer overlap remains outside
  this path.
- Paints non-empty model-authored charts in selected default/first/even running
  header and footer bands with the existing vector chart painter. Normalized
  chart dimensions are proportionally reduced, never enlarged, to the remaining
  section-local margin band, centered within its content width, and clipped at
  the fitted bounds; one transform scales labels, strokes, and geometry
  together. Source order, deterministic output, diagnostics, and body page
  counts remain unchanged. Arbitrary opened Office chart-part modeling,
  theme/external-data parity, and Word-exact overlap remain outside this bounded
  path.
- Applies model-backed paragraph before/after gaps in selected
  default/first/even running header and footer bands, including resolved
  `w:spacing` values decoded from referenced DOCX parts. Finite positive gaps
  advance later text, pictures, model-authored charts, and tables in source
  order; explicit zero keeps no gap, while an unset paragraph-after value
  retains the renderer's existing default trailing gap. A gap that consumes the
  remaining margin band ends that surface, and footer page numbers follow the
  bounded cursor without changing body page counts. Adjacent gaps remain
  additive; Word-exact collapse, contextual/automatic spacing, and
  automatic margin-conflict resolution remain outside this bounded path.
- Paints ordinary modeled tables in selected default/first/even running header
  and footer bands, including tables decoded from referenced DOCX parts. Rows
  use the body table painter and retain model-backed width, alignment, visual
  RTL placement, borders, cell shading, margins, and vertical alignment. Full
  rows paint in source order; a first row taller than the remaining margin band
  paints once under an explicit band clip and ends that surface without moving
  body content or changing page counts. Running-table source-only paragraph
  pagination hints, nested-grid geometry, inline media baseline/crop/effects,
  arbitrary opened Office chart parts, and Word-exact overlap remain outside this
  bounded path.
- Paints decoded block and reader-style inline raster images in the selected
  default/first/even running header and footer bands. Existing rotation-aware
  image bounds are proportionally reduced, never enlarged, to the remaining
  section-local margin band and centered within its content width; text order,
  page counts, missing/undecodable-image diagnostics, and body image behavior
  remain unchanged. Arbitrary opened Office chart parts, relationship display
  extents, crop/flip/effects, and Word-exact overlap remain outside this bounded
  path.
- Applies opened legacy-DOC positive non-multiple minimum and negative encoded
  exact LSPD to ordinary paragraphs in section-linked even/default/first
  headers and footers. A section-and-variant-aligned private render sidecar
  mirrors the existing six-story `PlcfHdd` mapping, including the unindexed
  default-header fallback, and follows the running surface selected for each
  emitted page. Exact over-tall content uses the existing margin-band clip.
  Legacy running-table tabs/media, the public model, shared-model conversion,
  and fresh DOCX output remain unchanged.
- Extends opened legacy-DOC positive non-multiple minimum and negative encoded
  exact LSPD into ordinary main-story table-cell paragraphs. A row/cell/block-
  aligned private render sidecar follows paragraph content through horizontal
  folds and retains only surviving vertical-merge owners; exact over-tall
  content uses the existing vertical line clip. The public model and standalone
  shared-model conversion remain unchanged; aligned direct cells now use the
  fresh-conversion bridge above, while nested-table descendants remain excluded.
- Applies opened legacy-DOC positive non-multiple LSPD as minimum line spacing
  and negative encoded LSPD as exact line spacing in top-level main-story PDF
  preview paragraphs. Style and direct-PAPX cascading retains the distinct
  source forms in a block-aligned private render sidecar, including both
  fragments around a promoted manual page break; exact over-tall content uses
  the existing vertical line clip. The public model and standalone shared-model
  conversion remain unchanged; opened-document fresh conversion now uses the
  bounded bridge above.
- Applies opened-DOCX `exact` and `atLeast` paragraph line spacing to ordinary
  paragraphs in referenced default/first/even running header and footer parts.
  Section-aligned private render hints mirror default-surface inheritance and
  follow the variant selected for each emitted page; exact over-tall content is
  clipped to its margin-band line box. Running-surface table-cell line rules,
  model-authored content, and fresh conversion remain outside this source-only
  bridge.
- Extends opened-DOCX `exact` and `atLeast` paragraph line spacing into real
  footnote and endnote blocks through a note-aligned private render sidecar.
  Separator and continuation records do not consume sidecar entries, and
  footnote/endnote concatenation preserves block order. Notes retain their
  existing flattened end-of-flow preview placement; page-bottom note
  composition, model-authored content, and fresh conversion remain outside this
  source-only bridge.
- Extends opened-DOCX `exact` and `atLeast` paragraph line spacing into ordinary
  and recursively nested table cells through merge-owner- and block-aligned
  private render hints. Explicit page-break fragments preserve their source
  rule; selected compatibility wrappers and nested tables retain recursive
  alignment; exact over-tall content is vertically clipped to its cell line
  box; and at-least values only expand shorter natural lines. Aligned direct
  cells now use the fresh-conversion bridge above. Table content in running
  headers/footers, model-authored content, nested-table-descendant conversion,
  nested grid geometry, and nesting beyond the renderer's existing 32-level
  flattening bound remain unchanged.
- Applies resolved `w:spacing` `exact` and `atLeast` line rules to top-level
  body paragraphs in opened-DOCX PDF previews through block-aligned private
  render hints. Values use twentieths of a point after style/direct cascading;
  exact boxes center fitting text and bottom-align with vertical clipping when
  undersized, while at-least boxes only expand shorter natural lines. Explicit
  page-break splits retain alignment. The public model and authored content
  remain unchanged; opened-document fresh conversion now uses the bounded bridge
  above.
- Computes `NOTEREF`/`FTNREF` fields that target a `w:customMarkFollows`
  footnote or endnote when its non-empty literal mark immediately follows the
  reference inside the bookmark. Ambiguous marks retain cached field text, and
  `REF \\f` no longer invents an auto-number for a custom-mark target.
- Preserves each opened DOCX drawing's non-empty `wp:docPr/@descr` alternative
  text on its cloned image occurrence, including the selected Markup
  Compatibility branch and shared-media relationships with different
  descriptions. Block and inline Markdown/HTML exports use the modeled alt text
  with format-appropriate escaping, and fresh DOCX conversion writes it back.
- Promotes an internal main-story end-of-section character (`0x0C`) from an
  opened legacy `.doc` file to the shared `PageBreak` block, so native preview,
  Markdown/HTML export, and fresh DOCX conversion retain the manual page break.
  The final `0x0C` in each non-final `PlcfSed` range is consumed only as its
  section terminator, preventing duplicate page movement. Repeated and
  marker-only breaks remain deterministic, while table cells and non-main
  stories retain their established newline representation.
- Flows visible top-level main-story end-of-column characters (`0x0E`) from
  opened legacy `.doc` files into the next active PDF preview column, advancing
  to a new page after the final column. Renderer-only character offsets retain
  the newline-backed public model, preserve marker-only progress, and exclude
  hidden characters, table cells, and non-main stories. Fresh conversion,
  manual page breaks, and Word-exact pagination remain outside this bounded
  path.
- Flows visible top-level body `w:br w:type="column"` markers from opened DOCX
  files into the next active PDF preview column, advancing to a new page after
  the final column. Renderer-only character offsets preserve the existing
  newline-backed public model and remain aligned across explicit page-break
  splits; marker-only paragraphs and repeated breaks make deterministic
  progress. Ordinary line breaks, hidden runs, table cells, notes,
  headers/footers, model-authored content, fresh conversion, and Word-exact
  pagination remain outside this bounded path.
- Applies legacy `.doc` `sprmSDxaColumns` spacing to opened-document PDF
  previews when a section has two or more evenly spaced columns. The parser
  bounds spacing to the specified nonnegative twip range, keeps zero and later
  valid values in source order, and preserves section-local boundaries. Unequal
  sections use the separate bounded private geometry path above; malformed
  sections keep the existing fallback. Legacy-backed `Document::to_docx()` now
  carries a validated source gap through an internal writer hint, while
  standalone model authoring remains count-only because the public section
  model has no gap field.
- Applies an opened DOCX section's explicit `w:cols/@w:space` value to both
  equal-column paragraph shaping and the corresponding PDF column origins,
  including section-local values selected through accepted-current Markup
  Compatibility branches. Explicit unequal-width `w:col` tracks now use the
  separate bounded private geometry path above, while model-authored sections
  retain the established preview gap because the public section model stores
  only the column count.
- Applies each modeled section's physical page width and height, including
  landscape layouts, plus per-side margins to PDF body shaping and pagination,
  running headers and footers, anchored floating-shape coordinates,
  top-and-bottom wrap bands, and emitted page media boxes. Exact per-column
  rewrapping and Word-exact pagination remain outside this bounded renderer
  bridge.
- Prunes an unreferenced internal image relationship and unreachable `word/media/*`
  target when `remove_body_block` removes the last retained reference, while
  preserving shared media, other relationship kinds, and unrelated package parts.
- Extends opened-DOCX table-style `w:cantSplit` pagination to all twelve
  bounded row/cell conditional selectors: horizontal and vertical bands,
  first/last columns and rows, and four corners. Named and hexadecimal
  `w:cnfStyle`, direct `w:tblLook`, inherited row/column band sizes, effective
  table visual direction, style precedence, and direct-row overrides are
  covered; `w:tblPrEx` remains margin-only for the supported exception path and
  other table-property exceptions remain outside this renderer slice.
- Makes strict render validation account for ActualText-aware RTL list-marker
  punctuation boundaries; the current 21-fixture public render corpus now
  reports 1.000 mean text recall with no fixture below the 0.97 floor.
- Allows `set_table_cell_text` to update direct text in a parent cell that
  contains a nested table while preserving the nested table's text and XML
  structure. Nested-grid layout remains a renderer ceiling.
- Honors effective `w:cantSplit` on recursively flattened nested DOCX table
  rows: fitting protected rows start on a fresh page when needed, while rows
  taller than a fresh page still split for deterministic progress. Nested grid
  geometry and Word-exact pagination remain outside the preview renderer.
- Applies row-local `w:tblPrEx/w:tblCellMar` margin exceptions in place of the
  direct table margin property and before direct cell `w:tcMar`. Sparse or empty
  exceptions inherit omitted sides from the table style or schema defaults,
  including logical `start`/`end` mapping under `w:bidiVisual`, accepted-current
  Markup Compatibility selection, vertical-merge ownership, nested tables, and
  fresh conversion/reopen. Other `w:tblPrEx` properties remain unsupported.
- Applies a table style's bounded horizontal/vertical bands, first/last rows and
  columns, and four corner regions after `wholeTable` for cell margins, flat
  shading, vertical alignment, and percentage preferred width. Selection honors
  inherited Office region precedence, named or hexadecimal `w:tblLook`, row and
  cell `w:cnfStyle`, Word's 0-3 row/column band sizes, repeated header rows,
  spans, RTL corner mapping, and vertical-merge restart ownership; table, row,
  and direct-cell declarations still override the style at their established
  layers. Conditional borders and paragraph/run formatting remain outside this
  subset.
- Inherits a table style's own `w:tblCellMar` through its `w:basedOn` chain as
  the table's cell-margin default, beneath direct `w:tblPr`/`w:tcMar`
  declarations and above the schema defaults, including the style's `wholeTable`
  and bounded conditional cell-presentation regions.
- Applies a table style's `w:tblBorders` — from its own `w:tblPr` or its
  `wholeTable` region, resolved through `w:basedOn` — when the table declares no
  borders of its own, so grid-style tables no longer lose their borders.
- Fills a table's width, indentation, and alignment from its table style —
  `w:tblW`, `w:tblInd`, and table `w:jc`, resolved through `w:basedOn` and the
  `wholeTable` region — for values the table itself leaves unset.
- Applies a table style's `wholeTable` and bounded conditional `w:tcPr` cell
  margins, shading, vertical alignment, and preferred width to cells that
  declare none of their own.
- Inherits a table style's `w:tblLayout` and `w:bidiVisual` for tables that
  declare neither, tracking whether the table declared them so an explicit off
  value still wins.
- Honors a paragraph style's `w:numPr`, so paragraphs whose style declares list
  membership become list items even without their own `w:numPr`; a direct
  declaration still wins.
- Determines a paragraph's heading level from its style rather than from
  `w:outlineLvl`, which records an outline position that Word also sets on
  ordinary body paragraphs; such paragraphs are no longer reported as headings.
- Writes a paragraph's own `outline_level`, so an outline position set without a
  heading survives a write and reopen instead of being dropped.
- Honors `w:lvlOverride` on a `.docx` list instance — both `w:startOverride` and
  a replacement `w:lvl` definition — so a list that restarts or redefines a
  level numbers correctly. Levels an override does not mention keep the abstract
  definition. Legacy `.doc` already honored the equivalent `LFOLVL` overrides.
- Honors legacy `.doc` `LVLF.fNoRestart` restart thresholds, including levels
  supplied by an `LFOLVL` formatting override, so a deeper sequence continues
  across the configured levels but restarts after a more significant one.
- Honors legacy `.doc` `LVLF.fLegal` on base and `LFOLVL` replacement levels,
  rendering every current-level template placeholder as Arabic while preserving
  an original ArabicLZ zero-padded format.
- Preserves strict legacy `.doc` `sprmSFTitlePage` section state through the
  shared model and fresh `.docx` conversion/reopen for both single- and
  multi-section documents.
- Preserves complete legacy `.doc` document-grid modes and valid line pitch from
  `sprmSClm` and `sprmSDyaLinePitch`, plus nonnegative `sprmSDxtCharSpace`,
  through each section's shared model state and fresh `.docx`
  conversion/reopen. Enabled grids without their required valid line pitch are
  omitted, invalid operands leave prior source-order state intact, and a valid
  negative character-pitch delta clears an earlier representable value because
  the existing public model stores that field as unsigned.
- Preserves all six legacy `.doc` `sprmSTextFlow` section directions through
  `SectionSetup` and the final `DocSetup`, including `@`-font/non-`@` glyph
  rotation distinctions, exact fresh `.docx` text-direction output, and reopen
  parity. Invalid values leave the last valid source-order state intact.
- Preserves legacy `.doc` section page-number formats and restart values from
  `sprmSNfcPgn`, `sprmSFPgnRestart`, `sprmSPgnStart97`, and `sprmSPgnStart`
  through the shared model and fresh `.docx` conversion/reopen. Supported
  MSONFC values map exactly, valid unrepresented values use a bounded decimal
  fallback, non-counting values use the spec-permitted decimal fallback,
  invalid values preserve prior source-order state, and disabled restarts
  ignore their stored start.

### Changed

- Replaces the public implementation checklist with a concise capability
  roadmap and explicit renderer, RTL, metafile, and editing ceilings. The
  detailed release roadmap remains outside the public crate.

### Fixed

- Keeps local release validation and registry-response tests compatible with
  Python 3.9 by avoiding newer integer bit-counting APIs and by closing HTTP
  error bodies only when a response body exists. Validation metrics, thresholds,
  publication behavior, and release policy are unchanged.

## 0.1.2 - 2026-08-02 (unpublished)

This version was prepared but never tagged or published; its changes shipped in
`0.1.3`.

### Added

- Expands strict single-DIB metafile raster extraction to source-bearing
  `EMR_BITBLT`, `EMR_STRETCHBLT`, `META_DIBBITBLT`, and
  `META_DIBSTRETCHBLT` records when they use exact `SRCCOPY`, zero source
  origins, one-to-one full-frame geometry, canonical contiguous DIB payloads,
  and, for EMF, an identity source transform with `DIB_RGB_COLORS`. Raw and
  gzip-wrapped forms use the same bounded path. Source-less WMF forms,
  1-bit EMF source blits, scaling, cropping, mirroring, composition, and
  general vector replay remain unsupported.
- Adds package-preserving plain top-level DOCX paragraph insertion through
  `Document::insert_body_paragraph`. Its `0..=body_blocks().len()` position
  space inserts before an indexed atomic direct paragraph/table/content-control
  or before final body section properties, reuses the existing WML escaping,
  significant-whitespace, tab, line-break, Unicode, and forbidden-control
  handling, and remains transactional under `EditSession`. Synthetic retained
  trees cover prefixed/default namespaces, middle/append/blank placement,
  internal section-boundary adjacency, untouched-part byte stability,
  deterministic reopen, stale/committed/rolled back read views, node-budget
  failure, invalid positions, and opaque or cross-block structural hazards.
  Rich paragraph/block properties, numbering, fields, bookmarks, revisions,
  relationships, nested/story-part insertion, duplication, and indexed content
  replacement remain outside this bounded API.
- Adds explicit part-local cached field inventory and result editing through
  `Document::fields_in_part` and `Document::set_field_result_in_part` for the
  main body, real standard footnotes/endnotes, and correctly typed existing
  header/footer parts, including shared or unreferenced physical parts. The
  inventory follows accepted-current revision and `mc:AlternateContent`
  selection, excludes note separator boilerplate, and is the authoritative
  zero-based index for the transactional edit.
- Adds an RAII `EditSession` for grouping existing package-preserving `.docx`
  mutations behind one explicit commit or package-exact drop/unwind rollback,
  including restoration of the pre-session touched-part state.
- Adds atomic `Document::refresh_read_view`; successful edit-session commits use
  it to reparse model, text, metadata, side tables, media, and renderer hints
  while retaining the authoritative package and touched-part evidence.
- Adds a deterministic public render-activation corpus for run paint/hidden
  text, explicit tabs and RTL tables, keep/widow pagination, equal-width
  columns, bounded `wrapTopAndBottom` flow, and
  `table-cell-lists.docx` body/direct-cell/nested-cell numbering, bullet, and
  RTL cases, with per-file provenance.
- Extends renderer validation with fixed-font all-page aHash, foreground ink
  IoU, explicit unmatched/capped page counts, configurable gates, and bounded
  page-pair raster streaming while retaining the historical page-1 aHash.

### Changed

- Replaces the stale `~0.93 mean text recall` figure with the measured public-corpus
  result — 0.996 mean text recall and a 1.00 mean page-count ratio, with 23 of 24
  documents at exactly 1.00 — and documents why the remaining right-to-left fixture
  scores lower: complex-script clusters can only be mapped to text through PDF
  `ActualText`, which Acrobat and Chrome honor but MuPDF/PyMuPDF, pdfminer.six, and
  pypdf ignore. Rendering is unaffected.

- States the roadmap's two conventions explicitly: documented behavior is
  implemented and covered by tests, and the named limits of each bounded slice
  describe the current build rather than scope that has been declined.
  Unchecked roadmap entries are stated as open projects that stay unchecked
  until evidence closes them. Renderer parity wording no longer presents the
  remaining pagination, floating-layout, computed-field, and visual-fidelity
  gaps as inherent to the design. No behavior change.

### Fixed

- Honors a bullet level's declared `w:lvlText` glyph instead of discarding it and
  letting the renderer guess by depth, so a document that asks for a specific
  bullet gets it; levels that declare none keep the existing fallback and a
  `%N` pattern is still treated as autonumber syntax.

- Opened `.docx` PDF previews now resolve default and explicit logical-start,
  center, end/right, and decimal tab stops in right/start-aligned RTL top-level
  body paragraphs from the right page text margin's leading edge. The stops
  retain their margin coordinates under resolved physical or logical left/right
  indents; deterministic fixed-font geometry covers default and explicit stops,
  multiple fields, segmented paint and source ranges, bounded line reflow, and
  the opened-DOCX path. The same source-aligned sidecar now reaches ordinary
  RTL table-cell paragraphs for center/end/decimal stops; nested-grid layout
  remains outside this bounded path.
- Opened `.docx` PDF previews now reserve line width for resolved custom and
  default tab advances before breaking, so supported top-level LTR left/start
  and RTL right/start body content that no longer fits after a tab moves to the
  next line instead of running past the paragraph box. The reservation only
  tightens and is bounded to three passes, so line composition stays
  deterministic, and emitted page counts follow the resulting line count.
  Unsupported directions and source contexts outside the resolved-tab support keep parley's own
  breaking unchanged. Post-tab field containment, Word-exact custom-tab-aware
  reflow, and Word-exact tab-driven pagination remain unsupported.
- Opened `.docx` PDF previews now keep resolved left, center, right, and
  decimal custom tab stops in page-text-margin coordinates under supported
  left, positive first-line, and hanging indents in left/start-aligned LTR
  top-level body paragraphs. Positive `w:defaultTabStop` intervals from
  `word/settings.xml` now configure default-tab fallback targets; the same
  margin-anchored grid remains clamped to the active paragraph box. Synthetic
  fixed-font geometry covers final field placement at resolved stops, all four
  supported tab alignments, continuation-line hanging indents, and
  exact-edge/out-of-box fallback. Separate opened-DOCX evidence covers parsed
  first-line/hanging values, private tab-sidecar activation, deterministic
  layout/PDF output, and unchanged page counts for the bounded fixtures.
  Resolved LTR tab stops in ordinary and recursively nested table-cell
  paragraphs now use the same bounded path through source-aligned sidecars,
  including the ordinary RTL table-cell path for center/end/decimal stops.
  Explicit left-aligned LTR stops in center-, right-, and justified-aligned
  paragraphs now use that path when the resolved stop is reachable. Ordinary
  RTL table-cell center/end/decimal stops and default-tab fallback in supported
  non-left LTR alignments now use the same bounded path. Supported LTR and RTL
  dot, hyphen, underscore, heavy, and middle-dot leaders plus bar tabs now
  paint through that sidecar path in top-level and table-cell paragraphs.
  Settings-defined default-tab intervals in unsupported contexts,
  implicit hanging-indent/list-marker tabs, post-tab field containment,
  custom-tab-aware line reflow, and Word-exact text-ruler behavior remain outside
  this bounded correction.
- PDF table cells now apply finite positive model-backed paragraph before/after
  spacing on emitted paragraphs to row measurement, row splitting, vertical
  alignment, repeated headers, and `LayoutPages` block and modeled `PAGE`-field
  records. Paragraph edges remain source-ordered across direct and recursively
  flattened nested cells, split rows attach before spacing only to the first
  fragment and after spacing only to the final fragment, and bounded line
  truncation does not invent a trailing edge. Unset, zero, negative, and
  non-finite values add no space. Table blocks now report the page of their first
  actually placed row instead of the pre-placement page. Synthetic direct-DOCX
  and style-inherited legacy-DOC
  fixtures cover deterministic layout and PDF output. Word-exact adjacent
  spacing collapse, line-unit/automatic/contextual spacing, spacing-only empty or
  hidden-only paragraphs, nested-grid geometry, table-cell images, and exact
  split-row vertical alignment remain outside this bounded preview behavior.
- PDF previews now paint reader-captured or deterministic fallback list markers
  in ordinary and recursively flattened table cells. One per-story fallback
  state spans surrounding body, direct-cell, nested-cell, and later body
  paragraphs; visual RTL table placement does not reverse it. Empty or
  hidden-only items retain their marker, and split rows or repeated headers
  reuse already-shaped markers without recounting. List-instance identity,
  source restart metadata, marker fonts/glyphs/tabs/alignment, nested-grid
  layout, and Word-exact RTL list typography remain outside this bounded
  support.
- Opened legacy `.doc` paragraphs now preserve direct and paragraph-style
  `sprmPShd80` palette and `sprmPShd` COLORREF shading when the source result
  collapses exactly to one explicit RGB fill. Clear shading uses its explicit
  background, solid shading uses its explicit foreground, and another
  supported pattern is retained only when both explicit colors are identical.
  Style-local values resolve through the bounded base chain before final
  direct-PAPX precedence. Repeated modifiers apply in source order; later
  structurally complete automatic, nil, patterned, invalid, or wrong-sized
  shading suppresses inherited or stale positive state, while a later valid
  modifier recovers. A truncated or unsizeable direct shading modifier
  suppresses and stops that PAPX scan; a structurally malformed style UPX
  invalidates its local style payload. Supported fills flow through the shared
  model, `.docx` conversion/reopen, and existing PDF paragraph paint without
  changing `LayoutPages`. Pattern fidelity, theme/automatic/nil distinction,
  document-default and table/list-style conditional shading, original legacy
  style-graph preservation through `.docx` conversion, paragraph borders,
  table-style effects, and piece `Pcd.Prm` paragraph properties remain
  unsupported.
- Opened legacy `.doc` tables now recover coherent positive physical top,
  left, bottom, right, inside-horizontal, and inside-vertical border channels
  from complete `sprmTDefTable` `TC80` records and direct row-mark
  `sprmTTableBorders80`/`sprmTTableBorders` operands. Compatible and modern
  direct properties following the definition apply in source order only while
  embedded cell edges remain default/inheritable. Visual RTL maps logical outer
  edges onto their physical sides, and recovered colors, widths, and supported
  line styles survive shared-model use and `.docx` conversion/reopen; PDF
  preview consumes the colors and widths through its existing solid-stroke
  paint without changing `LayoutPages`. Projection is limited to strict
  rectangular, unmerged tables with at least two rows and columns and six
  mutually coherent positive roles. Valid definitions with fewer or excess
  complete `TC80` records or equal boundaries still preserve table structure,
  while malformed operands, incomplete or zero-width grids, mixed
  automatic/explicit colors, nil/no-border roles, unsupported line effects,
  conflicting shared edges, pre-definition row borders, topology modifiers,
  and per-cell overrides retain the existing border fallback. Arbitrary
  per-cell conflict resolution, merged/ragged/nested, table-style-derived,
  piece-`Pcd.Prm`, PDF non-solid style paint, and Word-exact border rendering
  remain unsupported.
- DOCX table-formula operand scanning now ignores paragraph-property subtrees,
  so custom `w:pPr/w:tabs/w:tab` definitions no longer become visible cell
  tabs that block cached-result promotion and dependent formula evaluation.
  Genuine run-content markers, formula grammar, and tab-position layout retain
  their existing behavior.
- DOCX cached complex-field inventory now collects visible text and supported
  inline markers only while the paragraph-spanning result scan is inside run
  content. Custom paragraph tab-stop definitions under `w:pPr/w:tabs` therefore
  no longer appear as literal leading tabs in `Field::result` or
  `fields_in_part`, while genuine run tabs, breaks, hyphens, symbols, nested
  fields, accepted-current revisions, selected `mc:AlternateContent` branches,
  field order, and computed results retain their existing behavior across the
  already-supported body, real-note, and modeled header/footer stories. This
  does not compute tab positions, add namespace processing, or change
  field-family source-text reconstruction.
- DOCX `fill_content_control_by_tag`, `fill_content_controls_by_tag`, and
  `fill_template_fields` now encode input tabs/newlines as WordprocessingML
  `w:tab`/text-wrapping `w:br` run content in every already-supported body,
  real-note, and referenced header/footer location. Marker-to-plain refills
  remove stale text-wrapping markers, marker-only values written by these APIs
  retain an empty `w:t` anchor across save/reopen, alternate WML prefixes remain
  namespace-valid, and page/column breaks or foreign same-local-name elements
  are preserved. Exact marker-fragment node budgets, edge-whitespace attribute
  limits, overlap checks, and clone-and-swap rollback remain atomic. This does
  not create paragraphs, redistribute rich runs, synchronize data bindings,
  target source controls/results without any writable `w:t`, or expand the
  supported story set.
- Package-preserving `.docx` body text, cached field-result, content-control,
  and template-fill edits now traverse only the first direct
  `mc:Choice`/`mc:Fallback` branch of each `mc:AlternateContent`, including
  nested containers and namespace aliases. Untaken branches and malformed
  containers without a branch remain untouched; body field edits reject a
  reader/editor branch-inventory mismatch before mutation. Referenced
  header/footer and real-note template fills follow the same policy while
  excluding separator boilerplate. The explicit `replace_text_in_part` escape
  hatch retains its all-descendant behavior. This deliberately does not evaluate
  `Requires`, preprocess or delete alternate branches, or introduce a
  package-global writable field index.
- PDF rendering now honors model-backed whole-degree clockwise rotation for
  top-level and run-attached body raster images. Direct-model angles normalize
  modulo 360, images rotate around their centers, and finite axis-aligned
  rotated bounds drive proportional active-column/page-height fitting and
  pagination. Quarter turns use exact bounds, arbitrary angles retain
  deterministic output, and missing or undecodable image behavior is unchanged.
  Source-authored display extents, crop/flip/effects, floating-anchor offsets,
  exclusion-zone reflow, table-cell images, and Word-exact inline baseline
  placement remain unsupported.
- Opened legacy `.doc` pieces now retain their raw PCD `Prm` plus ordered CLX
  PRCs and apply bounded character formatting after CHPX. Literal `Prm0`
  values force bold, italic, strike, small caps, caps, and hidden text off or
  on. Precompiled `Prm1` groups additionally support literal underline and RTL,
  highlight palette values, and baseline/superscript/subscript, with supported
  values applied in source order and explicit clears preserved; underline
  styles collapse to the shared boolean model. Signed PRC lengths, payload
  bounds, and the 15-bit addressable count are validated, each group is scanned
  once during open, and complete unrelated property operands are skipped. The
  modifier remains aligned per emitted UTF-16 unit across compressed and
  uncompressed pieces, surrogate pairs, overlapping FC ranges, and every
  modeled story region; effective properties drive run boundaries and survive
  `.docx` conversion/reopen. Missing indices and malformed, style/reset-
  dependent, or style-relative groups atomically leave the CHPX-derived result
  unchanged. Piece-level font/size/color and complex-script effects,
  pictures/OLE, paragraph/list/table/section properties, tab changes,
  revision-original formatting, and full character-style resolution remain
  unsupported.
- Opened legacy `.doc` paragraphs now preserve valid unsigned-twip
  `sprmPDyaBefore`/`sprmPDyaAfter` spacing and positive proportional
  `sprmPDyaLine` LSPD values from paragraph styles and direct PAPX. Sparse
  style values resolve through the cycle/depth-bounded STSH base chain before
  final direct overrides; omitted values materialize the MS-DOC zero-before,
  zero-after, and single-spacing defaults through the shared model, `.docx`
  conversion/reopen, and top-level or table-cell PDF preview layout.
  Repeated values apply in source order, invalid values do not replace the last
  valid value, `sprmPIstd` discards earlier direct spacing, structurally
  malformed styles fall back atomically, and truncated direct modifiers retain
  their valid prefix.
  At-least/exact and explicit zero proportional LSPD values clear inherited
  multipliers but remain unset because the shared model cannot express those
  line rules. Line-unit, auto, and contextual spacing; Word-exact adjacent
  spacing; table/list-style effects; piece `Pcd.Prm` paragraph modifiers; and
  Data-stream indirection remain unsupported.
- Opened legacy `.doc` sections now preserve valid SED/SEPX page width, height,
  portrait/landscape orientation, nonnegative left/right/top/bottom margins,
  equal-width `sprmSCcolumns` counts from 1 through 44, and new/even/odd
  `sprmSBkc` break kinds through the shared model and `.docx`
  conversion/reopen. Valid `PlcfSed` boundaries create sections independently
  of header stories, and single-section properties reach the final `DocSetup`.
  Equal spacing follows the MS-DOC default and strict Bool8 overrides; an
  explicit unequal selector suppresses the modeled count, a later valid equal
  selector restores the last valid count, and invalid values leave prior state
  intact. Strict signed `fcSepx`/`cb` bounds and complete SPRM walking isolate
  malformed local payloads to the deterministic default while valid neighboring
  sections survive. Recovered equal-width columns reach the existing bounded PDF
  and `LayoutPages` flow. Preview pagination also starts modeled even/odd
  sections on the requested 1-based physical parity, inserting one body-empty
  filler when needed; section display-number restarts/formats do not affect that
  calculation. Continuous/new-column section marks normalize to the next-page
  fallback. Custom or unequal column widths/gaps, separator lines, manual column
  breaks, RTL column ordering, gutters/facing pages, header/footer distances,
  page borders/grids, vertical justification, negative fixed-position
  top/bottom semantics, section-relative odd/even running-surface selection,
  and Word-exact filler surfaces remain unsupported.
- Opened `.docx` tables now preserve a complete positive direct `w:tblGrid` as
  normalized `Table::col_widths_pct` when its column count matches the
  reconstructed cell/span grid. Revision-history grids, omitted/zero/invalid
  widths, excessive columns, and count mismatches retain the existing
  content-sized fallback without panicking. Fresh `.docx` output now serializes
  valid matching model proportions as deterministic positive `w:gridCol` twip
  widths and retains the equal-grid fallback for hostile or incomplete model
  values. The existing PDF path therefore honors opened-DOCX grid proportions,
  including visual RTL mirroring and column spans. Word-exact fixed/autofit
  layout, preferred-cell/table/grid conflict resolution, `gridBefore`/
  `gridAfter`, style and row-exception widths, and absolute page-aware width
  synthesis remain unsupported.
- PDF rendering now applies finite positive preferred-percentage table widths
  within the active page or section column, maps logical leading/center/trailing
  table placement through visual RTL, bounds non-negative leading indentation
  to the remaining horizontal space, and mirrors logical cells inside the local
  table box. Repeated headers and split-row fragments retain the same outer
  coordinates; malformed table or column percentages keep finite deterministic
  fallbacks. Absolute/auto widths, true fixed/autofit layout, style and row
  exceptions, floating or nested-grid placement, negative outdents, table
  `both` justification, and legacy `.doc` outer table geometry remain outside
  this preview-grade bridge.
- PDF table grids now honor six-way model-backed solid border colors and
  positive eighth-point widths. Physical top, left, bottom, right,
  inside-horizontal, and inside-vertical channels resolve side-specific values
  before uniform and black/0.4-point fallbacks. Line widths are capped at the
  ECMA-376 maximum of 96 eighths (12 points), then conservatively clamped
  table-wide to half the smallest laid-out cell dimension. Centered rectangles
  share one visible-width strip across unequal neighboring cells and rows,
  physical sides survive visual RTL placement, modeled row/column spans
  suppress covered inside edges, and split row fragments omit artificial
  horizontal seams. Repeated headers retain paint and edge identity while
  `LayoutPages` remains unchanged. Non-solid/none styles, cell borders, theme
  and table-style inheritance, cell spacing and border-conflict resolution,
  ragged-row repair, nested-grid borders, and legacy `.doc` per-cell,
  no-border, conflicting, merged/ragged, nested, or style-derived border
  recovery remain unsupported.
- Opened legacy `.doc` paragraphs now preserve strict modern logical
  `sprmPDxaLeft`, `sprmPDxaRight`, and `sprmPDxaLeft1` signed-twip indents from
  direct PAPX and paragraph styles. Sparse style values resolve through the
  existing cycle/depth-bounded STSH base chain before final direct overrides.
  Logical leading/trailing edges resolve against final paragraph direction,
  positive first-line offsets remain first-line indents, and negative offsets
  become hanging indents through the shared model and `.docx`
  conversion/reopen. Direct `sprmPNest` is additive when a style-derived or
  direct logical-left base exists; prohibited style nesting and direct
  nest-only values without a base remain unmaterialized. Invalid XAS values do
  not replace the last valid local value, structurally malformed styles fall
  back safely, and truncated direct modifiers retain the valid prefix.
  List-level, compatibility-era, character-unit, and mirrored indents, plus
  negative-indent PDF outdenting, remain unsupported.
- Opened legacy `.doc` tables now preserve strict direct `sprmTFBiDi` and
  compatibility `sprmTFBiDi90` Bool16 direction from row-terminating PAPX.
  Repeated values of each property apply in source order, either final
  property enables visual RTL, equivalent rows remain one table, and a
  direction change starts a separate table before merge/width resolution.
  Cells stay in source-logical order through the shared model and `.docx`
  conversion while the existing PDF renderer mirrors their visual positions.
  Invalid Bool16 values are ignored and truncated modifiers retain only the
  valid prefix. Table-style/Data-stream/`Pcd.Prm` table direction, additional
  position/wrapping/protection boundaries, and nested legacy tables remain
  unsupported.
- Opened legacy `.doc` paragraphs now preserve valid direct `sprmPFBiDi`
  Bool8 direction plus physical left/center/right and logical
  start/center/end alignment from `sprmPJc80` and `sprmPJc` through the shared
  model and `.docx` conversion/reopen. Supported distribution, Kashida, and
  Thai values collapse to generic justify; the indented logical value remains
  outside the shared alignment model. Logical start/end resolve against
  paragraph direction, and paragraphs without explicit justification use that
  logical start edge. Generated BiDi paragraphs retain explicit physical-left
  alignment when present. The same bounded direction/justification subset now
  resolves through cycle/depth-bounded paragraph-style STSH inheritance before
  final direct PAPX overrides. Character-style/language-derived direction,
  list-level and compatibility-era/character-unit/mirrored legacy indents,
  exact RTL list-level layout, table-style-derived visual RTL, piece `Pcd.Prm`
  paragraph direction/modifiers,
  and Markdown/HTML visual RTL remain unsupported.
- Opened legacy `.doc` CHPX runs now preserve literal direct `sprmCFBiDi`
  on/off values through the shared model, `.docx` conversion, and PDF run
  isolation. Style-relative operands use the conservative unknown fallback,
  while character-style/reset operators preserve established direction as
  required by MS-DOC. Character-style/language-derived direction,
  complex-script properties, table-style-derived visual RTL, style-relative or
  style/reset-dependent piece `Pcd.Prm` direction modifiers,
  and Markdown/HTML visual RTL remain unsupported.
- Opened legacy `.doc` CHPX runs now preserve literal direct
  `sprmCFSmallCaps` and `sprmCFCaps` on/off values through the shared model,
  `.docx` conversion, and PDF rendering. Style-relative toggle operands and
  complete character-style/reset modifiers conservatively discard stale
  direct state until a later literal value; character-style-derived
  capitalization, style-relative `Pcd.Prm` operands, and Markdown/HTML visual
  caps/small-caps remain unsupported.
- Opened legacy `.doc` CHPX runs now preserve direct `sprmCIss` normal,
  superscript, and subscript values through the shared model, `.docx`
  conversion, and PDF rendering. Complete character-style/reset modifiers
  conservatively discard stale direct state and use the baseline fallback until
  a later valid direct value; character-style-derived alignment,
  style/reset-dependent piece `Pcd.Prm` vertical-alignment modifiers,
  arbitrary `sprmCHpsPos` baseline shifts, and Markdown/HTML visual
  super/subscript remain unsupported.
- Opened legacy `.doc` CHPX runs now preserve `sprmCHighlight` Ico palette
  values through the shared model, `.docx` conversion, and PDF rendering,
  including explicit highlight clearing and deterministic rejection of invalid
  or truncated operands. Style/reset-dependent piece `Pcd.Prm` highlighting,
  full legacy character-style resolution, and Markdown/HTML visual
  highlighting remain unsupported.
- Opened legacy `.doc` tables now preserve relative column proportions from
  coherent `sprmTDefTable` `rgdxaCenter` row boundaries, including mixed
  internal row grids represented through the existing global column spans.
  Missing, zero-width, descending, or inconsistent outer-edge geometry keeps
  the deterministic content-sized fallback; absolute table sizing, autofit,
  indentation, preferred cell widths, table-style-derived RTL, and nested
  legacy tables remain unsupported.
- Legacy `.doc` list labels now honor bounded `PlfLfo`/`LFOData`/`LFOLVL`
  per-instance start and replacement-format overrides and continue numbering
  across `ilfo` instances that share an `lsid`; malformed variable records retain
  deterministic fixed-LFO fallback without panicking.
- Opened `.docx` tables now materialize direct `dxa`/`nil`
  `w:tblCellMar` defaults and per-side direct `w:tcMar` exceptions into
  physical cell margins, including logical `start`/`end` mapping under
  `w:bidiVisual` and the `0/115/0/115`-twip schema fallback. Malformed,
  percentage, and automatic declarations inherit from the lower layer;
  table-style, conditional-style, and `w:tblPrEx` margin inheritance remain
  unsupported.
- Floating-shape preview coordinates now distinguish the page, page-margin text
  rectangle, and physical left/right/top/bottom margin bands; bounded
  `wrapTopAndBottom` flow also honors top/bottom-margin anchors when their visual
  bounds intersect body text.
- Opened `.docx` rendering now honors effective table-row `w:cantSplit` from
  direct row properties and inherited table-style chains, including bounded
  `wholeTable`, all horizontal/vertical band, first/last row/column, and
  corner conditional regions selected by direct table `w:tblLook` or row
  `w:cnfStyle`. Row and column band sizes honor inherited and direct-table
  0-3 values, effective table visual direction, and later-region precedence.
  Default rows may use remaining page space, fitting protected rows move whole,
  and over-tall rows still split deterministically. `w:tblPrEx` remains a
  margin-exception path; its other table properties do not contribute row
  pagination.
- Opened legacy `.doc` rendering now honors direct table-row
  `sprmTFCantSplit` and compatibility `sprmTFCantSplit90`: rows remain
  splittable by default, fitting protected rows move whole, the modern property
  takes precedence when both are present, and over-tall rows still split for
  deterministic progress. Inherited/table-style and nested legacy row controls
  remain unsupported.
- Opened legacy `.doc` rendering now resolves paragraph-style STSH inheritance
  followed by direct PAPX for `sprmPFKeep`, `sprmPFKeepFollow`, default-on
  `sprmPFWidowControl`, and `sprmPFPageBreakBefore` on emitted nonblank
  top-level and ordinary table-cell paragraphs. Private source-aligned hints
  carry keep/widow behavior, the existing model property carries page breaks,
  and explicit direct-off values override inherited-on values. Over-tall
  content still splits deterministically. Other STSH properties,
  table/list-style effects, piece `Pcd.Prm` paragraph modifiers, nested legacy
  tables, and controls on discarded blank top-level paragraphs remain
  unsupported.
- Opened `.docx` table rows now honor resolved `keepNext`, `keepLines`, and
  default-on `widowControl` for direct, accepted-current wrapper-contained, and
  recursively nested cell paragraphs when choosing legal row fragments, while
  isolating paragraph chains by cell and retaining deterministic progress for
  over-tall content. Nested content remains a bounded flattened-text preview,
  not nested grid layout.
- Opened `.docx` renders now resolve inherited and direct left, center, right,
  and decimal tab stops in top-level body paragraphs, including `clear`
  overrides, and preserve authored zero paragraph after-spacing instead of
  substituting the preview default gap.
- Opened `.docx` paragraphs now resolve document defaults, the declared default
  paragraph style for unstyled paragraphs, and bounded explicit paragraph-style
  chains for before/after spacing, proportional automatic line spacing,
  first-line/hanging indents, flat RGB shading, and
  `pageBreakBefore`, with direct zero/off precedence. Exact/at-least line rules
  remain unset in the public proportional-spacing model; unsupported nearer
  nonzero line-unit spacing, nonzero character-unit indent, enabled automatic
  spacing, theme, pattern, nil, or malformed forms suppress representable
  inherited values instead of leaking base formatting;
  paragraph-style chain limits are evaluated independently of map iteration
  order. Zero line- and character-unit attributes clear related inherited unit
  overrides; nonzero unit values remain outside the represented subset.

## [0.1.1] - 2026-07-16

Patch-compatible stabilization release. Default-feature and all-feature public
APIs are checked against `v0.1.0` by `cargo-semver-checks`.

### Added

- Expands bounded WMF/EMF single-DIB raster extraction to 1/4/8-bit palettes,
  16/32-bit `BI_BITFIELDS`, and full-scan SETDIB records, with strict identity
  transfer semantics and decoded-allocation/inflation ceilings.
- Improves preview PDF fidelity for modeled run paint, vertical alignment, and
  hidden-content exclusion; paragraph shading, spacing, indents, and tabs; table
  cell margins and RTL visual order; bounded paragraph/run RTL shaping with
  bundled Arabic/Hebrew subsets; and equal-width section-column flow.
- Applies resolved `.docx` `keepNext`, `keepLines`, and default-on
  `widowControl` to opened-document PDF/layout pagination through private
  source-aligned hints; raw `DocModel` render entry points remain source-agnostic.
- Applies bounded forward `wrapTopAndBottom` exclusion after eligible recovered
  top-level paragraph anchors with explicit page/margin or enabled `simplePos`
  vertical geometry, while retaining overlay fallback for unsupported wrap cases.
- Adds typed enumeration plus package-preserving move/removal for conservative
  atomic direct `.docx` body paragraphs, tables, and content-control subtrees,
  with transactional cross-block range/field/section/opaque-content rejection.
- Computes side-table `STYLEREF` fields in headers, footers, footnotes,
  endnotes, comments, and text boxes with report/evaluator parity.
- Adds a license-clean public legacy `.doc` extraction corpus with exact report
  expectations and Apache POI 5.2.3 / LibreOffice 26.2.3.2 goldens.
- Executes generated WASM bindings under Node in CI and release preflight, and
  freezes document/render report JSON shapes with checked-in golden contracts.
- Adds release-mode public-corpus performance, bundled-font, fuzz-build,
  deterministic-corpus, package-identity, and semantic-version gates. Release
  automation publishes the checksum-verified `rwml-fonts` dependency first,
  waits for registry visibility, and then packages and publishes `rwml` with
  idempotent registry-identity checks.

### Fixed

- Tightens single-DIB WMF/EMF extraction to spec-correct inclusive EMF bounds,
  consistent headers and terminal records, frame-covering destinations, and an
  exact one-raster record stream so later vector composition stays unsupported.
- Preservation edits now resolve targets across accepted revisions, ignore
  deleted comment markers and nested tables, handle rejected header revisions,
  retain comment run formatting, and validate mutations before commit.
- Fixes 32-bit WASM compilation without changing the public `usize` APIs.

### Security

- Expands the edit fuzz target across the package-preserving mutation surface,
  makes its lockfile reproducible, and seeds parse/edit/render fuzzing from the
  public hostile-input corpus.

## [0.1.0] - 2026-07-04

First public release. `rwml` is a native Rust toolkit for Microsoft Word
documents — **read, write, edit, and render** — covering **both** legacy `.doc`
(Word 97–2003 OLE2 binary, [MS-DOC]) and modern `.docx` (OOXML WordprocessingML),
with no JVM, no Apache POI, no other `.docx` crate, and no subprocess.
`#![forbid(unsafe_code)]`, fuzzed, XXE-safe, and zip-bomb-guarded.

### Added

#### Formats & model
- **One model for both formats.** [`Document::open`] format-detects from the
  magic bytes (OLE2 `D0CF11E0` → `.doc`, ZIP `PK` → `.docx`) and **both** backends
  produce the same [`DocModel`], so the Markdown/HTML exporters, the `.docx`
  writer, and the PDF renderer all consume one IR and callers never branch on the
  source format.
- **Typed IR + Markdown/HTML export.** `Document::model` → `Vec<Block>`
  (`Paragraph | Table | Image | PageBreak | SectionBreak`) with lazy typed
  construction, plus `to_markdown` / `to_html` — no other Rust crate does this for
  the legacy binary `.doc` format. `CharProps` carries font/size/color/highlight/
  vert-align/small-caps; `ParaProps` gains spacing/indent/shading and `bidi`;
  `Cell`/`Table` gain shading, vertical alignment, and column widths; new
  `DocSetup`/`PageSetup`. All additive and `Default`.
- `extract_text(&[u8]) -> Result<String>` convenience entry point; `Document`
  API `open`, `text`, `main_text`, `footnote_text`, `header_text`, `char_count`,
  `is_complex`; typed [`Error`] enum with panic-free, bounds-checked parsing.

#### Reading
- **Unified `.docx` (OOXML WordprocessingML) reader.** Behind the default-on
  `docx` feature (`zip` + `quick-xml`), parses `word/document.xml`
  (paragraphs/runs with bold/italic/underline; tables with `gridSpan`/`vMerge` →
  colspan/rowspan), `word/styles.xml` (heading levels: `Heading N` / `제목 N`),
  `word/numbering.xml` (ordered vs bullet, per-level labels), `word/_rels` +
  `word/media` (hyperlink targets and inline images), comments, tracked
  revisions, footnotes/endnotes, and text boxes. Recursion-depth-capped, XXE-safe,
  and zip-bomb-guarded. Validated against python-docx on the 127-file Apache POI
  `.docx` corpus: **98.6% mean / 100% median set-word recall, 85/87 files ≥ 99%,
  0 panics.** Disable with `default-features = false` for a dependency-light
  `.doc`-only build.
- **Legacy `.doc` reader.** OLE2 compound-file access via `cfb`, FIB parsing by
  navigating variable-length sub-structures (never hard-coded offsets), CLX/
  piece-table decoding, UTF-16LE and codepage-aware 8-bit (`fCompressed`) piece
  decoding in the document's ANSI codepage derived from the FIB language id
  (`lid`) — Korean `0x0412` → cp949/EUC-KR, Japanese → cp932, etc. Rich second
  pass: CHPX character-property bins (bold/italic/underline/strike/hidden, font
  name from `SttbfFfn`, half-point size, color), STSH stylesheet + outline levels
  (headings, English `Heading N` and Korean `제목 N`), `sprmTDefTable`
  merge-aware tables (real colspan/rowspan), `PlfLst`/`LSTF`/`LVL` list
  autonumbers (decimal, roman, letter, ordinal, circled, and Korean `가나다`/
  `ㄱㄴㄷ`/`일이삼`/native counting), hyperlink field marks, and `PICF` inline
  PNG/JPEG/GIF images (`images()` ≈ POI `getAllPictures`). Control-mark handling
  matches POI (tab preserved; column break → newline; non-breaking hyphen/space
  normalized).
- **Per-section legacy `.doc` headers/footers.** `PlcfSed` section boundaries
  flow into the shared model as section breaks, so each legacy section's
  `PlcfHdd` story group (default/even/first header and footer variants) applies to
  its own section; `HeaderFooter` gains a public `section: Option<usize>` field.
  Single-section and malformed-table documents keep prior behavior.
- **Exact legacy `.doc` note and shape anchors.** Footnotes/endnotes anchor at
  their `PlcffndRef`/`PlcfendRef` reference positions (every note), and text boxes
  anchor at their `PlcSpaMom` `SPA` positions when counts align; a single
  unambiguous marker anchors to its containing body text, and malformed tables
  keep source-region anchors.
- **Legacy `.doc` comment author metadata.** Legacy comments carry `author` and
  `initials` recovered from the `PlcfandRef` `ATRDPre10` records and the
  `GrpXstAtnOwners` owner-name table; truncated tables leave the fields unset.
- **Comment metadata.** `Comment` gains a public `resolved: Option<bool>`
  recovered from `commentsExtended.xml` (`w15:done`), distinguishing resolved
  from open comments.
- **Style-inheritance-resolved run formatting** and richer read model surfaces
  across both backends (only direct `w:rPr` is read for `.docx`, matching `.doc`
  CHPX behavior).

#### Fields
- **Deterministic field evaluators**, source-order stable, spanning the field
  families: formula/expression (`=`, `IF`, `QUOTE`, `COMPARE`, `FILLIN`, `ASK`,
  `SET`, `NEXT`/`NEXTIF`/`SKIPIF`), table-position aggregate formulas, `PAGE`/
  `PAGEREF`, `REF`, `STYLEREF`, `TOC`/`TC`/`SEQ`, `NOTEREF`/`FTNREF`, document-
  info/date/stat (`DATE`, `TIME`, `AUTHOR`, `TITLE`, `NUMPAGES`, `FILESIZE`, …),
  `SECTION`/`SECTIONPAGES`/`REVNUM`, display/layout (`EQ`, `SYMBOL`, `ADVANCE`),
  numbering/list (`AUTONUM`/`AUTONUMLGL`/`AUTONUMOUT`/`LISTNUM`), legacy form
  (`FORMTEXT`/`FORMCHECKBOX`/`FORMDROPDOWN`), and diagnostic-only families
  (inserted/external content, mail-merge helpers, reference/index, action/
  automation, compatibility/private, barcode). Each `FieldKind` is distinguished
  from unknown fields; unsupported fields preserve cached display text and report
  a precise reason (`UnknownField` / `UnresolvedBookmark` / `UnsupportedSwitch` /
  `NoComputedResult`) with both field-kind counts and reason counts in
  diagnostics.
- **Caller-supplied field evaluation context.** `FieldContext` +
  `Document::fields_with_context` compute volatile fields deterministically from
  caller inputs: `DATE`/`TIME` with an explicit `\@` picture format the supplied
  timestamp, and `USERNAME`/`USERINITIALS`/`USERADDRESS` without literal overrides
  use the supplied identity. Context is an input and never overrides
  document-derived computed results; `fields()` behavior is unchanged.
- **MERGEFIELD / template-fill support** via `fill_template_fields` (content
  controls + MERGEFIELD cached results); INCLUDETEXT and other inserted/external
  content fields are named and keep cached text without evaluating linked content.
- **Side-table field evaluation.** The deterministic evaluation subset that
  applies to body text now also computes inside comment bodies and anchors,
  tracked-change (revision) text, note anchors, floating-shape and text-box text,
  and TOC heading sources, with matching document/render report parity.
- **`REF` numeric-picture and lock-result switches.** `REF <bookmark> \# "<pic>"`
  formats a numeric bookmark value through the shared number-picture formatter
  (falling back to cached text with `NoComputedResult` when the bookmark is
  non-numeric), and the `\!` lock-result switch is accepted as a neutral switch.
- **`NOTEREF` numbering fidelity.** `w:customMarkFollows` note references no
  longer consume an auto-number, so later `NOTEREF` ordinals are correct; and
  document-level `w:footnotePr`/`w:endnotePr` `numStart`/`numFmt` from
  `settings.xml` are applied to computed note numbers (per-page restart stays a
  documented layout-dependent ceiling).

#### Writing & editing
- **`.docx` authoring (`rwml::write_docx`, `DocBuilder`).** Build a `DocModel`
  from data and serialize a clean, Office-openable `.docx`: rich `w:rPr` (font,
  half-point size, color, highlight, small-caps, super/subscript), `w:pPr` (named
  heading styles via a synthesized `styles.xml` with `outlineLvl`, alignment,
  spacing, indent, shading, page-break-before), leveled lists, bordered tables
  with width/fixed-layout/alignment/indentation/per-side border widths/styles/
  colors and per-cell shading/width/margins/vertical alignment, images with alt
  text/pixel size/inline rotation/floating offsets, simple fields with cached
  results, `PAGEREF` helper runs, dirty TOC heading-range fields, run-anchored
  comments with reply parent ids and commentsExtended metadata, tracked
  insertion/deletion runs, run-level content controls with data-binding metadata,
  bookmarked runs, authored footnotes/endnotes, string custom document properties,
  raw custom XML data-store items, generated core metadata, explicit Word document
  ids, web-extension task pane package shells, page setup with section columns/
  document grids/text direction/title pages/page-number restarts, section breaks,
  and styled default/first/even running headers/footers with page numbers. All
  round-trips through the reader; opens in Word (verified via python-docx) and
  LibreOffice. `try_write_docx` is a fallible `write_docx`. See
  `examples/report.rs`.
- **Chart authoring.** `ChartBuilder` emits the current core OOXML chart families
  — bar/column/line/area (incl. stacked, 100%-stacked, and 3-D variants), radar,
  scatter, bubble, pie/doughnut (incl. exploded and 3-D), surface/3-D surface,
  high-low-close stock and stock, and pie-of-pie/bar-of-pie — with embedded
  workbook-backed source data,
  plus `wireframe()` styling for surface-family charts and `ChartShape` styling
  (cylinder/pyramid) for 3-D bar/column-family charts.
- **Package-preserving `.docx` editing (`Document::open` → edit → `save`).**
  Opening a `.docx` retains the whole OPC package, and `save() -> Result<Vec<u8>>`
  re-emits it with every unmodeled part preserved verbatim — themes, settings,
  fonts, comments, custom XML, charts, embeddings, and unknown/future parts. A
  no-op open→save is part-payload byte-stable. Editing is **element-tree only**:
  `replace_body_text`, `set_field_result`, `fill_content_controls_by_tag`,
  `fill_template_fields`, `accept_all_revisions`/`reject_all_revisions`,
  `set_hyperlink_target`, `set_comment_text`/`add_comment_on_text`,
  `replace_header_footer_text`/`replace_text_in_part`, `add_footnote_on_text`/
  `add_endnote_on_text`/`replace_note_text`, `set_table_cell_text`, image
  add/replace for PNG/JPEG/GIF/BMP/TIFF/WebP, and `set_core_property` mutate the
  live `document.xml` element tree or media parts, so fields, content controls
  (`w:sdt`), `mc:AlternateContent` shapes, comments, and tracked changes are
  preserved; lazy promotion re-serializes only the edited part. `Document::new()`
  starts from a bundled blank template. New internals: `opc` (OPC round-trip
  layer) and `xmltree` (an edit-preserving arena XML tree). Validated on the
  127-file corpus with python-docx as the strict external checker.
- **Marker fidelity.** `set_field_result`, `replace_body_text`,
  `replace_header_footer_text`, `replace_text_in_part`, `set_table_cell_text`,
  and comment/note edits write user-supplied tabs/newlines as WordprocessingML
  `w:tab`/`w:br` markers (and `xml:space="preserve"` for leading/trailing
  whitespace) instead of literal control characters.

#### Rendering (`render` feature)
- **PDF rendering (`rwml::render_pdf` / `to_pdf`).** Native typesetting with
  `parley` (Korean/CJK [UAX #14] line-breaking + script font fallback) and
  `krilla` (subsetted embedded fonts, selectable text). Honors run color/size/
  font, caps/small-caps, lists with real autonumber labels and indentation,
  bordered tables with shaded vertically-aligned cells and authored column widths,
  images (PNG/JPEG/GIF/WebP), and clickable hyperlink annotations; page size/
  orientation/per-side margins come from the model; multi-page tables repeat
  header rows and oversized rows split across pages.
  `render_pdf_with_fonts` registers caller-supplied fonts for headless/server use;
  `render_pdf_with_report` / `to_pdf_with_report` expose the emitted page count
  and renderer warnings. **Preview-grade, not a LibreOffice replacement**: exact
  pagination and exact floating-object layout differ. Measured against LibreOffice
  on a real corpus it reaches ~0.93 `.docx` text recall (~0.96 for `.doc`) with
  close page counts.
- **Layout-derived page numbers.** `layout_pages_with_fonts` /
  `Document::layout_pages_with_fonts` report the page count and the page each body
  `PAGE` field and top-level block lands on in rwml's own preview-grade pagination
  — matching rwml's PDF output, **not** Microsoft Word's pagination. Supplied
  fonts are used strictly (system fonts disabled), so identical document + font
  bytes yield identical results; values live in a separate `LayoutPages` record
  and never touch `Field::computed_result`.
- **Floating-shape overlays.** PDF previews draw recovered `.docx` `wp:anchor`
  geometry as approximate overlay boxes with `behindDoc` z-ordering, resolving the
  anchor line to a best-effort top-level body block page (through transparent
  content controls, custom XML, single-branch `mc:AlternateContent`, and
  accepted/current revision wrappers); compact placeholder lines stand in for
  preserved charts, OLE objects, and unsupported metafile images.
- **Metafile diagnostics.** Single-DIB raster extraction plus
  `report().features.metafiles` exposing WMF/EMF/EMZ/WMZ part path, format, byte
  size, compression flag, and header-derived dimensions when a raw or gzip-wrapped
  header makes that cheap to recover.
- **RTL property plumbing.** `ParaProps.bidi` (`w:bidi`), `CharProps.rtl`
  (`w:rtl`), and `Table.bidi_visual` (`w:bidiVisual`) are read from `.docx`,
  round-trip through `write_docx`, and get builder setters. Shaping and alignment
  mirroring in the renderer are a later milestone.
- **Optional `bundled-fonts` feature** pulls the separate OFL-licensed
  `rwml-fonts` companion crate and exposes `render_pdf_bundled` /
  `try_render_pdf_bundled` for KS X 1001 Hangul + hanja plus Latin PDF rendering
  without changing rwml's MIT license.

#### Diagnostics & safety
- **Feature inventory / report JSON.** `Document::report()` (and its `to_json()`)
  surfaces format, stats, edit capability/edited-part names, core + custom
  document properties, and a feature inventory (notes, text boxes, floating
  shapes, metafiles, fields with field-kind and reason counts). `edited_parts()`
  reports touched package parts.
- **Safety.** `#![forbid(unsafe_code)]`; malformed or hostile input returns an
  `Error` or read-only diagnostics, never a panic; bounds-checked parsing; fuzz
  targets (read, render, and a scripted preservation-`edit` target); XXE-safe XML
  (external entities never resolved); and a zip-bomb guard capping each `.docx`
  ZIP part's decompressed size at 64 MiB, rejected up front when the declared
  uncompressed size exceeds it.
- **Encrypted / obsolete detection.** Encrypted / XOR-obfuscated documents
  (`fEncrypted`/`fObfuscated`) return `Error::Encrypted`, and pre-Word-97 files
  (Word 6/95, `nFib < 0x00C1`) return `Error::UnsupportedVersion`, instead of
  emitting garbage.
- **Release tooling.** `scripts/render_validate.py` (recall / page-count /
  visual-hash vs LibreOffice), `scripts/bench_vs_mature.py` (extraction benchmark
  vs POI/LibreOffice goldens), `scripts/public_hygiene_audit.py`, and
  `scripts/release_manifest.py` (named `public-release` policy embedding required
  Rust gates and selected optional render/extraction thresholds).

### Removed
- **The Phase-A model-overlay edit surface** (`Document::body_mut()` /
  `apply_body_overlay`): regenerating `document.xml` from the lossy model cannot
  preserve body-coordinated constructs, so package-preserving editing is
  element-tree only.

[Unreleased]: https://github.com/HyunjoJung/rwml/compare/v0.1.4...HEAD
[0.1.4]: https://github.com/HyunjoJung/rwml/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/HyunjoJung/rwml/compare/v0.1.1...v0.1.3
[0.1.1]: https://github.com/HyunjoJung/rwml/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/HyunjoJung/rwml/releases/tag/v0.1.0
[MS-DOC]: https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-doc/
