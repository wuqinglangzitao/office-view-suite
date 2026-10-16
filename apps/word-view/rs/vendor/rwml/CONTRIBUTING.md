# Contributing to rwml

Thanks for your interest in improving `rwml` — a native Rust toolkit that reads,
exports, converts, and previews legacy `.doc` (Word 97–2003 binary, [MS-DOC]),
and reads, writes, package-preservingly edits, and previews modern `.docx`
(OOXML WordprocessingML, [ECMA-376]).

## Public source of truth

Every contribution must be reviewable from public repository material: the
code, tests, pull request, and any related issue. A private note, chat, or
planning artifact may help you work, but it cannot be required to understand or
reproduce the change. Summarize implementation-affecting decisions in the issue
or pull request.

Before starting work:

1. Search existing issues. A small, self-contained fix may go straight to a
   focused pull request. Open an issue first when the behavior, public API,
   compatibility boundary, or implementation direction needs discussion.
2. Implement a clear, bounded change directly. For broad or uncertain work,
   agree on the useful specification or architecture context in the public issue
   before writing a large patch.
3. Keep the change to one logical fix or feature and preserve any deliberately
   unsupported remainder in the issue or pull request.

Issues and pull requests are public. **Never upload confidential, private, or
proprietary documents, personal data, credentials, or other material you do not
have permission to redistribute.** Use a synthetic reproducer or a clearly
licensed public fixture instead. If the report may describe a vulnerability, do
not open a public issue; use [GitHub private vulnerability reporting] instead.

## Ground rules

- **No `unsafe`.** The crate is `#![forbid(unsafe_code)]`. Parsing untrusted
  files must never crash a host process — every byte access is bounds-checked,
  and malformed or hostile input must surface as an `Error` value or read-only
  diagnostics, never a panic.
- **Preserve before interpreting.** `.docx` edits mutate live WordprocessingML
  element trees and reserialize **only** touched parts; a no-op open→save is
  part-payload byte-stable. Don't regress package preservation.
- **Deterministic output.** Field evaluation and rendering must be deterministic
  and source-order stable.
- **Document every public item.** The crate denies `missing_docs`.
- **Keep dependencies minimal.** The legacy-`.doc` core depends only on `cfb`,
  `encoding_rs`, and `thiserror`; the default `docx` feature adds `zip`,
  `quick-xml`, and `flate2`, and the opt-in `render` feature adds `parley` and
  `krilla` (raising the MSRV to 1.92). New dependencies need a strong
  justification.
- **Follow the spec.** `.doc` behavior should trace to [MS-DOC] / [MS-CFB] and
  `.docx` behavior to [ECMA-376]; cite the relevant section in comments and the
  pull request when implementing format details.

## Optional AI and BMad tools

AI coding tools and the [BMad Method] are optional external aids. They are not
runtime or build dependencies, and a contributor must be able to use the normal
issue, test, and pull-request path without them. AI-assisted submissions have
the same standard as hand-written submissions: understand every changed line,
curate the result, run the relevant gates, and explain the reasoning publicly.

Do not include `_bmad-output/`, model transcripts, prompt dumps, memlogs, tool
caches, or personal AI settings in a pull request. An installer-generated
`_bmad/` tree or tool integration also does not belong in an unrelated feature
or fix; include project-level AI configuration only when a maintainer-approved
issue explicitly scopes that change. `_bmad-output/` is repository-ignored;
keep other personal tool files in your local `.git/info/exclude` or global Git
ignore rather than broadening the shared ignore rules. Check `git status --short`
before opening the pull request.

`rwml` does not require a shared BMad installation, `_bmad` configuration, or
`AGENTS.md`. If you use BMad locally, carry every decision needed for review
back into the public issue or pull request.

## Pull request workflow

`main` is protected, including for maintainers. Make changes on a focused topic
branch, rebase it on the current `origin/main`, and open a pull request; do not
push implementation or documentation commits directly to `main`. Pull requests
must be up to date and pass the required CI jobs before squash or rebase merge.

When an issue exists, link it and use `Closes #N` when the merged change fully
satisfies it. Keep the issue open until the pull request merges, and describe
any intentionally deferred compatibility ceiling instead of closing it early.

The pull request must state:

- **What** changed.
- **Why** the change is needed.
- **How** the implementation works at review-relevant depth.
- **Testing** performed, including the exact applicable commands below.
- **Compatibility boundary**, including producers, formats, views, or cases
  intentionally left unsupported.

## Validation by change area

The default and DOCX paths support Rust 1.85; the `render` feature requires Rust
1.92. Python 3 is required for corpus, hygiene, benchmark, and release tooling.
WASM validation additionally requires Node.js, the `wasm32-unknown-unknown`
target, and `wasm-bindgen-cli` 0.2.126. On Linux, render builds require
`libfontconfig1-dev` and `pkg-config`.

Run this common gate for every pull request:

```sh
python3 scripts/public_hygiene_audit.py
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo doc --no-deps
```

Then run every row that matches the files or behavior you changed. CI runs the
complete matrix on every pull request and remains authoritative.

