//! Robustness + structure harness: run the full pipeline (open → text → model →
//! markdown → html → images) over many `.doc`/`.docx` files, distinguishing a
//! clean [`rwml::Error`] from a real panic (the crate's panic-free contract), and
//! counting the structure produced.
//!
//! ```text
//! cargo run -p rwml --example robustness -- file1.doc file2.docx ...
//! ```

use std::panic::{catch_unwind, AssertUnwindSafe};

fn main() {
    std::panic::set_hook(Box::new(|_| {})); // silence default backtrace noise
    let (mut ok, mut err, mut panic) = (0u32, 0u32, 0u32);
    let (mut headings, mut tables, mut bolds, mut images) = (0u64, 0u64, 0u64, 0u64);

    for path in std::env::args().skip(1) {
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let result = catch_unwind(AssertUnwindSafe(|| match rwml::Document::open(&bytes) {
            Ok(doc) => {
                let _ = doc.text();
                let md = doc.to_markdown();
                let _ = doc.to_html();
                let imgs = doc.images().len();
                let h = md.lines().filter(|l| l.starts_with('#')).count();
                let t = md.matches("| --- ").count() + md.matches("<table>").count();
                let b = md.matches("**").count() / 2;
                Ok::<_, String>((h, t, b, imgs))
            }
            Err(e) => Err(format!("{e:?}")),
        }));
        match result {
            Ok(Ok((h, t, b, im))) => {
                ok += 1;
                headings += h as u64;
                tables += t as u64;
                bolds += b as u64;
                images += im as u64;
                println!("OK\t{path}\th={h} t={t} b={b} img={im}");
            }
            Ok(Err(e)) => {
                err += 1;
                println!("ERR\t{path}\t{e}");
            }
            Err(_) => {
                panic += 1;
                println!("PANIC\t{path}");
            }
        }
    }
    eprintln!(
        "=== files: ok={ok} clean_err={err} PANIC={panic} | structure: headings={headings} tables={tables} bold_runs={bolds} images={images} ==="
    );
}
