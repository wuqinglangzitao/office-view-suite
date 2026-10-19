//! Print a compact JSON feature report for a Word file.
//!
//! ```text
//! cargo run --example diagnose -- file.docx
//! ```

use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: diagnose <file.doc|.docx>");
        return ExitCode::from(64);
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("read {path}: {e}");
            return ExitCode::from(66);
        }
    };
    match rwml::Document::open(&bytes) {
        Ok(doc) => {
            println!("{}", doc.report().to_json());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{path}: {e}");
            ExitCode::from(1)
        }
    }
}
