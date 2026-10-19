import pathlib
import unittest


WORKFLOW = (
    pathlib.Path(__file__).resolve().parents[1]
    / ".github"
    / "workflows"
    / "release.yml"
)


def step_body(text, name):
    marker = f"      - name: {name}\n"
    start = text.index(marker)
    end = text.find("\n      - ", start + len(marker))
    return text[start:] if end == -1 else text[start:end]


class ReleaseWorkflowTests(unittest.TestCase):
    def test_release_workflow_is_tag_only(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertNotIn("workflow_dispatch", text)
        self.assertIn("tags: ['v*']", text)

    def test_local_release_preflight_is_non_publishing_and_complete(self):
        preflight = WORKFLOW.parents[2] / "scripts" / "release_preflight.py"
        text = preflight.read_text(encoding="utf-8")

        self.assertNotIn("cargo publish", text)
        self.assertNotIn("gh release", text)
        self.assertIn("CARGO_TARGET_DIR", text)
        self.assertIn("cargo-target", text)
        self.assertIn('COMMAND_ENV["PATH"]', text)
        self.assertIn('"-m", "venv"', text)
        self.assertIn('PYMUPDF_REQUIREMENT = "PyMuPDF==1.28.2"', text)
        self.assertIn('PILLOW_REQUIREMENT = "Pillow==12.3.0"', text)
        self.assertIn('PYTHON_DOCX_REQUIREMENT = "python-docx==1.2.0"', text)
        self.assertIn("JSONDecoder", text)
        for command in [
            "public_hygiene_audit.py",
            "gen_public_corpus.py",
            'CARGO, "audit"',
            "render_validate.py",
            "bench_vs_mature.py",
            "validate_edit_check.py",
            "release_manifest.py",
            "cargo package",
        ]:
            self.assertIn(command, text)
        self.assertIn('"fuzz/Cargo.toml"', text)
        for artifact in [
            "rwml-{version}.crate",
            "rwml-fonts-{version}.crate",
            "public-hygiene.json",
            "render-validation.json",
            "extract-benchmark.json",
            "rwml-release-manifest.json",
        ]:
            self.assertIn(artifact, text)

    def test_release_workflow_publishes_manifest_artifact(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn("python3 scripts/public_hygiene_audit.py", text)
        self.assertIn("python3 scripts/public_hygiene_audit.py --json", text)
        self.assertIn("cargo fmt --all -- --check", text)
        self.assertIn("cargo clippy --all-targets -- -D warnings", text)
        self.assertIn(
            "cargo clippy --all-targets --all-features -- -D warnings", text
        )
        self.assertIn("cargo test --all-targets --no-default-features", text)
        self.assertIn("cargo test --doc --all-features", text)
        self.assertIn("cargo doc --no-deps --all-features", text)
        self.assertIn(
            '"$RUNNER_TEMP/rwml-release-tools/bin/python" -m unittest '
            "discover -s tests -p 'test_*.py'",
            text,
        )
        self.assertIn("scripts/release_manifest.py", text)
        self.assertIn("--git-rev \"$GITHUB_SHA\"", text)
        self.assertIn(
            'crate_version=$(cargo metadata --no-deps --format-version 1', text
        )
        self.assertIn(
            'if [[ "$GITHUB_REF_TYPE" != "tag" ]] || [[ "$GITHUB_REF_NAME" != v* ]]; then',
            text,
        )
        self.assertIn('"$GITHUB_REF_NAME" != "v${crate_version}"', text)
        self.assertIn(
            'echo "RWML_VERSION=${crate_version}" >> "$GITHUB_ENV"', text
        )
        self.assertIn('--version "$RWML_VERSION"', text)
        self.assertNotIn('VERSION="${GITHUB_REF_NAME#v}"', text)
        self.assertIn("--release-policy public-release", text)
        self.assertIn("--enforce-policy-inputs", text)
        self.assertIn(
            "--hygiene-report target/release-evidence/public-hygiene.json", text
        )
        self.assertIn("--corpus-manifest corpus/public/MANIFEST.tsv", text)
        self.assertIn("--corpus-manifest corpus/public/RENDER_MANIFEST.tsv", text)
        self.assertIn("cargo test --all-targets --features render", text)
        self.assertIn(
            "cargo test --test bundled_fonts --all-features --locked", text
        )
        self.assertIn(
            "cargo test --release --test performance --locked -- --ignored --nocapture",
            text,
        )
        for command in [
            "cargo install wasm-bindgen-cli --version 0.2.126 --locked",
            "cargo build --lib --target wasm32-unknown-unknown --locked",
            "wasm-bindgen --target nodejs --out-dir target/wasm-node",
            "node tests/wasm_node_smoke.cjs target/wasm-node corpus/public/synthetic/comments.docx",
            "node tests/wasm_demo_report_format.mjs",
        ]:
            self.assertIn(command, text)
        self.assertIn(
            "cargo check --manifest-path fuzz/Cargo.toml --all-targets --locked",
            text,
        )
        self.assertIn("python3 scripts/gen_public_corpus.py --check", text)
        self.assertIn("target/release-evidence/public-hygiene.json", text)
        self.assertIn("target/release-evidence/rwml-release-manifest.json", text)
        self.assertIn("target/package/rwml-${RWML_VERSION}.crate", text)
        self.assertIn(
            "${{ runner.temp }}/rwml-release-assets/rwml-${{ env.RWML_VERSION }}.crate",
            text,
        )
        self.assertIn("actions/upload-artifact@v7", text)
        for artifact in [
            "rwml-${RWML_VERSION}.crate",
            "rwml-fonts-${RWML_VERSION}.crate",
            "public-hygiene.json",
            "render-validation.json",
            "extract-benchmark.json",
            "rwml-release-manifest.json",
        ]:
            self.assertIn(artifact, text)

    def test_release_workflow_logs_strict_render_evidence_on_failure(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        step = step_body(text, "Generate strict revision-bound evidence")

        self.assertIn("set +e", step)
        self.assertIn("render_status=$?", step)
        self.assertIn('cat "$evidence_dir/render-validation.json"', step)
        self.assertIn('exit "$render_status"', step)
        self.assertLess(
            step.index('cat "$evidence_dir/render-validation.json"'),
            step.index('exit "$render_status"'),
        )

    def test_release_installs_pinned_python_tools_before_tests_and_reuses_them(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        install = step_body(text, "Install pinned Python validation tools")
        verify = step_body(text, "Verify the crate builds, tests, and packages")
        evidence = step_body(text, "Generate strict revision-bound evidence")

        self.assertIn("PyMuPDF==1.28.2 Pillow==12.3.0 python-docx==1.2.0", install)
        self.assertIn('assert docx.__version__ == "1.2.0"', install)
        self.assertIn('assert pymupdf.__version__ == "1.28.2"', install)
        self.assertIn('assert PIL.__version__ == "12.3.0"', install)
        self.assertIn("rwml-release-tools/bin/python", verify)
        self.assertIn("rwml-release-tools/bin/python", evidence)
        self.assertNotIn("pip install", verify)
        self.assertNotIn("pip install", evidence)
        self.assertIn(
            "cargo run --locked --example validate_edit --features docx", verify
        )
        self.assertIn("scripts/validate_edit_check.py", verify)
        self.assertLess(
            text.index("- name: Install pinned Python validation tools"),
            text.index("- name: Verify the crate builds, tests, and packages"),
        )

    def test_release_and_preflight_run_pinned_rustsec_audit(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        preflight = (
            WORKFLOW.parents[2] / "scripts" / "release_preflight.py"
        ).read_text(encoding="utf-8")
        contributing = (
            WORKFLOW.parents[2] / "CONTRIBUTING.md"
        ).read_text(encoding="utf-8")

        self.assertIn(
            "cargo install cargo-audit --version 0.22.1 --locked", text
        )
        self.assertIn("run: cargo audit", text)
        self.assertIn('run([CARGO, "audit"])', preflight)
        self.assertIn(
            "cargo install cargo-audit --version 0.22.1 --locked", contributing
        )
        self.assertIn("\ncargo audit\n", contributing)

    def test_contributing_release_reproduction_uses_ignored_preflight_contract(self):
        contributing = (
            WORKFLOW.parents[2] / "CONTRIBUTING.md"
        ).read_text(encoding="utf-8")
        release_section = contributing[
            contributing.index("## Release validation") : contributing.index(
                "## Tests and fixtures"
            )
        ]

        self.assertIn(
            "python3 scripts/release_preflight.py --output-dir target/release-preflight",
            release_section,
        )
        self.assertNotIn("dist/", release_section)
        self.assertIn("python-docx==1.2.0", release_section)
        self.assertIn("all 21 package-preserving edit outputs", release_section)
        self.assertIn("exact three Apache", release_section)

    def test_release_identity_requires_tag_revision_to_be_on_origin_main(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        identity = step_body(text, "Verify release identity")

        self.assertIn("git fetch origin main:refs/remotes/origin/main", identity)
        self.assertIn(
            'git merge-base --is-ancestor "$GITHUB_SHA" origin/main', identity
        )
        self.assertIn("is not on protected origin/main", identity)

    def test_release_legacy_benchmark_requires_all_three_poi_and_lo_oracles(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        evidence = step_body(text, "Generate strict revision-bound evidence")
        preflight = (
            WORKFLOW.parents[2] / "scripts" / "release_preflight.py"
        ).read_text(encoding="utf-8")

        for token in [
            "--extract-bin",
            "--min-lo-recall-mean",
            "--min-scored",
            "--max-scored",
            "--min-lo-scored",
            "--max-lo-scored",
        ]:
            self.assertIn(token, evidence)
            self.assertIn(f'"{token}"', preflight)
        self.assertIn(
            "--min-scored 3 --max-scored 3 --min-lo-scored 3 --max-lo-scored 3",
            evidence,
        )

    def test_generated_release_evidence_stays_outside_the_package_source(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        gitignore = (WORKFLOW.parents[2] / ".gitignore").read_text(encoding="utf-8")

        self.assertIn("target/release-evidence", text)
        self.assertNotIn("dist/", text)
        self.assertIn("/target", gitignore.splitlines())

    def test_release_evidence_survives_packaging_and_is_validated_before_use(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        install = step_body(text, "Install pinned Python validation tools")
        evidence = step_body(text, "Generate strict revision-bound evidence")
        manifest = step_body(text, "Generate release manifest")
        upload = step_body(text, "Upload release manifest artifacts")
        create_release = step_body(text, "Create GitHub Release")

        self.assertIn(
            'evidence_dir="$RUNNER_TEMP/rwml-release-evidence"', evidence
        )
        self.assertIn("PyMuPDF==1.28.2 Pillow==12.3.0 python-docx==1.2.0", install)
        self.assertNotIn("target/release-evidence", evidence)
        self.assertIn(
            'python3 -m json.tool "$evidence_dir/render-validation.json"',
            evidence,
        )
        self.assertIn(
            'python3 -m json.tool "$evidence_dir/extract-benchmark.json"',
            evidence,
        )
        self.assertIn(
            'install -m 0644 "$RUNNER_TEMP/rwml-release-evidence/render-validation.json"',
            manifest,
        )
        self.assertIn(
            'install -m 0644 "$RUNNER_TEMP/rwml-release-evidence/extract-benchmark.json"',
            manifest,
        )
        self.assertIn('assets_dir="$RUNNER_TEMP/rwml-release-assets"', manifest)
        self.assertIn("python3 -m json.tool", manifest)
        self.assertIn("${{ runner.temp }}/rwml-release-assets", upload)
        self.assertIn("$RUNNER_TEMP/rwml-release-assets", create_release)

    def test_release_workflow_checks_patch_compatible_public_api(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        self.assertIn("fetch-depth: 0", text)
        self.assertIn("dtolnay/rust-toolchain@1.92.0", text)
        self.assertNotIn("dtolnay/rust-toolchain@stable", text)
        self.assertIn("components: rustfmt, clippy", text)
        self.assertIn("targets: wasm32-unknown-unknown", text)
        self.assertIn(
            "cargo install cargo-semver-checks --version 0.48.0 --locked", text
        )
        self.assertIn(
            "cargo semver-checks check-release --baseline-rev v0.1.3 "
            "--release-type patch --default-features",
            text,
        )

    def test_release_workflow_publishes_font_dependency_before_main_package(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        font_package = "cargo package --manifest-path rwml-fonts/Cargo.toml"
        font_publish_step = "- name: Publish bundled font crate dependency"
        main_package_step = "- name: Package main crate"
        font_publish = step_body(text, "Publish bundled font crate dependency")
        main_package = step_body(text, "Package main crate")
        main_identity = step_body(text, "Verify main crate registry identity")
        manifest = step_body(text, "Generate release manifest")
        upload = step_body(text, "Upload release manifest artifacts")
        main_publish = step_body(text, "Publish main crate")
        self.assertIn("cargo test --manifest-path rwml-fonts/Cargo.toml", text)
        self.assertNotIn(
            "cargo test --manifest-path rwml-fonts/Cargo.toml --locked", text
        )
        self.assertIn(font_package, text)
        self.assertIn("cargo package --list > /dev/null", text)
        self.assertIn(font_publish_step, text)
        self.assertIn('if [[ "$font_version" != "$RWML_VERSION" ]]', font_publish)
        self.assertIn("CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}", font_publish)
        self.assertIn("python3 scripts/publish_crate.py", font_publish)
        self.assertIn("--name rwml-fonts", font_publish)
        self.assertIn("--manifest-path rwml-fonts/Cargo.toml", font_publish)
        self.assertIn(
            'rwml-fonts/target/package/rwml-fonts-${font_version}.crate',
            font_publish,
        )
        self.assertIn(main_package_step, text)
        self.assertIn("for attempt in {1..12}", main_package)
        self.assertIn('cargo info "rwml-fonts@${RWML_VERSION}"', main_package)
        self.assertIn("registry_visible=true", main_package)
        package_commands = [
            line.strip() for line in main_package.splitlines() if line.strip() == "cargo package"
        ]
        self.assertEqual(package_commands, ["cargo package"])
        self.assertLess(main_package.index("cargo info"), main_package.index("cargo package"))
        self.assertIn("python3 scripts/publish_crate.py", main_identity)
        self.assertIn("--name rwml", main_identity)
        self.assertIn('target/package/rwml-${RWML_VERSION}.crate', main_identity)
        self.assertIn("--check-only", main_identity)
        self.assertIn("target/package/rwml-${RWML_VERSION}.crate", manifest)
        self.assertIn(
            "rwml-fonts/target/package/rwml-fonts-${RWML_VERSION}.crate", manifest
        )
        self.assertIn(
            "${{ runner.temp }}/rwml-release-assets/rwml-${{ env.RWML_VERSION }}.crate",
            upload,
        )
        self.assertIn(
            "${{ runner.temp }}/rwml-release-assets/"
            "rwml-fonts-${{ env.RWML_VERSION }}.crate",
            upload,
        )
        self.assertIn("CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_REGISTRY_TOKEN }}", main_publish)
        self.assertIn("python3 scripts/publish_crate.py", main_publish)
        self.assertIn("--name rwml", main_publish)
        self.assertIn('target/package/rwml-${RWML_VERSION}.crate', main_publish)
        self.assertNotIn("--token", text)
        self.assertLess(text.index(font_package), text.index(font_publish_step))
        self.assertLess(text.index(font_publish_step), text.index(main_package_step))
        ordered_steps = [
            "- name: Package main crate",
            "- name: Verify main crate registry identity",
            "- name: Generate release manifest",
            "- name: Upload release manifest artifacts",
            "- name: Publish main crate",
        ]
        positions = [text.index(step) for step in ordered_steps]
        self.assertEqual(positions, sorted(positions))
        self.assertIn(
            "cargo semver-checks check-release --baseline-rev v0.1.3 "
            "--release-type patch --all-features",
            text,
        )


if __name__ == "__main__":
    unittest.main()
