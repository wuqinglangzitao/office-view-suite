# rwml WASM Inspector

This is a static browser demo for the `rwml::wasm` read/report adapter. It loads
a local `.doc` or `.docx` file, calls the same Rust core through the
`wasm-bindgen` web target, and displays plain text, Markdown, HTML preview, JSON
diagnostics, observed feature markers, and warnings.

It is not an editing UI. The current M8 surface is intentionally read-only until
browser editing has the same transaction and diagnostics discipline as the
native package-preserving editor.

## Build

Install `wasm-pack`, then build the crate for the web target from the repository
root:

```sh
wasm-pack build --target web --out-dir examples/wasm-demo/pkg
```

## Run

Serve the demo directory over HTTP:

```sh
cd examples/wasm-demo
python3 -m http.server 8080
```

Open `http://localhost:8080/` and choose a `.doc` or `.docx` fixture. The demo
uses `Document::open`, the normal text/Markdown/HTML exporters, and
`DocumentReport::to_json()` through the WASM adapter; it does not contain a
browser-specific parser.
