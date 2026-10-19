//! Round-trip self-test: read a Word file, write it back as `.docx`, re-read the
//! result, and report how much of the text survived (word-level recall) plus the
//! table count on each side. Used to validate the write layer at corpus scale.
//!
//! ```text
//! cargo run --example roundtrip -- input.doc
//! ```
//! Prints `PASS <name> recall=<r> words=<n> tbl=<a>/<b>` or `FAIL <name> <why>`.

use std::collections::HashSet;
use std::process::ExitCode;

use rwml::{Block, Document};

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn tables(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .map(|b| match b {
            Block::Table(t) => {
                1 + t
                    .rows
                    .iter()
                    .flat_map(|r| &r.cells)
                    .map(|c| tables(&c.blocks))
                    .sum::<usize>()
            }
            _ => 0,
        })
        .sum()
}

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: roundtrip <input.doc|.docx>");
        return ExitCode::from(2);
    };
    let name = path.rsplit(['/', '\\']).next().unwrap_or(&path).to_string();
    let Ok(bytes) = std::fs::read(&path) else {
        println!("FAIL {name} read-error");
        return ExitCode::FAILURE;
    };
    let d1 = match Document::open(&bytes) {
        Ok(d) => d,
        Err(e) => {
            println!("FAIL {name} open1: {e}");
            return ExitCode::FAILURE;
        }
    };
    let t1 = norm(&d1.text());
    let tbl1 = tables(&d1.model().blocks);

    let docx = d1.to_docx();
    let d2 = match Document::open(&docx) {
        Ok(d) => d,
        Err(e) => {
            println!("FAIL {name} reopen: {e}");
            return ExitCode::FAILURE;
        }
    };
    let t2 = norm(&d2.text());
    let tbl2 = tables(&d2.model().blocks);

    let w1: Vec<&str> = t1.split(' ').filter(|w| !w.is_empty()).collect();
    let set2: HashSet<&str> = t2.split(' ').filter(|w| !w.is_empty()).collect();
    let kept = w1.iter().filter(|w| set2.contains(*w)).count();
    let recall = if w1.is_empty() {
        1.0
    } else {
        kept as f64 / w1.len() as f64
    };
    println!(
        "PASS {name} recall={recall:.3} words={} tbl={tbl1}/{tbl2}",
        w1.len()
    );
    ExitCode::SUCCESS
}
