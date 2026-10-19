# Third-Party Licenses

`rwml` is distributed under the MIT License (see [`LICENSE`](LICENSE)). It links
the following third-party crates, all under permissive licenses compatible with
MIT.

## Direct dependencies (always)

| Crate | License | Repository |
| --- | --- | --- |
| [`cfb`](https://crates.io/crates/cfb) | MIT | mdsteele/rust-cfb |
| [`encoding_rs`](https://crates.io/crates/encoding_rs) | (Apache-2.0 OR MIT) AND BSD-3-Clause | hsivonen/encoding_rs |
| [`thiserror`](https://crates.io/crates/thiserror) | MIT OR Apache-2.0 | dtolnay/thiserror |

## `docx` feature (default)

| Crate | License | Repository |
| --- | --- | --- |
| [`zip`](https://crates.io/crates/zip) | MIT | zip-rs/zip2 |
| [`quick-xml`](https://crates.io/crates/quick-xml) | MIT | tafia/quick-xml |

## `render` feature

The PDF renderer pulls in the Linebender text/graphics stack and a PDF emitter.
All are permissively licensed (Apache-2.0 OR MIT unless noted).

| Crate | License | Repository |
| --- | --- | --- |
| [`parley`](https://crates.io/crates/parley) | Apache-2.0 OR MIT | linebender/parley |
| [`fontique`](https://crates.io/crates/fontique) | Apache-2.0 OR MIT | linebender/parley |
| [`swash`](https://crates.io/crates/swash) | Apache-2.0 OR MIT | dfrg/swash |
| [`krilla`](https://crates.io/crates/krilla) | MIT OR Apache-2.0 | LaurenzV/krilla |
| [`peniko`](https://crates.io/crates/peniko) | Apache-2.0 OR MIT | linebender/peniko |
| [`pdf-writer`](https://crates.io/crates/pdf-writer) | MIT OR Apache-2.0 | typst/pdf-writer |

These crates pull in further transitive dependencies (e.g. `skrifa`, `read-fonts`,
`zeno`, `tiny-skia-path`, `yoke`, `png`, `zune-jpeg`, `gif`, `image-webp`), each
under permissive (Apache-2.0/MIT/BSD/Zlib-class) licenses compatible with MIT.

License texts for every crate are available in their respective repositories and
in the `~/.cargo` registry cache after a build (`cargo about` / `cargo deny` can
regenerate this list). No third-party source is vendored into this crate.

## Format reference

This crate implements the publicly documented Microsoft Word binary format
([MS-DOC]) and the OLE2 Compound File Binary format ([MS-CFB]). No Microsoft
source code or proprietary material is used; only the open specifications.

[MS-DOC]: https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-doc/
[MS-CFB]: https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-cfb/
