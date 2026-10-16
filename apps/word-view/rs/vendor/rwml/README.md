# rwml

**A native Rust toolkit for Microsoft Word documents.** `rwml` reads legacy
`.doc` and modern `.docx` into one document model, writes styled `.docx`, edits
existing `.docx` packages without rewriting untouched parts, and renders native
preview PDFs.

[![Crates.io](https://img.shields.io/crates/v/rwml.svg)](https://crates.io/crates/rwml)
[![Docs.rs](https://docs.rs/rwml/badge.svg)](https://docs.rs/rwml)
[![CI](https://github.com/HyunjoJung/rwml/actions/workflows/ci.yml/badge.svg)](https://github.com/HyunjoJung/rwml/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![MSRV](https://img.shields.io/badge/MSRV-1.85%20(render%201.92)-orange.svg)

The core library requires no JVM, Apache POI, Microsoft Office automation, or
subprocess. It is built for document pipelines that need to accept legacy
codepages, Korean cp949 documents, and untrusted input through bounded parsers
that return typed errors for malformed or unsupported files.

```sh
cargo add rwml@0.1.4
```

## What it does

| Input | Read | Create `.docx` | Edit in place | Export | Native PDF |
|---|:---:|:---:|:---:|---|:---:|
| `.doc` (Word 97–2003) | ✓ | ✓ conversion | — | text · Markdown · HTML | ✓ preview |
| `.docx` | ✓ | ✓ styled | ✓ package-preserving | text · Markdown · HTML | ✓ preview |
| `DocModel` built in Rust | — | ✓ | — | Markdown · HTML | ✓ preview |

One model ties the paths together. [`Document::open`] detects the format from
the bytes, and both readers produce the same [`DocModel`]. Exporters, the DOCX
writer, diagnostics, and the PDF renderer consume that model.

```text
 .doc  ┐                          ┌→ text · Markdown · HTML
 .docx ┼→ Document::open → DocModel ┼→ write_docx
 Rust  ┘                          ├→ package-preserving .docx edits
                                 └→ native preview PDF
```

### At a glance

| Release | Safety and toolchain | Public release evidence |
|---|---|---|
| `0.1.4` · MIT | no `unsafe` · core MSRV 1.85 · render MSRV 1.92 | 21 DOCX fixtures / 26 rendered pages · 3 generated DOC oracle fixtures · strict tag-bound manifest |

Version `0.1.4` is published on
[crates.io](https://crates.io/crates/rwml/0.1.4) and
[docs.rs](https://docs.rs/rwml/0.1.4/rwml/). Its exact crates, checksums,
public-hygiene result, extraction benchmark, render validation, and release
manifest are attached to the
[`v0.1.4` release](https://github.com/HyunjoJung/rwml/releases/tag/v0.1.4).

## Start using rwml

**Read** either Word format with the same API:

```rust
let bytes = std::fs::read("report.docx")?;
let doc = rwml::Document::open(&bytes)?;

println!("{}", doc.to_markdown());
println!("{}", doc.report().to_json());

let model = doc.model();
for block in &model.blocks {
    // Paragraph, Table, Image, PageBreak, or SectionBreak
}
```

For plain-text indexing, the shortest path is:

```rust
let text = rwml::extract_text(&std::fs::read("legacy.doc")?)?;
```

**Author** a fresh `.docx` from Rust data:

```rust
let model = rwml::DocBuilder::new()
    .title("Quarterly operations report")
    .heading(1, "Summary")
    .paragraph("Generated without Word, a JVM, or a template.")
    .numbered_list(["Open the source", "Build the model", "Write DOCX"])
    .build();

std::fs::write("report.docx", rwml::write_docx(&model))?;
```

**Edit** an existing `.docx` while retaining untouched package parts:

```rust
let mut doc = rwml::Document::open(&std::fs::read("template.docx")?)?;

doc.replace_body_text("DRAFT", "FINAL")?;
doc.fill_template_fields([
    ("client-name", "Acme & Co"),
    ("report-period", "2026 Q3"),
])?;

let touched = doc.edited_parts();
std::fs::write("final.docx", doc.save()?)?;
println!("updated package parts: {touched:?}");
```

**Inspect or convert** from the CLI:

```sh
cargo install rwml --version =0.1.4 --locked

rwml extract file.docx
rwml convert legacy.doc md
rwml diagnose file.docx
rwml to-docx legacy.doc converted.docx
```

PDF output is opt-in because it adds the shaping and PDF stack:

```sh
cargo add rwml@0.1.4 --features render
# or build the current checkout with deterministic bundled OFL font subsets;
# this source version automatically registers them for `to-pdf`
cargo install --path . --locked --features bundled-fonts

rwml to-pdf file.docx preview.pdf --report-json render.json
```

The renderer is a deterministic **preview/report renderer**, not a Word layout
engine. See [Compatibility and limits](#compatibility-and-limits) before using
it for pagination-sensitive output.

## Choose a path

Start with the smallest surface that matches the job.

| Goal | Start here | Cargo feature | Runnable example |
|---|---|---|---|
| Search or index Word files | `extract_text` | none for `.doc`; `docx` for `.docx` | `examples/extract.rs` |
| Inspect rich structure | `Document::open`, `Document::model`, `Document::report` | default `docx` | `examples/roundtrip.rs` |
| Author a new document | `DocBuilder`, `write_docx` | default `docx` | `examples/report.rs` |
| Convert `.doc` to `.docx` | `Document::to_docx` | default `docx` | `examples/to_docx.rs` |
| Preserve and edit an existing package | `Document::open`, edit methods, `save` | default `docx` | `examples/validate_edit.rs` |
| Render a preview PDF | `try_to_pdf`, `render_pdf` | `render` | `examples/to_pdf.rs` |
| Run a browser-side inspector | `rwml::wasm` | target-specific `wasm-bindgen` | `examples/wasm-demo/` |

API details live on [docs.rs](https://docs.rs/rwml/latest/rwml/), and the
[`examples/`](examples) directory keeps complete programs that can be run from
a checkout.

## Why rwml?

- **One model for old and new Word files.** Callers do not branch between a
  legacy parser and an OOXML parser before exporting or inspecting content.
- **Preserve before interpreting.** Package-preserving edits reserialize only
  touched XML or media parts; a no-op DOCX open/save keeps each part payload
  byte-stable.
- **Safe in-process parsing.** The crate forbids `unsafe`, bounds binary and XML
  work, does not resolve external XML entities, and guards decompression limits.
- **Explicit fidelity.** Diagnostics report preserved-but-unmodeled objects,
  cached field fallbacks, read-only edit reasons, and renderer warnings instead
  of silently claiming Word parity.
- **Native preview output.** The optional renderer shapes text, embeds subsetted
  fonts, emits selectable text, and handles Korean/CJK line breaking without
  launching Office or LibreOffice.
- **Small core dependency set.** A legacy-DOC-only build uses `cfb`,
  `encoding_rs`, and `thiserror`; DOCX and rendering remain additive features.

## Cargo features

| Feature | Default | Surface |
|---|:---:|---|
| `docx` | Yes | DOCX read/write, conversion, and package-preserving editing |
| `render` | No | Native PDF rendering with `parley` and `krilla`; MSRV 1.92 |
| `bundled-fonts` | No | `render` plus OFL Noto subsets for Korean/hanja, Arabic, and Hebrew |

Use `default-features = false` for the dependency-light legacy `.doc` reader:

```toml
rwml = { version = "0.1.4", default-features = false }
```

## Public validation

The release contract depends on redistributable inputs and machine-readable
evidence, not on private documents.

| Gate | Public input | Release check |
|---|---|---|
| DOCX parsing and preservation | 21 generated or permissively licensed documents | expected diagnostics, open/save part stability, bounded edit validation |
| Legacy DOC extraction | 3 generated Word 97–2003 documents | exact Apache POI 5.2.3 and LibreOffice 26.2.3.2 text oracles |
| PDF preview | the same 21-document manifest, 26 pages | text recall, page counts, visual summaries, zero skipped inputs |
| Public hygiene | source tree plus bounded Office-package inspection | filenames, metadata, text parts, corpus provenance, and license-clean inputs |

The inputs and their provenance are under
[`corpus/public/`](corpus/public). To run the fast public checks:

```sh
python3 scripts/gen_public_corpus.py --check
cargo test --test public_corpus
cargo test --features render --test public_corpus
```

Pull-request and protected-branch CI run the feature/MSRV matrix, formatting,
clippy, documentation, dependency audit, fuzz-target build, and public-corpus
checks. The tag-only release workflow adds release-mode performance checks,
extraction and rendering oracles, exact-package preflight, and release-manifest
validation. The longer parser/edit/render fuzz smoke is a separate on-demand
workflow. See [CONTRIBUTING.md](CONTRIBUTING.md#release-validation) to reproduce
the relevant gates.

## How it works

**DOCX** is an OPC ZIP package of XML parts. `rwml` parses the main document,
styles, numbering, relationships, notes, comments, headers and footers, fields,
tables, drawings, revisions, and supported metadata into the shared model and
diagnostic sidecars. The retained package and its live XML trees stay available
for bounded edits so unrelated parts survive.

**DOC** is an OLE2 compound file. `rwml` navigates the FIB and piece table,
decodes UTF-16 or the declared legacy codepage, and performs bounded passes over
character, paragraph, list, table, section, field, note, image, and style data.
Malformed, encrypted, and unsupported generations return typed errors rather
than partial ciphertext or panics.

**Writing** serializes a `DocModel` into a fresh OOXML package. **Rendering**
flows that model through deterministic page, paragraph, list, table, image, and
font-shaping stages before emitting PDF. Opening and resaving an existing DOCX
uses the separate preservation path rather than regenerating the package from
the lossy common model.

## Compatibility and limits

`rwml` deliberately distinguishes supported behavior from preserved content and
preview approximations.

| Area | Current contract | Important boundary |
|---|---|---|
| `.doc` | read, inspect, export, convert, preview | no in-place editing and no legacy `.doc` writer; Word 6/95 and encrypted files are rejected |
| `.docx` read | rich model plus diagnostics for major WordprocessingML surfaces | not every producer extension or layout-dependent field can be interpreted |
| Fresh `.docx` writing | styled text, lists, tables, images, links, comments, notes, revisions, fields, charts, sections, and metadata | fresh output contains the supported model, not unknown source parts |
| Package-preserving edit | focused text, field, control, comment, note, image, metadata, table, hyperlink, and bounded body-block operations | arbitrary rich nested edits and general cross-block rewriting are not exposed |
| PDF | selectable-text preview with page geometry, styles, lists, tables, images, links, and bounded floating-shape hints | not Word-exact pagination, floating-object reflow, or complete Office-Art rendering |
| WASM | extraction, Markdown/HTML, and diagnostics | the included browser example is an inspector, not an editing UI |

Unknown DOCX parts are retained by the preservation path when the package is
safe to edit. Unsupported charts, OLE objects, metafiles, floating shapes, and
layout-dependent fields may appear as diagnostics or preview placeholders. For
Word-exact or archival PDF conversion, use Word or LibreOffice as the renderer.

## Project map

| Path | Purpose |
|---|---|
| `src/docx/` | DOCX readers, fields, styles, numbering, revisions, and charts |
| `src/write/` | fresh DOCX generation |
| `src/render.rs` | native preview layout and PDF emission |
| `src/assemble.rs`, `src/chpx.rs`, `src/papx.rs`, `src/stsh.rs` | legacy DOC assembly and formatting |
| `tests/` | public API, preservation, format, rendering, and workflow contracts |
| `corpus/public/` | license-clean fixtures, manifests, oracles, and provenance |
| `examples/` | runnable read, write, edit, render, and WASM entry points |
| `scripts/` | deterministic corpus, evidence, hygiene, benchmark, and release tools |

## Roadmap

Work is chosen from reproducible files and focused regression tests.

| Area | Direction | Boundary to keep explicit |
|---|---|---|
| Read and fields | deepen bounded DOC/DOCX parsing and deterministic field evaluation | values requiring Word layout or external state remain cached-with-reason |
| Editing | add mutations only where preservation and rollback are unambiguous | no generic DOM editor or silent package regeneration |
| PDF preview | improve model-backed layout, tables, scripts, images, and sections | no promise of Word-exact pagination |
| RTL and complex scripts | expand verified mixed-script and table behavior | typography and punctuation must be fixture-backed |
| Corpus and interoperability | add license-clean producer fixtures and mature-tool oracles | private documents are never required to reproduce a public claim |

Open an [issue](https://github.com/HyunjoJung/rwml/issues) with a minimal file or
structural reproducer when a real document exposes a missing case.

## Contributing: one public path

Contributions should be understandable from the code, tests, pull request, and
any related public issue. Do not make a change depend on an unpublished internal
planning document; summarize any decision that affects the implementation in
the issue or PR.

```text
issue or change request
  ├─ clear, bounded change → implement directly
  └─ broad or uncertain change → prepare only the useful spec/architecture context
         ↓
focused code + regression test → local gate → small public PR
```

The normal path does not require an AI tool:

1. Search existing issues. A small, self-contained fix can go directly to a
   focused pull request; open an issue first when the behavior, public API, or
   compatibility boundary needs discussion.
2. Fork the repository and create a focused topic branch from current `main`.
3. Add a red regression test before changing behavior. Prefer a synthetic file
   or a minimal structural fixture over a private document.
4. Implement one logical change and run the relevant gate.
5. Open a pull request that links any related issue and explains any deliberately
   unsupported remainder.

### Optional BMad workflow

Contributors who use an AI coding tool may follow the
[BMad Method](https://github.com/bmad-code-org/BMAD-METHOD) as a proportional
planning and implementation path. It is optional, external to `rwml`, and is
not a runtime or build dependency.

```sh
# Requires Node.js 20.12+, Python 3.10+, and uv.
npx bmad-method install
```

- For a clear one-session issue, invoke `bmad-build` with the issue or change
  request directly.
- For a broad change, run `bmad-help` and add only the planning depth the work
  needs. Carry the resulting decisions into the public issue or PR rather than
  relying on a private artifact.

Keep local BMad working output and private AI transcripts out of the pull
request. The tests, code, PR description, and any related public issue must
contain every decision a reviewer needs.

This mirrors BMad's direct-versus-planned entry model while keeping the same
review standard for manual and AI-assisted contributions.

The common local gate is:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo doc --no-deps
```

Renderer, corpus, release, and full-feature changes have additional gates. Read
[CONTRIBUTING.md](CONTRIBUTING.md) before opening a PR; it defines safety,
preservation, dependency, test, and release requirements.

## Documentation and community

- [API documentation](https://docs.rs/rwml/latest/rwml/) — public types and methods
- [`examples/`](examples) — runnable programs
- [`corpus/public/`](corpus/public) — validation data and provenance
- [CHANGELOG.md](CHANGELOG.md) — release history and compatibility notes
- [CONTRIBUTING.md](CONTRIBUTING.md) — development and review gates
- [Code of Conduct](.github/CODE_OF_CONDUCT.md) — community expectations
- [GitHub Issues](https://github.com/HyunjoJung/rwml/issues) — bugs and proposals
- [Security policy](.github/SECURITY.md) — private vulnerability reporting

## License

Licensed under the [MIT License](LICENSE). Third-party dependency licenses are
listed in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md). Bundled font
subsets retain their upstream OFL licenses and provenance under
[`rwml-fonts/`](rwml-fonts).

## Trademarks

`rwml` takes its name from **WordprocessingML**, the ECMA-376 markup for
word-processing documents. It is an independent open-source project, not
affiliated with, authorized by, or endorsed by Microsoft. Microsoft, Microsoft
Word, and the `.doc` / `.docx` file formats are trademarks or registered
trademarks of Microsoft Corporation, referenced only to indicate format
compatibility.

The implementation is based on publicly documented [MS-DOC], [MS-CFB], and
OOXML specifications and contains no Microsoft source code.

[MS-DOC]: https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-doc/
[MS-CFB]: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/
[`Document::open`]: https://docs.rs/rwml/latest/rwml/struct.Document.html#method.open
[`DocModel`]: https://docs.rs/rwml/latest/rwml/struct.DocModel.html
