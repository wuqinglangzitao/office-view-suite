import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest
import zipfile
from contextlib import contextmanager
from io import BytesIO, StringIO


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "scripts" / "public_hygiene_audit.py"
SPEC = importlib.util.spec_from_file_location("public_hygiene_audit", SCRIPT)
public_hygiene_audit = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = public_hygiene_audit
SPEC.loader.exec_module(public_hygiene_audit)


@contextmanager
def audit_root(path: pathlib.Path):
    old_repo = public_hygiene_audit.REPO
    public_hygiene_audit.REPO = path
    try:
        yield
    finally:
        public_hygiene_audit.REPO = old_repo


class PublicHygieneAuditTests(unittest.TestCase):
    def test_path_audit_flags_non_public_corpus_and_domain_trace_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            private_fixture = root / "corpus" / "private" / "sample.txt"
            public_fixture = root / "corpus" / "public" / "synthetic" / "sample.txt"
            domain_trace = root / "docs" / ("kr" + "-bid-notes.md")

            with audit_root(root):
                findings = public_hygiene_audit.audit_paths(
                    [private_fixture, public_fixture, domain_trace]
                )

        self.assertEqual(
            [(finding.path, finding.kind) for finding in findings],
            [
                ("corpus/private/sample.txt", "non_public_corpus_file"),
                ("docs/" + "kr" + "-bid-notes.md", "kr_bid_trace"),
            ],
        )

    def test_text_audit_flags_secrets_paths_and_private_corpus_defaults(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            doc = root / "README.md"
            doc.write_text(
                "\n".join(
                    [
                        "Compatibility/private fields are Word field kinds.",
                        "token=" + "sk-" + ("A" * 32),
                        "path=/Users/" + "alice/Documents/private.doc",
                        "export " + "RWML_" + "BENCH_CORPUS" + "=/tmp/local-corpus",
                    ]
                ),
                encoding="utf-8",
            )

            with audit_root(root):
                findings = public_hygiene_audit.audit_text_file(doc)

        self.assertEqual(
            [(finding.line, finding.kind) for finding in findings],
            [
                (2, "openai_api_key"),
                (3, "mac_home_path"),
                (4, "private_corpus_default"),
            ],
        )

    def test_text_audit_flags_forward_slash_windows_home_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            doc = root / "README.md"
            doc.write_text(
                "path=C:" + "/Users/" + "alice/Documents/private.doc\n",
                encoding="utf-8",
            )

            with audit_root(root):
                findings = public_hygiene_audit.audit_text_file(doc)

        self.assertEqual(
            [(finding.line, finding.kind) for finding in findings],
            [(1, "windows_home_path")],
        )

    def test_text_audit_flags_generic_windows_drive_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            doc = root / "README.md"
            doc.write_text(
                "path=D:" + "\\build\\private.doc\n",
                encoding="utf-8",
            )

            with audit_root(root):
                findings = public_hygiene_audit.audit_text_file(doc)

        self.assertEqual(
            [(finding.line, finding.kind) for finding in findings],
            [(1, "windows_drive_path")],
        )

    def test_text_audit_flags_windows_unc_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            doc = root / "README.md"
            unc_path = "\\\\" + "buildhost\\share\\private.doc"
            doc.write_text("path=" + unc_path + "\n", encoding="utf-8")

            with audit_root(root):
                findings = public_hygiene_audit.audit_text_file(doc)

        self.assertEqual(
            [(finding.line, finding.kind) for finding in findings],
            [(1, "windows_unc_path")],
        )

    def test_text_audit_does_not_flag_escaped_markdown_literals_as_unc_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            doc = root / "README.md"
            text = "literal=\"Cover" + "\\n\\n" + "\\\\pagebreak" + "\\n\\nDetail\""
            doc.write_text(text + "\n", encoding="utf-8")

            with audit_root(root):
                findings = public_hygiene_audit.audit_text_file(doc)

        self.assertEqual(findings, [])

    def test_text_audit_flags_windows_profile_env_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            doc = root / "README.md"
            doc.write_text(
                "path=%" + "USERPROFILE" + "%\\Documents\\private.doc\n",
                encoding="utf-8",
            )

            with audit_root(root):
                findings = public_hygiene_audit.audit_text_file(doc)

        self.assertEqual(
            [(finding.line, finding.kind) for finding in findings],
            [(1, "windows_profile_env_path")],
        )

    def test_text_audit_flags_windows_appdata_env_paths(self):
        for env in ("APPDATA", "LOCALAPPDATA"):
            with self.subTest(env=env):
                with tempfile.TemporaryDirectory() as tmp:
                    root = pathlib.Path(tmp)
                    doc = root / "README.md"
                    doc.write_text(
                        "path=%" + env + "%\\Vendor\\private.doc\n",
                        encoding="utf-8",
                    )

                    with audit_root(root):
                        findings = public_hygiene_audit.audit_text_file(doc)

                self.assertEqual(
                    [(finding.line, finding.kind) for finding in findings],
                    [(1, "windows_profile_env_path")],
                )

    def test_text_audit_flags_powershell_profile_env_paths(self):
        for env in ("USERPROFILE", "APPDATA", "LOCALAPPDATA"):
            with self.subTest(env=env):
                with tempfile.TemporaryDirectory() as tmp:
                    root = pathlib.Path(tmp)
                    doc = root / "README.md"
                    doc.write_text(
                        "path=$" + "env:" + env + "\\Vendor\\private.doc\n",
                        encoding="utf-8",
                    )

                    with audit_root(root):
                        findings = public_hygiene_audit.audit_text_file(doc)

                self.assertEqual(
                    [(finding.line, finding.kind) for finding in findings],
                    [(1, "powershell_profile_env_path")],
                )

    def test_text_audit_flags_shell_home_paths(self):
        for home_path in (
            "$" + "HOME" + "/private.doc",
            "${" + "HOME" + "}/private.doc",
        ):
            with self.subTest(home_path=home_path):
                with tempfile.TemporaryDirectory() as tmp:
                    root = pathlib.Path(tmp)
                    doc = root / "README.md"
                    doc.write_text("path=" + home_path + "\n", encoding="utf-8")

                    with audit_root(root):
                        findings = public_hygiene_audit.audit_text_file(doc)

                self.assertEqual(
                    [(finding.line, finding.kind) for finding in findings],
                    [(1, "shell_home_path")],
                )

    def test_text_audit_flags_common_private_key_headers(self):
        for key_prefix in ("DSA ", "ENCRYPTED "):
            with self.subTest(key_prefix=key_prefix):
                with tempfile.TemporaryDirectory() as tmp:
                    root = pathlib.Path(tmp)
                    doc = root / "README.md"
                    doc.write_text(
                        "key=-----BEGIN " + key_prefix + "PRIVATE KEY-----\n",
                        encoding="utf-8",
                    )

                    with audit_root(root):
                        findings = public_hygiene_audit.audit_text_file(doc)

                self.assertEqual(
                    [(finding.line, finding.kind) for finding in findings],
                    [(1, "private_key")],
                )

    def test_text_audit_flags_yaml_private_corpus_defaults(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            doc = root / ".github" / "workflows" / "release.yml"
            doc.parent.mkdir(parents=True)
            doc.write_text(
                "RWML_" + "RENDER_CORPUS" + ": corpus/private/render\n",
                encoding="utf-8",
            )

            with audit_root(root):
                findings = public_hygiene_audit.audit_text_file(doc)

        self.assertEqual(
            [(finding.line, finding.kind) for finding in findings],
            [(1, "private_corpus_default")],
        )

    def test_skip_policy_ignores_binary_suffixes_and_generated_dirs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            docx = root / "corpus" / "public" / "synthetic" / "fixture.docx"
            target_file = root / "target" / "debug" / "generated.txt"
            source_file = root / "src" / "lib.rs"
            source_file.parent.mkdir(parents=True)
            source_file.write_text("pub fn marker() {}\n", encoding="utf-8")

            with audit_root(root):
                self.assertTrue(public_hygiene_audit.should_skip(docx))
                self.assertTrue(public_hygiene_audit.should_skip(target_file))
                self.assertFalse(public_hygiene_audit.should_skip(source_file))

    def test_skip_policy_ignores_only_provenanced_bundled_font(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            bundled_font = root / "rwml-fonts" / "fonts" / "NotoSansKR-rwml-subset.ttf"
            bundled_full_font = (
                root / "rwml-fonts" / "fonts" / "NotoSansKR-rwml-subset-full.ttf"
            )
            bundled_arabic = (
                root / "rwml-fonts" / "fonts" / "NotoSansArabic-rwml-subset.ttf"
            )
            bundled_hebrew = (
                root / "rwml-fonts" / "fonts" / "NotoSansHebrew-rwml-subset.ttf"
            )
            other_font = root / "rwml-fonts" / "fonts" / "other.ttf"
            provenance = root / "rwml-fonts" / "PROVENANCE.md"
            bundled_font.parent.mkdir(parents=True)
            bundled_font.write_bytes(b"\x00\x01\x00\x00")
            bundled_full_font.write_bytes(b"\x00\x01\x00\x00")
            bundled_arabic.write_bytes(b"\x00\x01\x00\x00")
            bundled_hebrew.write_bytes(b"\x00\x01\x00\x00")
            other_font.write_bytes(b"\x00\x01\x00\x00")
            provenance.write_text("font provenance\n", encoding="utf-8")

            with audit_root(root):
                self.assertTrue(public_hygiene_audit.should_skip(bundled_font))
                self.assertTrue(public_hygiene_audit.should_skip(bundled_full_font))
                self.assertTrue(public_hygiene_audit.should_skip(bundled_arabic))
                self.assertTrue(public_hygiene_audit.should_skip(bundled_hebrew))
                self.assertFalse(public_hygiene_audit.should_skip(other_font))

    def test_docx_audit_scans_package_member_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            docx = root / "corpus" / "public" / "synthetic" / "metadata.docx"
            docx.parent.mkdir(parents=True)
            workbook = BytesIO()
            with zipfile.ZipFile(workbook, "w") as nested:
                nested.writestr("[Content_Types].xml", "<Types/>")
                nested.writestr("xl/" + "kr" + "-bid/sharedStrings.xml", "<sst/>")
            with zipfile.ZipFile(docx, "w") as archive:
                archive.writestr("[Content_Types].xml", "<Types/>")
                archive.writestr("word/" + "kr" + "-bid-notes.xml", "<w:document/>")
                archive.writestr(
                    "/Users/" + "alice/Documents/private.xml",
                    "<private/>",
                )
                archive.writestr("word/embeddings/workbook.xlsx", workbook.getvalue())

            with audit_root(root):
                findings = public_hygiene_audit.audit_docx_file(docx)

        self.assertEqual(
            [(finding.path, finding.line, finding.kind) for finding in findings],
            [
                (
                    "corpus/public/synthetic/metadata.docx::word/"
                    + "kr"
                    + "-bid-notes.xml",
                    None,
                    "kr_bid_trace",
                ),
                (
                    "corpus/public/synthetic/metadata.docx::/Users/"
                    + "alice/Documents/private.xml",
                    None,
                    "mac_home_path",
                ),
                (
                    "corpus/public/synthetic/metadata.docx::word/embeddings/workbook.xlsx::xl/"
                    + "kr"
                    + "-bid/sharedStrings.xml",
                    None,
                    "kr_bid_trace",
                ),
            ],
        )

    def test_docx_audit_scans_textual_package_parts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            docx = root / "corpus" / "public" / "synthetic" / "metadata.docx"
            docx.parent.mkdir(parents=True)
            with zipfile.ZipFile(docx, "w") as archive:
                archive.writestr(
                    "[Content_Types].xml",
                    '<Types><Default Extension="png" ContentType="image/png"/></Types>',
                )
                archive.writestr(
                    "docProps/core.xml",
                    "\n".join(
                        [
                            "<cp:coreProperties>",
                            "token=" + "sk-" + ("A" * 32),
                            "path=/Users/" + "alice/Documents/private.docx",
                            "project=" + "kr" + "-bid",
                            "</cp:coreProperties>",
                        ]
                    ),
                )
                archive.writestr(
                    "word/media/image.png",
                    b"\x00" + ("/Users/" + "alice/not-text").encode("utf-8"),
                )

            with audit_root(root):
                findings = public_hygiene_audit.audit_docx_file(docx)

        self.assertEqual(
            [(finding.path, finding.line, finding.kind) for finding in findings],
            [
                (
                    "corpus/public/synthetic/metadata.docx::docProps/core.xml",
                    2,
                    "openai_api_key",
                ),
                (
                    "corpus/public/synthetic/metadata.docx::docProps/core.xml",
                    3,
                    "mac_home_path",
                ),
                (
                    "corpus/public/synthetic/metadata.docx::docProps/core.xml",
                    4,
                    "kr_bid_trace",
                ),
            ],
        )

    def test_docx_audit_scans_embedded_office_package_text_parts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            docx = root / "corpus" / "public" / "synthetic" / "chart.docx"
            docx.parent.mkdir(parents=True)
            workbook = BytesIO()
            with zipfile.ZipFile(workbook, "w") as nested:
                nested.writestr("[Content_Types].xml", "<Types/>")
                nested.writestr(
                    "xl/sharedStrings.xml",
                    "\n".join(
                        [
                            "<sst>",
                            "project=" + "kr" + "-bid",
                            "path=/Users/" + "alice/Documents/source.xlsx",
                            "</sst>",
                        ]
                    ),
                )
                nested.writestr(
                    "xl/media/image.png",
                    b"\x00" + ("/Users/" + "alice/not-scanned").encode("utf-8"),
                )
            with zipfile.ZipFile(docx, "w") as archive:
                archive.writestr("[Content_Types].xml", "<Types/>")
                archive.writestr("word/embeddings/workbook.xlsx", workbook.getvalue())

            with audit_root(root):
                findings = public_hygiene_audit.audit_docx_file(docx)

        self.assertEqual(
            [(finding.path, finding.line, finding.kind) for finding in findings],
            [
                (
                    "corpus/public/synthetic/chart.docx::word/embeddings/workbook.xlsx::xl/sharedStrings.xml",
                    2,
                    "kr_bid_trace",
                ),
                (
                    "corpus/public/synthetic/chart.docx::word/embeddings/workbook.xlsx::xl/sharedStrings.xml",
                    3,
                    "mac_home_path",
                ),
            ],
        )

    def test_full_audit_checks_non_public_docx_paths_before_content_skip(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            docx = root / "corpus" / "private" / "fixture.docx"
            docx.parent.mkdir(parents=True)
            with zipfile.ZipFile(docx, "w") as archive:
                archive.writestr("[Content_Types].xml", "<Types/>")

            old_git_files = public_hygiene_audit.git_files
            try:
                public_hygiene_audit.git_files = lambda: [docx]
                with audit_root(root):
                    findings = public_hygiene_audit.audit()
            finally:
                public_hygiene_audit.git_files = old_git_files

        self.assertEqual(
            [(finding.path, finding.kind) for finding in findings],
            [("corpus/private/fixture.docx", "non_public_corpus_file")],
        )

    def test_full_audit_scans_top_level_office_packages(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            workbook = root / "corpus" / "public" / "synthetic" / "chart-data.xlsx"
            workbook.parent.mkdir(parents=True)
            with zipfile.ZipFile(workbook, "w") as archive:
                archive.writestr("[Content_Types].xml", "<Types/>")
                archive.writestr(
                    "xl/sharedStrings.xml",
                    "project=" + "kr" + "-bid",
                )

            old_git_files = public_hygiene_audit.git_files
            try:
                public_hygiene_audit.git_files = lambda: [workbook]
                with audit_root(root):
                    findings = public_hygiene_audit.audit()
            finally:
                public_hygiene_audit.git_files = old_git_files

        self.assertEqual(
            [(finding.path, finding.line, finding.kind) for finding in findings],
            [
                (
                    "corpus/public/synthetic/chart-data.xlsx::xl/sharedStrings.xml",
                    1,
                    "kr_bid_trace",
                )
            ],
        )

    def test_full_audit_scans_legacy_doc_text_views(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            legacy_doc = root / "corpus" / "public" / "synthetic" / "legacy.doc"
            legacy_doc.parent.mkdir(parents=True)
            legacy_doc.write_bytes(
                b"\xd0\xcf\x11\xe0"
                + ("project=" + "kr" + "-bid\n").encode("ascii")
                + ("path=/Users/" + "alice/Documents/source.doc").encode("utf-16le")
            )

            old_git_files = public_hygiene_audit.git_files
            try:
                public_hygiene_audit.git_files = lambda: [legacy_doc]
                with audit_root(root):
                    findings = public_hygiene_audit.audit()
            finally:
                public_hygiene_audit.git_files = old_git_files

        self.assertEqual(
            [(finding.path, finding.line, finding.kind) for finding in findings],
            [
                ("corpus/public/synthetic/legacy.doc", None, "kr_bid_trace"),
                ("corpus/public/synthetic/legacy.doc", None, "mac_home_path"),
            ],
        )

    def test_json_output_reports_schema_and_findings(self):
        finding = public_hygiene_audit.Finding("README.md", 3, "kind", "detail")
        old_audit = public_hygiene_audit.audit
        old_stdout = sys.stdout
        try:
            public_hygiene_audit.audit = lambda: [finding]
            sys.stdout = StringIO()
            status = public_hygiene_audit.main(["--json"])
            payload = json.loads(sys.stdout.getvalue())
        finally:
            public_hygiene_audit.audit = old_audit
            sys.stdout = old_stdout

        self.assertEqual(status, 1)
        self.assertEqual(payload["schema"], "rwml.public-hygiene-audit.v1")
        self.assertFalse(payload["passed"])
        self.assertEqual(payload["findings"], [finding.as_dict()])

    def test_json_payload_rejects_non_finite_values(self):
        with self.assertRaisesRegex(ValueError, "Out of range float values"):
            public_hygiene_audit.json_payload({"score": float("nan")})


if __name__ == "__main__":
    unittest.main()
