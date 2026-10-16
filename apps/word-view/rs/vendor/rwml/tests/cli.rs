#![cfg(feature = "docx")]

use std::io::Write;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn docx_fixture(parts: &[(&str, &str)]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let cursor = std::io::Cursor::new(&mut out);
        let mut zip = zip::ZipWriter::new(cursor);
        let opt = zip::write::SimpleFileOptions::default();
        for (name, body) in parts {
            zip.start_file(*name, opt).unwrap();
            zip.write_all(body.as_bytes()).unwrap();
        }
        zip.finish().unwrap();
    }
    out
}

fn plain_docx() -> Vec<u8> {
    docx_fixture(&[
        (
            "[Content_Types].xml",
            r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#,
        ),
        (
            "_rels/.rels",
            r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#,
        ),
        (
            "word/document.xml",
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Hello CLI</w:t></w:r></w:p><w:p><w:r><w:t>Second line</w:t></w:r></w:p></w:body></w:document>"#,
        ),
    ])
}

fn write_temp_docx(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "rwml-cli-{name}-{}-{stamp}.docx",
        std::process::id()
    ));
    std::fs::write(&path, bytes).unwrap();
    path
}

fn temp_output_path(name: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "rwml-cli-{name}-{}-{stamp}.docx",
        std::process::id()
    ))
}

#[cfg(feature = "render")]
fn temp_output_path_with_ext(name: &str, ext: &str) -> std::path::PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "rwml-cli-{name}-{}-{stamp}.{ext}",
        std::process::id()
    ))
}

fn run_rwml(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rwml"))
        .args(args)
        .output()
        .expect("run rwml cli")
}

#[cfg(feature = "render")]
fn run_rwml_render(args: &[&str]) -> std::process::Output {
    let out = run_rwml(args);
    if out.status.success()
        || !String::from_utf8_lossy(&out.stderr).contains("requires the render feature")
    {
        return out;
    }

    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "run",
            "--quiet",
            "--features",
            "render",
            "--bin",
            "rwml",
            "--",
        ])
        .args(args)
        .output()
        .expect("run render-enabled rwml cli")
}

#[test]
fn cli_extract_prints_plain_text() {
    let path = write_temp_docx("extract", &plain_docx());

    let out = run_rwml(&["extract", path.to_str().unwrap()]);

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        "Hello CLI\nSecond line"
    );
}

#[test]
fn cli_diagnose_prints_report_json() {
    let path = write_temp_docx("diagnose", &plain_docx());

    let out = run_rwml(&["diagnose", path.to_str().unwrap()]);

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let json = String::from_utf8(out.stdout).unwrap();
    assert!(json.starts_with(r#"{"format":"docx","#), "{json}");
    assert!(json.contains(r#""package_preserving":true"#), "{json}");
    assert!(json.ends_with("]}\n"), "{json}");
}

#[test]
fn cli_convert_supports_txt_markdown_and_html() {
    let path = write_temp_docx("convert", &plain_docx());
    let path = path.to_str().unwrap();

    let txt = run_rwml(&["convert", path, "txt"]);
    assert!(
        txt.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&txt.stderr)
    );
    assert_eq!(
        String::from_utf8(txt.stdout).unwrap(),
        "Hello CLI\nSecond line\n"
    );

    let md = run_rwml(&["convert", path, "md"]);
    assert!(
        md.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&md.stderr)
    );
    assert_eq!(
        String::from_utf8(md.stdout).unwrap(),
        "Hello CLI\n\nSecond line\n"
    );

    let html = run_rwml(&["convert", path, "html"]);
    assert!(
        html.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&html.stderr)
    );
    let html = String::from_utf8(html.stdout).unwrap();
    assert!(html.contains("<p>Hello CLI</p>"), "{html}");
    assert!(html.contains("<p>Second line</p>"), "{html}");
}

#[test]
fn cli_to_docx_writes_reopenable_docx() {
    let input = write_temp_docx("to-docx-input", &plain_docx());
    let output = temp_output_path("to-docx-output");

    let out = run_rwml(&["to-docx", input.to_str().unwrap(), output.to_str().unwrap()]);

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let converted = std::fs::read(&output).expect("converted output exists");
    let doc = rwml::Document::open(&converted).expect("converted docx reopens");
    assert_eq!(doc.text(), "Hello CLI\nSecond line");
}

#[cfg(feature = "render")]
#[test]
fn cli_to_pdf_writes_pdf_and_report_json() {
    let input = write_temp_docx("to-pdf-input", &plain_docx());
    let output = temp_output_path_with_ext("to-pdf-output", "pdf");
    let report = temp_output_path_with_ext("to-pdf-report", "json");

    let out = run_rwml_render(&[
        "to-pdf",
        input.to_str().unwrap(),
        output.to_str().unwrap(),
        "--report-json",
        report.to_str().unwrap(),
    ]);

    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let pdf = std::fs::read(&output).expect("pdf output exists");
    assert!(
        pdf.starts_with(b"%PDF"),
        "pdf output must start with PDF header"
    );

    let json = std::fs::read_to_string(&report).expect("report output exists");
    assert!(json.contains(r#""pages":"#), "{json}");
    assert!(json.contains(r#""warnings":"#), "{json}");
}

#[cfg(all(feature = "render", feature = "bundled-fonts"))]
#[test]
fn cli_to_pdf_registers_bundled_fonts_when_feature_is_enabled() {
    let bytes = plain_docx();
    let input = write_temp_docx("to-pdf-bundled-input", &bytes);
    let output = temp_output_path_with_ext("to-pdf-bundled-output", "pdf");

    let out = run_rwml(&["to-pdf", input.to_str().unwrap(), output.to_str().unwrap()]);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let fonts = vec![
        rwml_fonts::noto_sans_kr_subset_with_hanja().to_vec(),
        rwml_fonts::noto_sans_arabic_subset().to_vec(),
        rwml_fonts::noto_sans_hebrew_subset().to_vec(),
    ];
    let expected = rwml::Document::open(&bytes)
        .unwrap()
        .try_to_pdf_with_fonts(&fonts)
        .unwrap();
    assert_eq!(std::fs::read(output).unwrap(), expected);
}
