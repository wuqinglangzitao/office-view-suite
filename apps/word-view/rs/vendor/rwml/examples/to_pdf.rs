//! Render any Word file (legacy `.doc` or modern `.docx`) to a native A4 PDF
//! through the shared document model. Requires the `render` feature.
//!
//! ```text
//! cargo run --example to_pdf --features render -- input.doc [output.pdf]
//! cargo run --example to_pdf --features render -- input.doc output.pdf --report-json report.json
//! cargo run --example to_pdf --features render -- input.doc output.pdf --fixed-fonts
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut positional = Vec::new();
    let mut report_json: Option<PathBuf> = None;
    let mut fixed_fonts = false;
    while let Some(arg) = args.next() {
        if arg == "--report-json" {
            let Some(path) = args.next() else {
                eprintln!(
                    "usage: to_pdf <input.doc|.docx> [output.pdf] [--report-json report.json] [--fixed-fonts]"
                );
                return ExitCode::from(2);
            };
            report_json = Some(PathBuf::from(path));
        } else if arg == "--fixed-fonts" {
            fixed_fonts = true;
        } else {
            positional.push(arg);
        }
    }
    let Some(input) = positional.first() else {
        eprintln!(
            "usage: to_pdf <input.doc|.docx> [output.pdf] [--report-json report.json] [--fixed-fonts]"
        );
        return ExitCode::from(2);
    };
    let out = positional.get(1).map(PathBuf::from).unwrap_or_else(|| {
        let mut p = PathBuf::from(input);
        p.set_extension("pdf");
        p
    });

    let bytes = match std::fs::read(input) {
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
    let fixed_fonts = fixed_fonts.then(|| {
        vec![
            rwml_fonts::noto_sans_kr_subset_with_hanja().to_vec(),
            rwml_fonts::noto_sans_arabic_subset().to_vec(),
            rwml_fonts::noto_sans_hebrew_subset().to_vec(),
        ]
    });
    let (pdf, report) = if report_json.is_some() {
        let rendered = match &fixed_fonts {
            Some(fonts) => doc.try_to_pdf_with_fonts_and_report(fonts),
            None => doc.try_to_pdf_with_report(),
        };
        match rendered {
            Ok(rendered) => (rendered.pdf, Some(rendered.report)),
            Err(e) => {
                eprintln!("render {input}: {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        let rendered = match &fixed_fonts {
            Some(fonts) => doc.try_to_pdf_with_fonts(fonts),
            None => doc.try_to_pdf(),
        };
        match rendered {
            Ok(pdf) => (pdf, None),
            Err(e) => {
                eprintln!("render {input}: {e}");
                return ExitCode::FAILURE;
            }
        }
    };
    if let Err(e) = std::fs::write(&out, &pdf) {
        eprintln!("write {}: {e}", out.display());
        return ExitCode::FAILURE;
    }
    if let (Some(path), Some(report)) = (report_json, report) {
        if let Err(e) = std::fs::write(&path, report.to_json()) {
            eprintln!("write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    }
    eprintln!("wrote {} ({} bytes)", out.display(), pdf.len());
    ExitCode::SUCCESS
}
