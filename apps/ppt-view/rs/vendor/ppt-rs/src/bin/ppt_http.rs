//! ppt_http - REST API server for ppt-rs
//!
//! ```bash
//! cargo run --features http --bin ppt_http
//! cargo run --features http --bin ppt_http -- --host 0.0.0.0 --port 3000
//! ```

#[tokio::main]
async fn main() {
    let mut host = "127.0.0.1".to_string();
    let mut port = 8080u16;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => {
                host = args.next().unwrap_or_else(|| {
                    eprintln!("ppt_http: --host requires a value");
                    std::process::exit(1);
                });
            }
            "--port" => {
                let value = args.next().unwrap_or_else(|| {
                    eprintln!("ppt_http: --port requires a value");
                    std::process::exit(1);
                });
                port = value.parse().unwrap_or_else(|_| {
                    eprintln!("ppt_http: invalid port '{value}'");
                    std::process::exit(1);
                });
            }
            "--help" | "-h" => {
                eprintln!(
                    "Usage: ppt_http [--host HOST] [--port PORT]\n\
                     \n\
                     Defaults: host=127.0.0.1 port=8080\n\
                     Endpoints: GET /health, POST /v1/generate, /v1/markdown, /v1/validate,\n\
                                POST /v1/export/markdown, /v1/export/html, /v1/diff"
                );
                return;
            }
            other => {
                eprintln!("ppt_http: unknown argument '{other}' (try --help)");
                std::process::exit(1);
            }
        }
    }

    let addr = format!("{host}:{port}");
    if let Err(e) = ppt_rs::http::run_server(&addr).await {
        eprintln!("ppt_http: {e}");
        std::process::exit(1);
    }
}
