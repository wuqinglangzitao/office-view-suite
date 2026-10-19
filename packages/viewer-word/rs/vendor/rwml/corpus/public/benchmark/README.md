# Public legacy extraction benchmark

This directory contains three generated Word 97-2003 `.doc` fixtures and the
plain-text outputs produced from those exact binaries by Apache POI 5.2.3 and
LibreOffice 26.2.3.2. It is a small, license-clean baseline for the public
release extraction gate; larger real-world corpora remain opt-in and local.

The `.doc` files were exported with LibreOffice's `MS Word 97` filter from the
repository-owned synthetic `.docx` files listed in the corpus attribution. The
goldens use the layout expected by `scripts/bench_vs_mature.py`:

- `sample/*.doc` - generated legacy fixtures;
- `sample-poi/*.poi.txt` - `WordExtractor.getText()` output;
- `sample-lo/*.txt` - LibreOffice `Text` filter output;
- `LEGACY_MANIFEST.tsv` - exact rwml report statistics and warning classes.

Run the release-strength public comparison with:

```sh
cargo build --release --example extract --locked
python3 scripts/bench_vs_mature.py --corpus corpus/public/benchmark --json \
  --version 0.1.1 --git-rev "$(git rev-parse HEAD)" \
  --min-poi-recall-mean 0.95 --min-poi-f1-mean 0.95 \
  --max-errors 0 --min-scored 1 \
  --output target/release-evidence/0.1.1/extract-benchmark.json
```
