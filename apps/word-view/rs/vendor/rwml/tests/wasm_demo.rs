use std::fs;
use std::path::Path;

#[test]
fn wasm_demo_is_a_read_only_browser_surface_over_core_exports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let html_path = root.join("examples/wasm-demo/index.html");
    let readme_path = root.join("examples/wasm-demo/README.md");

    let html = fs::read_to_string(&html_path).expect("wasm demo index.html exists");
    let readme = fs::read_to_string(&readme_path).expect("wasm demo README exists");

    for required in [
        r#"type="file""#,
        r#"accept=".doc,.docx,application/msword,application/vnd.openxmlformats-officedocument.wordprocessingml.document""#,
        "init(",
        "extractText(",
        "markdown(",
        "html(",
        "reportJson(",
        "featureList",
        "warningList",
        "previewFrame",
    ] {
        assert!(
            html.contains(required),
            "wasm demo missing required marker {required:?}"
        );
    }

    for forbidden in [
        "save(",
        "replace_body_text",
        "set_field_result",
        "accept_all_revisions",
        "reject_all_revisions",
        "set_hyperlink_target",
        "add_comment_on_text",
    ] {
        assert!(
            !html.contains(forbidden),
            "wasm demo must remain read-only before edit UI hardening: {forbidden}"
        );
    }

    assert!(
        readme.contains("wasm-pack build --target web")
            && readme.contains("python3 -m http.server")
            && readme.contains("not an editing UI"),
        "wasm demo README should document local build/run and read-only scope"
    );
}
