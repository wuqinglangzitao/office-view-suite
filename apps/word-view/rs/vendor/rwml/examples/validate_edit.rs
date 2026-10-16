//! Package-preserving edit validator: for every `.docx` in `<indir>`, write a
//! passthrough-save copy to `<outdir>/pass` and an element-tree image-insert copy
//! to `<outdir>/bimg`. If `<indir>/MANIFEST.tsv` exists, it is authoritative and
//! must exactly match the recursively discovered `.docx` files.
//!
//! The companion `scripts/validate_edit_check.py` checks every output opens in
//! python-docx (stricter OPC validation than rwml's own reader), that passthrough
//! copies are byte-identical per part, and that image-insert copies contain an
//! inline image. Both commands fail closed on an empty or partial run.
//!
//! ```text
//! cargo run --example validate_edit --features docx -- <indir> <outdir>
//! python scripts/validate_edit_check.py <indir> <outdir>
//! ```

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;

use rwml::Document;

const MANIFEST_NAME: &str = "MANIFEST.tsv";

#[derive(Debug)]
struct InputDocument {
    relative: PathBuf,
    source: PathBuf,
}

#[derive(Debug, Default)]
struct ValidationCounts {
    expected: usize,
    completed: usize,
    input_fail: usize,
    open_fail: usize,
    pass_fail: usize,
    image_fail: usize,
}

fn is_safe_relative_docx(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path.extension().and_then(|extension| extension.to_str()) == Some("docx")
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn collect_docx(root: &Path, current: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(current)
        .map_err(|error| format!("read directory {}: {error}", current.display()))?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("read entry in {}: {error}", current.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("inspect {}: {error}", path.display()))?;
        if file_type.is_dir() {
            collect_docx(root, &path, out)?;
        } else if file_type.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("docx")
        {
            let relative = path
                .strip_prefix(root)
                .map_err(|error| format!("relativize {}: {error}", path.display()))?
                .to_path_buf();
            out.push(relative);
        }
    }
    Ok(())
}

fn manifest_docx(root: &Path, manifest: &Path) -> Result<Vec<PathBuf>, String> {
    let text = fs::read_to_string(manifest)
        .map_err(|error| format!("read {}: {error}", manifest.display()))?;
    let mut paths = Vec::new();
    let mut seen = BTreeSet::new();
    for (line_index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("path\t") {
            continue;
        }
        let raw = line.split('\t').next().unwrap_or_default();
        let relative = PathBuf::from(raw);
        if !is_safe_relative_docx(&relative) {
            return Err(format!(
                "{}:{} has an invalid DOCX path: {raw:?}",
                manifest.display(),
                line_index + 1
            ));
        }
        if !seen.insert(relative.clone()) {
            return Err(format!(
                "{}:{} repeats DOCX path: {raw}",
                manifest.display(),
                line_index + 1
            ));
        }
        let source = root.join(&relative);
        if !source.is_file() {
            return Err(format!(
                "{}:{} references missing DOCX: {}",
                manifest.display(),
                line_index + 1,
                source.display()
            ));
        }
        paths.push(relative);
    }
    if paths.is_empty() {
        return Err(format!("{} contains no DOCX inputs", manifest.display()));
    }
    paths.sort();
    Ok(paths)
}

fn discover_documents(root: &Path) -> Result<Vec<InputDocument>, String> {
    if !root.is_dir() {
        return Err(format!(
            "input directory does not exist: {}",
            root.display()
        ));
    }

    let mut discovered = Vec::new();
    collect_docx(root, root, &mut discovered)?;
    discovered.sort();
    if discovered.is_empty() {
        return Err(format!("no DOCX inputs found under {}", root.display()));
    }

    let manifest = root.join(MANIFEST_NAME);
    let selected = if manifest.is_file() {
        let listed = manifest_docx(root, &manifest)?;
        let listed_set: BTreeSet<_> = listed.iter().cloned().collect();
        let discovered_set: BTreeSet<_> = discovered.iter().cloned().collect();
        if listed_set != discovered_set {
            let missing: Vec<_> = listed_set.difference(&discovered_set).collect();
            let unlisted: Vec<_> = discovered_set.difference(&listed_set).collect();
            return Err(format!(
                "{} does not exactly match the recursive DOCX inventory; missing={missing:?} unlisted={unlisted:?}",
                manifest.display()
            ));
        }
        listed
    } else {
        discovered
    };

    Ok(selected
        .into_iter()
        .map(|relative| InputDocument {
            source: root.join(&relative),
            relative,
        })
        .collect())
}

fn write_output(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("output has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create output directory {}: {error}", parent.display()))?;
    fs::write(path, bytes).map_err(|error| format!("write {}: {error}", path.display()))
}

