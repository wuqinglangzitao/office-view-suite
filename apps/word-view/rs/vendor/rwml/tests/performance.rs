#![cfg(feature = "docx")]

use std::fs;
use std::hint::black_box;
use std::path::Path;
use std::time::{Duration, Instant};

use rwml::Document;

const ITERATIONS: usize = 25;
const EXPECTED_DOCUMENTS: usize = 21;
const MAX_ELAPSED: Duration = Duration::from_secs(10);

#[test]
#[ignore = "release-mode performance gate"]
fn public_corpus_parse_and_report_stays_within_release_smoke_budget() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("corpus/public");
    let manifest = fs::read_to_string(root.join("MANIFEST.tsv")).expect("read corpus manifest");
    let corpus: Vec<_> = manifest
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("path\t")
        })
        .map(|line| {
            let relative = line.split('\t').next().expect("manifest path");
            let bytes = fs::read(root.join(relative))
                .unwrap_or_else(|error| panic!("read {relative}: {error}"));
            (relative.to_owned(), bytes)
        })
        .collect();

    assert_eq!(corpus.len(), EXPECTED_DOCUMENTS);
    let input_bytes: usize = corpus.iter().map(|(_, bytes)| bytes.len()).sum();
    let operations = corpus.len() * ITERATIONS;
    let mut report_bytes = 0usize;
    let started = Instant::now();

    for _ in 0..ITERATIONS {
        for (relative, bytes) in &corpus {
            let document =
                Document::open(bytes).unwrap_or_else(|error| panic!("open {relative}: {error}"));
            report_bytes += black_box(document.report().to_json()).len();
        }
    }

    let elapsed = started.elapsed();
    let operations_per_second = operations as f64 / elapsed.as_secs_f64();
    eprintln!(
        "performance_smoke documents={} iterations={} operations={} input_bytes={} report_bytes={} elapsed_ms={} operations_per_second={:.1}",
        corpus.len(),
        ITERATIONS,
        operations,
        input_bytes,
        report_bytes,
        elapsed.as_millis(),
        operations_per_second,
    );
    assert!(
        elapsed <= MAX_ELAPSED,
        "public corpus parse/report smoke took {elapsed:?}, exceeding {MAX_ELAPSED:?}"
    );
}
