use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!("{}", usage());
        return ExitCode::from(64);
    };

    match command.as_str() {
        "extract" => {
            let Some(path) = args.next() else {
                eprintln!("usage: rwml extract <file.doc|.docx>");
                return ExitCode::from(64);
            };
            match read_file(&path).and_then(|bytes| rwml::extract_text(&bytes)) {
                Ok(text) => {
                    print!("{text}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{path}: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "diagnose" => {
            let Some(path) = args.next() else {
                eprintln!("usage: rwml diagnose <file.doc|.docx>");
                return ExitCode::from(64);
            };
            match read_file(&path).and_then(|bytes| rwml::Document::open(&bytes)) {
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
        "convert" => {
            let Some(path) = args.next() else {
                eprintln!("usage: rwml convert <file.doc|.docx> [txt|md|html]");
                return ExitCode::from(64);
            };
            let format = args.next().unwrap_or_else(|| "md".to_string());
            match read_file(&path).and_then(|bytes| rwml::Document::open(&bytes)) {
                Ok(doc) => match format.as_str() {
                    "txt" => {
                        println!("{}", doc.text());
                        ExitCode::SUCCESS
                    }
                    "md" => {
                        println!("{}", doc.to_markdown());
                        ExitCode::SUCCESS
                    }
                    "html" => {
                        println!("{}", doc.to_html());
                        ExitCode::SUCCESS
                    }
                    _ => {
                        eprintln!("unknown convert format {format:?}; expected txt, md, or html");
                        ExitCode::from(64)
                    }
                },
                Err(e) => {
                    eprintln!("{path}: {e}");
                    ExitCode::from(1)
                }
            }
        }
        "to-docx" => {
            let Some(input) = args.next() else {
                eprintln!("usage: rwml to-docx <input.doc|.docx> <output.docx>");
                return ExitCode::from(64);
            };
            let Some(output) = args.next() else {
                eprintln!("usage: rwml to-docx <input.doc|.docx> <output.docx>");
                return ExitCode::from(64);
            };
            match read_file(&input)
                .and_then(|bytes| rwml::Document::open(&bytes))
                .map(|doc| rwml::write_docx(&doc.model()))
                .and_then(|bytes| write_file(&output, &bytes))
            {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{input}: {e}");
                    ExitCode::from(1)
                }
            }
        }
        #[cfg(feature = "render")]
        "to-pdf" => {
            let Some(input) = args.next() else {
                eprintln!(
                    "usage: rwml to-pdf <input.doc|.docx> <output.pdf> [--report-json report.json]"
                );
                return ExitCode::from(64);
            };
            let Some(output) = args.next() else {
                eprintln!(
                    "usage: rwml to-pdf <input.doc|.docx> <output.pdf> [--report-json report.json]"
                );
                return ExitCode::from(64);
            };
            let mut report_json = None;
            while let Some(arg) = args.next() {
                if arg == "--report-json" {
                    let Some(path) = args.next() else {
                        eprintln!("usage: rwml to-pdf <input.doc|.docx> <output.pdf> [--report-json report.json]");
                        return ExitCode::from(64);
                    };
                    report_json = Some(path);
                } else {
                    eprintln!("unknown to-pdf option {arg:?}");
                    return ExitCode::from(64);
                }
            }

            match render_pdf(&input, &output, report_json.as_deref()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{input}: {e}");
                    ExitCode::from(1)
                }
            }
        }
        #[cfg(not(feature = "render"))]
        "to-pdf" => {
            eprintln!("rwml to-pdf requires the render feature");
            ExitCode::from(64)
        }
        "-h" | "--help" | "help" => {
            println!("{}", usage());
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("unknown command {command:?}\n{}", usage());
            ExitCode::from(64)
        }
    }
}

fn usage() -> &'static str {
    "usage: rwml <command> [args]\n\ncommands:\n  extract <file.doc|.docx>\n  diagnose <file.doc|.docx>\n  convert <file.doc|.docx> [txt|md|html]\n  to-docx <input.doc|.docx> <output.docx>\n  to-pdf <input.doc|.docx> <output.pdf> [--report-json report.json] (requires render)"
}

fn read_file(path: &str) -> rwml::Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| rwml::Error::Docx(format!("read {path}: {e}")))
}

fn write_file(path: &str, bytes: &[u8]) -> rwml::Result<()> {
    std::fs::write(path, bytes).map_err(|e| rwml::Error::Docx(format!("write {path}: {e}")))
}

#[cfg(feature = "render")]
fn render_pdf(input: &str, output: &str, report_json: Option<&str>) -> rwml::Result<()> {
    let doc = read_file(input).and_then(|bytes| rwml::Document::open(&bytes))?;
    #[cfg(feature = "bundled-fonts")]
    let fonts = vec![
        rwml_fonts::noto_sans_kr_subset_with_hanja().to_vec(),
        rwml_fonts::noto_sans_arabic_subset().to_vec(),
        rwml_fonts::noto_sans_hebrew_subset().to_vec(),
    ];
    #[cfg(not(feature = "bundled-fonts"))]
    let fonts: Vec<Vec<u8>> = Vec::new();

    if let Some(report_path) = report_json {
        let rendered = doc.try_to_pdf_with_fonts_and_report(&fonts)?;
        write_file(output, &rendered.pdf)?;
        let json = rendered.report.to_json();
        write_file(report_path, json.as_bytes())?;
    } else {
        let pdf = doc.try_to_pdf_with_fonts(&fonts)?;
        write_file(output, &pdf)?;
    }
    Ok(())
}
