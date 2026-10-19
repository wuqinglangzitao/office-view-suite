import pathlib
import unittest


WORKFLOW = (
    pathlib.Path(__file__).resolve().parents[1] / ".github" / "workflows" / "ci.yml"
)


class CiWorkflowTests(unittest.TestCase):
    def test_ci_workflow_runs_public_hygiene_audit(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn("python3 scripts/public_hygiene_audit.py", text)

    def test_ci_workflow_runs_no_default_gate(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn("cargo test --all-targets", text)
        self.assertIn("cargo test --all-targets --no-default-features", text)
        self.assertIn("cargo test --all-targets --features render", text)

    def test_ci_workflow_runs_python_tooling_tests(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn(
            '"$RUNNER_TEMP/rwml-python-tools/bin/python" -m unittest '
            "discover -s tests -p 'test_*.py'",
            text,
        )

    def test_ci_installs_pinned_python_tools_before_tests_and_edit_validation(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        install = "- name: Install pinned Python validation tools"
        tests = "- name: Python release and evidence tooling tests"
        edits = "- name: Package-preserving edit interoperability"

        self.assertIn("PyMuPDF==1.28.2 Pillow==12.3.0 python-docx==1.2.0", text)
        self.assertIn('assert docx.__version__ == "1.2.0"', text)
        self.assertIn('assert pymupdf.__version__ == "1.28.2"', text)
        self.assertIn('assert PIL.__version__ == "12.3.0"', text)
        self.assertLess(text.index(install), text.index(tests))
        self.assertLess(text.index(tests), text.index(edits))
        self.assertIn(
            "cargo run --locked --example validate_edit --features docx", text
        )
        self.assertIn("scripts/validate_edit_check.py", text)

    def test_windows_python_tests_use_the_same_pinned_tool_environment(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        windows = text[text.index("  windows-portability:\n") : text.index("\n  wasm:\n")]

        self.assertIn("PyMuPDF==1.28.2 Pillow==12.3.0 python-docx==1.2.0", windows)
        self.assertLess(
            windows.index("- name: Install pinned Python validation tools"),
            windows.index("- name: Python release and evidence tooling tests"),
        )
        self.assertIn("python -m unittest discover -s tests", windows)

    def test_ci_workflow_runs_bundled_font_gate(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn(
            "cargo test --test bundled_fonts --all-features --locked", text
        )

    def test_ci_workflow_runs_release_mode_performance_gate(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn(
            "cargo test --release --test performance --locked -- --ignored --nocapture",
            text,
        )

    def test_ci_workflow_builds_and_executes_wasm_bindings(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        for command in [
            "cargo install wasm-bindgen-cli --version 0.2.126 --locked",
            "cargo build --lib --target wasm32-unknown-unknown --locked",
            "wasm-bindgen --target nodejs --out-dir target/wasm-node",
            "node tests/wasm_node_smoke.cjs target/wasm-node corpus/public/synthetic/comments.docx",
            "node tests/wasm_demo_report_format.mjs",
        ]:
            self.assertIn(command, text)

    def test_ci_workflow_checks_fuzz_targets_and_public_corpus_determinism(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn(
            "cargo check --manifest-path fuzz/Cargo.toml --all-targets --locked",
            text,
        )
        self.assertIn("python3 scripts/gen_public_corpus.py --check", text)

    def test_ci_workflow_builds_no_default_msrv_surface(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn("dtolnay/rust-toolchain@1.85.0", text)
        self.assertIn("dtolnay/rust-toolchain@1.92.0", text)
        self.assertIn("cargo build --no-default-features", text)

    def test_ci_workflow_checks_patch_compatible_public_api(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        check_job = text[text.index("  check:\n") : text.index("\n  wasm:\n")]

        self.assertIn("fetch-depth: 0", text)
        self.assertIn("dtolnay/rust-toolchain@1.92.0", check_job)
        self.assertNotIn("dtolnay/rust-toolchain@stable", check_job)
        self.assertIn(
            "cargo install cargo-semver-checks --version 0.48.0 --locked", text
        )
        self.assertIn(
            "cargo semver-checks check-release --baseline-rev v0.1.3 "
            "--release-type patch --default-features",
            text,
        )
        self.assertIn(
            "cargo semver-checks check-release --baseline-rev v0.1.3 "
            "--release-type patch --all-features",
            text,
        )

    def test_ci_workflow_runs_pinned_rustsec_audit(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        check_job = text[
            text.index("  check:\n") : text.index("\n  windows-portability:\n")
        ]

        self.assertIn(
            "cargo install cargo-audit --version 0.22.1 --locked", check_job
        )
        self.assertIn("run: cargo audit", check_job)

    def test_ci_workflow_runs_windows_portability_gate(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        windows_job = text[
            text.index("  windows-portability:\n") : text.index("\n  wasm:\n")
        ]

        for expected in [
            "runs-on: windows-latest",
            "actions/checkout@v7",
            "dtolnay/rust-toolchain@1.92.0",
            "actions/setup-python@v7",
            'python-version: "3.13"',
            "PyMuPDF==1.28.2 Pillow==12.3.0 python-docx==1.2.0",
            'assert docx.__version__ == "1.2.0"',
            'assert pymupdf.__version__ == "1.28.2"',
            'assert PIL.__version__ == "12.3.0"',
            "python scripts/gen_public_corpus.py --check",
            "PYTHONDONTWRITEBYTECODE: \"1\"",
            "python -m unittest discover -s tests -p 'test_*.py'",
        ]:
            self.assertIn(expected, windows_job)


if __name__ == "__main__":
    unittest.main()