fn validate_edits(indir: &Path, outdir: &Path) -> Result<ValidationCounts, String> {
    let documents = discover_documents(indir)?;
    let png: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x03, 0x08, 0x02, 0x00, 0x00, 0x00, 0x36,
        0x88, 0x49, 0xD6, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0x60,
        0xC0, 0x02, 0x00, 0x00, 0x15, 0x00, 0x01, 0x39, 0xC1, 0xE0, 0x23, 0x00, 0x00, 0x00, 0x00,
        0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    let mut counts = ValidationCounts {
        expected: documents.len(),
        ..ValidationCounts::default()
    };

    for document in documents {
        let label = document.relative.display();
        let bytes = match fs::read(&document.source) {
            Ok(bytes) => bytes,
            Err(error) => {
                counts.input_fail += 1;
                eprintln!("INPUT-FAIL {label}: {error}");
                continue;
            }
        };

        let pass_ok = match Document::open(&bytes) {
            Ok(doc) => match doc.save() {
                Ok(saved) => {
                    let output = outdir.join("pass").join(&document.relative);
                    match write_output(&output, &saved) {
                        Ok(()) => true,
                        Err(error) => {
                            counts.pass_fail += 1;
                            eprintln!("PASS-WRITE-FAIL {label}: {error}");
                            false
                        }
                    }
                }
                Err(error) => {
                    counts.pass_fail += 1;
                    eprintln!("PASS-SAVE-FAIL {label}: {error}");
                    false
                }
            },
            Err(error) => {
                counts.open_fail += 1;
                eprintln!("OPEN-FAIL {label}: {error}");
                false
            }
        };

        let image_ok = match Document::open(&bytes) {
            Ok(mut doc) => match doc
                .add_image_png(png, "rwmlimg.png")
                .and_then(|_| doc.save())
            {
                Ok(saved) => {
                    let output = outdir.join("bimg").join(&document.relative);
                    match write_output(&output, &saved) {
                        Ok(()) => true,
                        Err(error) => {
                            counts.image_fail += 1;
                            eprintln!("BIMG-WRITE-FAIL {label}: {error}");
                            false
                        }
                    }
                }
                Err(error) => {
                    counts.image_fail += 1;
                    eprintln!("BIMG-EDIT-FAIL {label}: {error}");
                    false
                }
            },
            Err(error) => {
                counts.image_fail += 1;
                eprintln!("BIMG-OPEN-FAIL {label}: {error}");
                false
            }
        };

        if pass_ok && image_ok {
            counts.completed += 1;
        }
    }

    println!(
        "expected={} completed={} input_fail={} open_fail={} pass_fail={} image_fail={}",
        counts.expected,
        counts.completed,
        counts.input_fail,
        counts.open_fail,
        counts.pass_fail,
        counts.image_fail
    );
    if counts.completed != counts.expected
        || counts.input_fail != 0
        || counts.open_fail != 0
        || counts.pass_fail != 0
        || counts.image_fail != 0
    {
        return Err("package-preserving edit generation failed".to_owned());
    }
    Ok(counts)
}

fn usage(program: &OsString) {
    eprintln!(
        "usage: {} <input-directory> <output-directory>",
        Path::new(program).display()
    );
}

fn main() -> ExitCode {
    let mut args = std::env::args_os();
    let program = args
        .next()
        .unwrap_or_else(|| OsString::from("validate_edit"));
    let Some(indir) = args.next() else {
        usage(&program);
        return ExitCode::from(64);
    };
    let Some(outdir) = args.next() else {
        usage(&program);
        return ExitCode::from(64);
    };
    if args.next().is_some() {
        usage(&program);
        return ExitCode::from(64);
    }

    match validate_edits(Path::new(&indir), Path::new(&outdir)) {
        Ok(_) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("validate_edit: {error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_directory(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rwml-validate-edit-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create test directory");
        path
    }

    #[test]
    fn public_manifest_discovers_exactly_twenty_one_nested_documents() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/public");
        let documents = discover_documents(&root).expect("discover public corpus");

        assert_eq!(documents.len(), 21);
        assert!(documents
            .iter()
            .all(|document| document.relative.components().count() >= 2));
    }

    #[test]
    fn manifest_must_match_recursive_inventory() {
        let root = temporary_directory("manifest-mismatch");
        fs::create_dir_all(root.join("synthetic")).expect("create corpus directory");
        fs::write(root.join("synthetic/listed.docx"), b"listed").expect("write listed");
        fs::write(root.join("synthetic/unlisted.docx"), b"unlisted").expect("write unlisted");
        fs::write(root.join(MANIFEST_NAME), "synthetic/listed.docx\t0\n").expect("write manifest");

        let error = discover_documents(&root).expect_err("unlisted input must fail");
        assert!(error.contains("does not exactly match"));
        fs::remove_dir_all(root).expect("remove test directory");
    }

    #[test]
    fn empty_input_tree_is_rejected() {
        let root = temporary_directory("empty");

        let error = discover_documents(&root).expect_err("empty corpus must fail");
        assert!(error.contains("no DOCX inputs"));
        fs::remove_dir_all(root).expect("remove test directory");
    }
}