| Change area | Additional local gate |
|---|---|
| Legacy `.doc` core, feature boundaries, or optional dependencies | `cargo test --all-targets --no-default-features` |
| Renderer or all-feature behavior | `cargo clippy --all-targets --all-features -- -D warnings`<br>`cargo test --all-targets --features render`<br>`cargo test --doc --all-features`<br>`cargo doc --no-deps --all-features` |
| Public Rust API, Rustdoc, or examples | `cargo semver-checks check-release --baseline-rev v0.1.3 --release-type patch --default-features`<br>`cargo semver-checks check-release --baseline-rev v0.1.3 --release-type patch --all-features`<br>`cargo test --doc --all-features`<br>`cargo doc --no-deps --all-features` |
| Public corpus or corpus manifests | `python3 scripts/gen_public_corpus.py --check`<br>`cargo test --release --test performance --locked -- --ignored --nocapture` |
| Python scripts or release/evidence tooling | `python3 -m unittest discover -s tests -p 'test_*.py'` |
| Fuzz targets or parsing/edit/render surfaces used by fuzzing | `cargo check --manifest-path fuzz/Cargo.toml --all-targets --locked` |
| Bundled fonts or `rwml-fonts/` | `cargo test --test bundled_fonts --all-features --locked`<br>`cargo test --manifest-path rwml-fonts/Cargo.toml`<br>`cargo package --manifest-path rwml-fonts/Cargo.toml` |
| WASM adapter or browser example | `cargo build --lib --target wasm32-unknown-unknown --locked`<br>`wasm-bindgen --target nodejs --out-dir target/wasm-node target/wasm32-unknown-unknown/debug/rwml.wasm`<br>`node tests/wasm_node_smoke.cjs target/wasm-node corpus/public/synthetic/comments.docx`<br>`node tests/wasm_demo_report_format.mjs` |
| Dependencies or `Cargo.lock` | `cargo audit` |
| MSRV, dependency declarations, or Cargo features | `cargo +1.85.0 build --no-default-features --locked`<br>`cargo +1.85.0 build --locked`<br>`cargo +1.92.0 build --features render --locked` |

Install `cargo-semver-checks` 0.48.0 before running the public-API row:

```sh
cargo install cargo-semver-checks --version 0.48.0 --locked
```

Install `cargo-audit` before running the dependency row if it is not already
available:

```sh
cargo install cargo-audit --version 0.22.1 --locked
```

## Release validation

Release publishing is tag-bound. Create the version-matching `v*` tag only
after the protected-branch CI result is green; the release workflow rejects
branch dispatches, verifies that the tag points at the exact workflow revision
and that revision is on protected `origin/main`, and compares the package against
the latest published `v0.1.3` API baseline. It then generates strict render and
extraction evidence, packages both crates, records checksums, and attaches the
exact packages and evidence to the GitHub Release.

Install the pinned `cargo-audit` command above, then use the non-publishing
preflight to reproduce the release evidence and package contract. It requires a
clean worktree and local LibreOffice; every generated file stays under the
ignored `target/release-preflight/` tree.

```sh
cargo audit
python3 scripts/release_preflight.py --output-dir target/release-preflight
```

The preflight creates a platform-correct virtual environment pinned to
`PyMuPDF==1.28.2`, `Pillow==12.3.0`, and `python-docx==1.2.0`; runs the Python
tooling tests without optional image-metric skips; generates and externally
opens all 21 package-preserving edit outputs; requires the exact three Apache
POI and three LibreOffice legacy oracles; and produces both crate archives plus
the revision-bound evidence manifest. It has no registry or GitHub write path.

## Tests and fixtures

- `.doc` unit tests build minimal valid OLE2/`.doc` files in memory so the
  parser is exercised end-to-end without private binary fixtures; `.docx` tests
  drive the public API (`Document::open`, `fields()`, `write_docx`, …) and the
  synthetic public corpus under `corpus/public/`.
- Use TDD (a red test first) for behavior changes. When fixing a real-world
  file, add a focused regression test that captures the structural quirk (piece
  table shape, encoding, control marks, element layout, package relationship,
  or rendering input) rather than committing the original document.
- Fixtures must be synthetic or clearly licensed for public redistribution and
  recorded consistently with the public corpus provenance rules.

## Scope

`rwml` maps both `.doc` and `.docx` into one shared `DocModel`. Legacy `.doc`
supports reading, inspection, export, conversion, and preview, but has no writer
or in-place editing path. Modern `.docx` additionally supports fresh writing and
bounded package-preserving edits. Most remaining work is deeper compatibility —
field semantics, layout/rendering fidelity, additional fixtures, and validation
depth — rather than new top-level APIs; see the roadmap in the
[README](README.md#roadmap). Preview-grade rendering is not a LibreOffice/Word
replacement, and layout-exact pagination is intentionally out of scope.

By participating, you agree to the repository's
[Code of Conduct](.github/CODE_OF_CONDUCT.md).

[GitHub private vulnerability reporting]: https://github.com/HyunjoJung/rwml/security/advisories/new
[BMad Method]: https://github.com/bmad-code-org/BMAD-METHOD
[MS-DOC]: https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-doc/ccd7b486-7881-484c-a137-51170af7cc22
[MS-CFB]: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/53989ce4-7b05-4f8d-829b-d08d6148375b
[ECMA-376]: https://ecma-international.org/publications-and-standards/standards/ecma-376/
