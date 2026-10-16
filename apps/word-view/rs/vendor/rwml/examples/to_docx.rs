//! Convert any Word file (legacy `.doc` or modern `.docx`) to a clean `.docx`
//! through the shared document model — the write half of the unification story.
//!
//! ```text
//! cargo run --example to_docx -- input.doc [output.docx]
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(input) = args.next() else {
        eprintln!("usage: to_docx <input.doc|.docx> [output.docx]");
        return ExitCode::from(2);
    };
    let out = args.next().map(PathBuf::from).unwrap_or_else(|| {
        let mut p = PathBuf::from(&input);
        p.set_extension("out.docx");
        p
    });

    let bytes = match std::fs::read(&input) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("read {input}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let doc = match rwml::Document::open(&bytes) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("parse {input}: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = std::fs::write(&out, doc.to_docx()) {
        eprintln!("write {}: {e}", out.display());
        return ExitCode::FAILURE;
    }
    eprintln!("wrote {}", out.display());
    ExitCode::SUCCESS
}
