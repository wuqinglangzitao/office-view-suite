# rwml public validation corpus

A small, **license-clean** corpus for validating rwml's readers and the `.docx`
package-preserving editor in the open (CI and anyone who clones the repo). It
complements the maintainer's larger private real-world corpus, which is not
redistributable.

The corpus includes generated `.docx` fixtures plus a small generated Word
97-2003 `.doc` extraction benchmark. No third-party `.doc` file is redistributed:
the legacy fixtures were exported from repository-owned synthetic sources, and
their Apache POI and LibreOffice text outputs are checked alongside exact rwml
report expectations. The public hygiene audit scans bounded decoded byte views
of every legacy binary and blocks oversized files.

Every file here is safe to redistribute:

- `MANIFEST.tsv` — expected `Document::report()` feature counts and warning
  classes for every committed public `.docx` fixture, including the generated
  synthetic set and permissively licensed vendored files. It is checked by
  `tests/public_corpus.rs`.
- `RENDER_MANIFEST.tsv` — expected native-render page counts and render warning
  classes for every manifest-listed fixture. It is checked when tests run with
  `--features render` and keeps strict release validation aligned with
  `MANIFEST.tsv`.
- `benchmark/` — three generated `.doc` fixtures, exact report expectations, and
  Apache POI 5.2.3 / LibreOffice 26.2.3.2 extraction goldens. It is also the
  self-contained input for the strict public extraction benchmark.
- `synthetic/` — generated from scratch by [`scripts/gen_public_corpus.py`](../../scripts/gen_public_corpus.py);
  see [`PROVENANCE.md`](PROVENANCE.md) for the per-file purpose and origin.
  You own these outright (no third-party content). They deliberately carry the **unmodeled
  content a package-preserving editor must round-trip intact**: tracked changes (`w:ins`/`w:del`),
  content controls (`w:sdt`), text boxes (`mc:AlternateContent` + `w:txbxContent`), footnotes,
  comments, headers/footers, fields, hyperlinks, nested tables, unsupported object markers, tables, floating shape placement metadata, and an inline PNG image. Dedicated render fixtures activate run paint and hidden text, explicit top-level body tabs, table margins and RTL order, small-page keep pagination, equal-width columns, mixed Arabic/Hebrew direction, bounded `wrapTopAndBottom` flow, and `table-cell-lists.docx` body/direct-cell/nested-cell numbering, bullet fallback, and RTL table placement. Regenerate with
  `python scripts/gen_public_corpus.py` (deterministic — a no-op in git if unchanged).
- `vendored/` — a few real-producer files copied from permissively-licensed upstreams
  (CC0 / MIT only). See [`ATTRIBUTION.md`](ATTRIBUTION.md) for the source and license of each.

## What the validation checks

For every `.docx` here, rwml must:
1. **open** it (`Document::open`);
2. **match expected diagnostics** for manifest-listed synthetic fixtures (`Document::report`);
3. **no-op `open → save` is part-payload byte-stable** — every unmodeled part (footnotes,
   comments, the text box, tracked changes, headers, media, …) round-trips byte-for-byte;
4. **element-tree edit works** — `add_image_png` produces a package that re-opens (python-docx)
   with the new inline image, and the unmodeled content still survives.
5. with `--features render`, **native render reports match the public render
   manifest** and each synthetic fixture emits a non-empty PDF.

For LibreOffice A/B evidence, `scripts/render_validate.py` uses the bundled Noto
subsets by default and reports the retained page-1 aHash plus bounded all-page
aHash, foreground ink IoU, and explicit unmatched/capped page counts. Reference
PDFs remain temporary and are not committed.

Run it with the in-tree example + the python-docx checker:

```sh
cargo run --example validate_edit --features docx -- corpus/public <outdir>
python scripts/validate_edit_check.py corpus/public <outdir>
```
