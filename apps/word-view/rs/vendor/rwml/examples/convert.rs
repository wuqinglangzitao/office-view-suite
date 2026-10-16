//! Convert a Word `.doc` or `.docx` to Markdown, HTML, or plain text (the input
//! format is auto-detected).
//!
//! ```text
//! cargo run -p rwml --example convert -- file.docx md
//! cargo run -p rwml --example convert -- file.doc html
//! cargo run -p rwml --example convert -- file.docx txt
//! ```

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .expect("usage: convert <file.doc|.docx> [md|html|txt]");
    let fmt = args.next().unwrap_or_else(|| "md".to_string());
    let bytes = std::fs::read(&path).expect("read file");
    let doc = rwml::Document::open(&bytes).expect("parse Word document");
    let out = match fmt.as_str() {
        "html" => doc.to_html(),
        "txt" => doc.text(),
        _ => doc.to_markdown(),
    };
    println!("{out}");
}
