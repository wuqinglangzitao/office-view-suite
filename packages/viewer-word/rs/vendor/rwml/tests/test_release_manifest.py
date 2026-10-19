import hashlib
import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "scripts" / "release_manifest.py"
SPEC = importlib.util.spec_from_file_location("release_manifest", SCRIPT)
release_manifest = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = release_manifest
SPEC.loader.exec_module(release_manifest)


def write_passing_hygiene(path: pathlib.Path) -> None:
    path.write_text(
        json.dumps(
            {
                "schema": "rwml.public-hygiene-audit.v1",
                "passed": True,
                "findings": [],
            }
        ),
        encoding="utf-8",
    )


def write_passing_validation(
    path: pathlib.Path, documents: tuple[str, ...] = ("a.docx",)
) -> None:
    path.write_text(
        json.dumps(
            {
                "summary": {
                    "documents": len(documents),
                    "recall_min": 0.97,
                    "below_recall_min": 0,
                    "mean_recall": 0.9,
                    "skipped": 0,
                },
                "gate": {
                    "passed": True,
                    "checks": [
                        {
                            "metric": "below_recall_min",
                            "op": "<=",
                            "threshold": 0,
                            "actual": 0,
                            "passed": True,
                        },
                        {
                            "metric": "mean_recall",
                            "op": ">=",
                            "threshold": 0.9,
                            "actual": 0.9,
                            "passed": True,
                        },
                        {
                            "metric": "skipped",
                            "op": "<=",
                            "threshold": 0,
                            "actual": 0,
                            "passed": True,
                        },
                    ],
                },
                "rows": [
                    {"document": document, "status": "pass", "recall": 0.97}
                    for document in documents
                ],
            }
        ),
        encoding="utf-8",
    )


def write_passing_benchmark(path: pathlib.Path) -> None:
    path.write_text(
        json.dumps(
            {
                "summary": {
                    "files": 1,
                    "scored": 1,
                    "poi_recall_mean": 0.95,
                    "poi_f1_mean": 0.95,
                    "errors": 0,
                },
                "schema": "rwml.benchmark-report.v1",
                "benchmark": "extract-vs-mature",
                "gate": {
                    "passed": True,
                    "checks": [
                        {
                            "metric": "poi_recall_mean",
                            "op": ">=",
                            "threshold": 0.95,
                            "actual": 0.95,
                            "passed": True,
                        },
                        {
                            "metric": "poi_f1_mean",
                            "op": ">=",
                            "threshold": 0.95,
                            "actual": 0.95,
                            "passed": True,
                        },
                        {
                            "metric": "scored",
                            "op": ">=",
                            "threshold": 1,
                            "actual": 1,
                            "passed": True,
                        },
                        {
                            "metric": "errors",
                            "op": "<=",
                            "threshold": 0,
                            "actual": 0,
                            "passed": True,
                        },
                    ],
                },
            }
        ),
        encoding="utf-8",
    )


def write_public_corpus_document(
    root: pathlib.Path, relative: str = "synthetic/a.docx"
) -> pathlib.Path:
    document = root / relative
    document.parent.mkdir(parents=True, exist_ok=True)
    document.write_bytes(b"public corpus fixture")
    return document


def write_matching_public_manifests(root: pathlib.Path) -> tuple[pathlib.Path, pathlib.Path]:
    corpus = root / "MANIFEST.tsv"
    render_corpus = root / "RENDER_MANIFEST.tsv"
    write_public_corpus_document(root)
    corpus.write_text(
        "# path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n",
        encoding="utf-8",
    )
    render_corpus.write_text(
        "# path\tpages\twarnings\nsynthetic/a.docx\t1\t-\n",
        encoding="utf-8",
    )
    return corpus, render_corpus


class ReleaseManifestTests(unittest.TestCase):
    def test_manifest_records_artifact_sizes_checksums_and_validation_summary(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact_a = root / "rwml-aarch64-apple-darwin.tar.gz"
            artifact_b = root / "rwml-x86_64-unknown-linux-gnu.tar.gz"
            validation = root / "render-validation.json"
            benchmark_a = root / "extract-a-benchmark.json"
            benchmark_b = root / "extract-b-benchmark.json"
            artifact_a.write_bytes(b"darwin artifact")
            artifact_b.write_bytes(b"linux artifact")
            validation.write_text(
                json.dumps(
                    {
                        "summary": {
                            "documents": 3,
                            "measured": 2,
                            "mean_recall": 0.93,
                        },
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "below_recall_min",
                                    "op": "<=",
                                    "threshold": 0,
                                    "actual": 0,
                                    "passed": True,
                                }
                            ],
                        },
                        "rows": [{"document": "sample.docx", "status": "pass"}],
                    }
                ),
                encoding="utf-8",
            )
            benchmark_a.write_text(
                json.dumps(
                    {
                        "schema": "rwml.benchmark-report.v1",
                        "summary": {
                            "files": 4,
                            "scored": 4,
                            "poi_recall_mean": 0.96,
                        },
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "poi_recall_mean",
                                    "op": ">=",
                                    "threshold": 0.95,
                                    "actual": 0.96,
                                    "passed": True,
                                }
                            ],
                        },
                        "rows": [{"file": "sample", "poi_recall": 1.0}],
                    }
                ),
                encoding="utf-8",
            )
            benchmark_b.write_text(
                json.dumps(
                    {
                        "schema": "rwml.benchmark-report.v1",
                        "summary": {
                            "files": 2,
                            "scored": 1,
                            "poi_recall_mean": 0.5,
                        },
                        "gate": {
                            "passed": False,
                            "checks": [
                                {
                                    "metric": "poi_recall_mean",
                                    "op": ">=",
                                    "threshold": 0.95,
                                    "actual": 0.5,
                                    "passed": False,
                                }
                            ],
                        },
                        "rows": [{"file": "other", "poi_recall": 0.5}],
                    }
                ),
                encoding="utf-8",
            )

            manifest = release_manifest.release_manifest(
                [artifact_b, artifact_a],
                validation_report=validation,
                benchmark_reports=[benchmark_b, benchmark_a],
                version="0.1.0",
                git_rev="abc123",
            )

        self.assertEqual(manifest["schema"], "rwml.release-manifest.v1")
        self.assertEqual(manifest["version"], "0.1.0")
        self.assertEqual(manifest["git_rev"], "abc123")
        self.assertEqual(
            [artifact["name"] for artifact in manifest["artifacts"]],
            [
                "rwml-aarch64-apple-darwin.tar.gz",
                "rwml-x86_64-unknown-linux-gnu.tar.gz",
            ],
        )
        self.assertEqual(manifest["artifacts"][0]["bytes"], len(b"darwin artifact"))
        self.assertEqual(
            manifest["artifacts"][0]["sha256"],
            hashlib.sha256(b"darwin artifact").hexdigest(),
        )
        self.assertEqual(
            manifest["validation"]["summary"],
            {"documents": 3, "measured": 2, "mean_recall": 0.93},
        )
        self.assertEqual(
            manifest["validation"]["gate"],
            {
                "passed": True,
                "checks": [
                    {
                        "metric": "below_recall_min",
                        "op": "<=",
                        "threshold": 0,
                        "actual": 0,
                        "passed": True,
                    }
                ],
            },
        )
        self.assertNotIn("rows", manifest["validation"])
        self.assertEqual(
            manifest["benchmarks"],
            [
                {
                    "path": benchmark_a.as_posix(),
                    "summary": {
                        "files": 4,
                        "scored": 4,
                        "poi_recall_mean": 0.96,
                    },
                    "gate": {
                        "passed": True,
                        "checks": [
                            {
                                "metric": "poi_recall_mean",
                                "op": ">=",
                                "threshold": 0.95,
                                "actual": 0.96,
                                "passed": True,
                            }
                        ],
                    },
                },
                {
                    "path": benchmark_b.as_posix(),
                    "summary": {
                        "files": 2,
                        "scored": 1,
                        "poi_recall_mean": 0.5,
                    },
                    "gate": {
                        "passed": False,
                        "checks": [
                            {
                                "metric": "poi_recall_mean",
                                "op": ">=",
                                "threshold": 0.95,
                                "actual": 0.5,
                                "passed": False,
                            }
                        ],
                    },
                }
            ],
        )

    def test_manifest_sorts_same_named_inputs_by_full_path(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            alpha = root / "alpha"
            zeta = root / "zeta"
            alpha.mkdir()
            zeta.mkdir()
            artifact_alpha = alpha / "rwml.tar.gz"
            artifact_zeta = zeta / "rwml.tar.gz"
            benchmark_alpha = alpha / "benchmark.json"
            benchmark_zeta = zeta / "benchmark.json"
            artifact_alpha.write_bytes(b"alpha artifact")
            artifact_zeta.write_bytes(b"zeta artifact")
            for path, files in [(benchmark_alpha, 1), (benchmark_zeta, 2)]:
                path.write_text(
                    json.dumps(
                        {
                            "summary": {"files": files},
                            "gate": {"passed": True, "checks": []},
                        }
                    ),
                    encoding="utf-8",
                )

            manifest = release_manifest.release_manifest(
                [artifact_zeta, artifact_alpha],
                benchmark_reports=[benchmark_zeta, benchmark_alpha],
                release_policy="public-release",
            )

        self.assertEqual(
            [artifact["path"] for artifact in manifest["artifacts"]],
            [artifact_alpha.as_posix(), artifact_zeta.as_posix()],
        )
        self.assertEqual(
            [benchmark["path"] for benchmark in manifest["benchmarks"]],
            [benchmark_alpha.as_posix(), benchmark_zeta.as_posix()],
        )
        self.assertEqual(
            manifest["release_evidence"]["provided"]["benchmark_reports"],
            [benchmark_alpha.as_posix(), benchmark_zeta.as_posix()],
        )

    def test_manifest_rejects_duplicate_artifact_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            artifact = pathlib.Path(tmp) / "rwml.tar.gz"
            artifact.write_bytes(b"release artifact")

            with self.assertRaisesRegex(ValueError, "duplicate artifact path"):
                release_manifest.release_manifest([artifact, artifact])

    def test_manifest_rejects_duplicate_evidence_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            benchmark = root / "benchmark.json"
            corpus = root / "MANIFEST.tsv"
            artifact.write_bytes(b"release artifact")
            benchmark.write_text(
                json.dumps({"summary": {"files": 1}, "gate": {"passed": True, "checks": []}}),
                encoding="utf-8",
            )
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n",
                encoding="utf-8",
            )

            cases = [
                ("benchmark report", {"benchmark_reports": [benchmark, benchmark]}),
                ("corpus manifest", {"corpus_manifests": [corpus, corpus]}),
            ]
            for label, kwargs in cases:
                with self.subTest(label=label):
                    with self.assertRaisesRegex(ValueError, f"duplicate {label} path"):
                        release_manifest.release_manifest([artifact], **kwargs)

    def test_manifest_rejects_blank_release_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            artifact = pathlib.Path(tmp) / "rwml.tar.gz"
            artifact.write_bytes(b"release artifact")

            cases = [
                ("version", {"version": " "}),
                ("git_rev", {"git_rev": ""}),
            ]
            for label, kwargs in cases:
                with self.subTest(label=label):
                    with self.assertRaisesRegex(ValueError, f"{label} must not be empty"):
                        release_manifest.release_manifest([artifact], **kwargs)

    def test_manifest_rejects_non_string_release_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            artifact = pathlib.Path(tmp) / "rwml.tar.gz"
            artifact.write_bytes(b"release artifact")

            cases = [
                ("version", {"version": 1}),
                ("git_rev", {"git_rev": 1}),
            ]
            for label, kwargs in cases:
                with self.subTest(label=label):
                    with self.assertRaisesRegex(ValueError, f"{label} must be a string"):
                        release_manifest.release_manifest([artifact], **kwargs)

    def test_manifest_rejects_padded_release_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            artifact = pathlib.Path(tmp) / "rwml.tar.gz"
            artifact.write_bytes(b"release artifact")

            cases = [
                ("version", {"version": " 0.1.0"}),
                ("git_rev", {"git_rev": "abc123 "}),
            ]
            for label, kwargs in cases:
                with self.subTest(label=label):
                    with self.assertRaisesRegex(
                        ValueError,
                        f"{label} must not have surrounding whitespace",
                    ):
                        release_manifest.release_manifest([artifact], **kwargs)

    def test_manifest_rejects_whitespace_release_metadata(self):
        with tempfile.TemporaryDirectory() as tmp:
            artifact = pathlib.Path(tmp) / "rwml.tar.gz"
            artifact.write_bytes(b"release artifact")

            cases = [
                ("version", {"version": "0.1.0 beta"}),
                ("git_rev", {"git_rev": "abc123\nnext"}),
            ]
            for label, kwargs in cases:
                with self.subTest(label=label):
                    with self.assertRaisesRegex(
                        ValueError,
                        f"{label} must not contain whitespace",
                    ):
                        release_manifest.release_manifest([artifact], **kwargs)

    def test_hygiene_summary_rejects_passed_report_with_findings(self):
        with tempfile.TemporaryDirectory() as tmp:
            hygiene = pathlib.Path(tmp) / "public-hygiene.json"
            hygiene.write_text(
                json.dumps({"passed": True, "findings": [{"path": "private.docx"}]}),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "cannot pass with hygiene findings"):
                release_manifest.hygiene_summary(hygiene)

    def test_hygiene_summary_rejects_failed_report_without_findings(self):
        with tempfile.TemporaryDirectory() as tmp:
            hygiene = pathlib.Path(tmp) / "public-hygiene.json"
            hygiene.write_text(
                json.dumps({"passed": False, "findings": []}),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "cannot fail without hygiene findings"):
                release_manifest.hygiene_summary(hygiene)

    def test_hygiene_summary_rejects_non_object_findings(self):
        with tempfile.TemporaryDirectory() as tmp:
            hygiene = pathlib.Path(tmp) / "public-hygiene.json"
            hygiene.write_text(
                json.dumps({"passed": False, "findings": ["private.docx"]}),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "hygiene finding is not an object"):
                release_manifest.hygiene_summary(hygiene)

    def test_hygiene_summary_rejects_findings_missing_required_fields(self):
        with tempfile.TemporaryDirectory() as tmp:
            hygiene = pathlib.Path(tmp) / "public-hygiene.json"
            hygiene.write_text(
                json.dumps({"passed": False, "findings": [{"path": "private.docx"}]}),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "hygiene finding missing required field: line",
            ):
                release_manifest.hygiene_summary(hygiene)

    def test_hygiene_summary_rejects_unexpected_finding_keys(self):
        with tempfile.TemporaryDirectory() as tmp:
            hygiene = pathlib.Path(tmp) / "public-hygiene.json"
            hygiene.write_text(
                json.dumps(
                    {
                        "passed": False,
                        "findings": [
                            {
                                "path": "private.docx",
                                "line": 1,
                                "kind": "private",
                                "detail": "blocked",
                                "note": "extra",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "hygiene finding key is invalid: note",
            ):
                release_manifest.hygiene_summary(hygiene)

    def test_hygiene_summary_rejects_findings_with_invalid_field_types(self):
        with tempfile.TemporaryDirectory() as tmp:
            hygiene = pathlib.Path(tmp) / "public-hygiene.json"
            hygiene.write_text(
                json.dumps(
                    {
                        "passed": False,
                        "findings": [
                            {
                                "path": "private.docx",
                                "line": "1",
                                "kind": "private",
                                "detail": "blocked",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "hygiene finding line is invalid"):
                release_manifest.hygiene_summary(hygiene)

    def test_hygiene_summary_rejects_empty_or_padded_finding_text(self):
        cases = [
            ("path", ""),
            ("kind", " private"),
            ("detail", " "),
        ]
        for field, value in cases:
            with self.subTest(field=field):
                with tempfile.TemporaryDirectory() as tmp:
                    hygiene = pathlib.Path(tmp) / "public-hygiene.json"
                    finding = {
                        "path": "private.docx",
                        "line": 1,
                        "kind": "private",
                        "detail": "blocked",
                    }
                    finding[field] = value
                    hygiene.write_text(
                        json.dumps({"passed": False, "findings": [finding]}),
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        ValueError,
                        f"hygiene finding {field} is invalid",
                    ):
                        release_manifest.hygiene_summary(hygiene)

    def test_hygiene_summary_rejects_local_finding_paths(self):
        unix_path = "/".join(("", "Users", "alice", "private.docx"))
        windows_path = "C:" + unix_path
        for finding_path in (unix_path, windows_path):
            with self.subTest(finding_path=finding_path):
                with tempfile.TemporaryDirectory() as tmp:
                    hygiene = pathlib.Path(tmp) / "public-hygiene.json"
                    hygiene.write_text(
                        json.dumps(
                            {
                                "passed": False,
                                "findings": [
                                    {
                                        "path": finding_path,
                                        "line": 1,
                                        "kind": "private",
                                        "detail": "blocked",
                                    }
                                ],
                            }
                        ),
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        ValueError,
                        "hygiene finding path is invalid",
                    ):
                        release_manifest.hygiene_summary(hygiene)

    def test_hygiene_summary_rejects_non_canonical_finding_kind(self):
        for kind in ("local path", "\u8b66\u544a"):
            with self.subTest(kind=kind):
                with tempfile.TemporaryDirectory() as tmp:
                    hygiene = pathlib.Path(tmp) / "public-hygiene.json"
                    hygiene.write_text(
                        json.dumps(
                            {
                                "passed": False,
                                "findings": [
                                    {
                                        "path": "private.docx",
                                        "line": 1,
                                        "kind": kind,
                                        "detail": "blocked",
                                    }
                                ],
                            }
                        ),
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        ValueError,
                        "hygiene finding kind is invalid",
                    ):
                        release_manifest.hygiene_summary(hygiene)

    def test_hygiene_summary_rejects_boolean_finding_line(self):
        with tempfile.TemporaryDirectory() as tmp:
            hygiene = pathlib.Path(tmp) / "public-hygiene.json"
            hygiene.write_text(
                json.dumps(
                    {
                        "passed": False,
                        "findings": [
                            {
                                "path": "private.docx",
                                "line": True,
                                "kind": "private",
                                "detail": "blocked",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "hygiene finding line is invalid"):
                release_manifest.hygiene_summary(hygiene)

    def test_hygiene_summary_rejects_non_positive_finding_line(self):
        for line in (0, -1):
            with self.subTest(line=line):
                with tempfile.TemporaryDirectory() as tmp:
                    hygiene = pathlib.Path(tmp) / "public-hygiene.json"
                    hygiene.write_text(
                        json.dumps(
                            {
                                "passed": False,
                                "findings": [
                                    {
                                        "path": "private.docx",
                                        "line": line,
                                        "kind": "private",
                                        "detail": "blocked",
                                    }
                                ],
                            }
                        ),
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        ValueError,
                        "hygiene finding line is invalid",
                    ):
                        release_manifest.hygiene_summary(hygiene)

    def test_hygiene_summary_rejects_duplicate_findings(self):
        with tempfile.TemporaryDirectory() as tmp:
            hygiene = pathlib.Path(tmp) / "public-hygiene.json"
            finding = {
                "path": "private.docx",
                "line": 1,
                "kind": "private",
                "detail": "blocked",
            }
            hygiene.write_text(
                json.dumps({"passed": False, "findings": [finding, finding]}),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "duplicate hygiene finding"):
                release_manifest.hygiene_summary(hygiene)

    def test_hygiene_summary_rejects_non_object_report(self):
        with tempfile.TemporaryDirectory() as tmp:
            hygiene = pathlib.Path(tmp) / "public-hygiene.json"
            hygiene.write_text(json.dumps([]), encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "does not contain a JSON object"):
                release_manifest.hygiene_summary(hygiene)

    def test_report_summary_rejects_non_object_report(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(json.dumps([]), encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "does not contain a JSON object"):
                release_manifest.report_summary(validation)

    def test_cli_identifies_empty_json_evidence_path(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.crate"
            hygiene = root / "public-hygiene.json"
            artifact.write_bytes(b"release artifact")
            hygiene.write_text("", encoding="utf-8")

            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--hygiene-report",
                    str(hygiene),
                    str(artifact),
                ],
                check=False,
                capture_output=True,
                text=True,
            )

        self.assertEqual(completed.returncode, 2)
        self.assertIn(f"{hygiene}: invalid JSON", completed.stderr)

    def test_report_summary_rejects_empty_summary(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(json.dumps({"summary": {}}), encoding="utf-8")

            with self.assertRaisesRegex(ValueError, "summary must not be empty"):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_non_finite_summary_values(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps({"summary": {"mean_recall": float("nan")}}),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "summary contains non-finite value"):
                release_manifest.report_summary(validation)

    def test_report_summary_accepts_reference_stability_flag(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {
                            "reference_stable": True,
                            "unstable_references": [],
                        }
                    }
                ),
                encoding="utf-8",
            )

            report = release_manifest.report_summary(validation)

        self.assertEqual(
            report["summary"],
            {"reference_stable": True, "unstable_references": []},
        )

    def test_report_summary_rejects_non_numeric_summary_values(self):
        for key, value in (
            ("documents", True),
            ("mean_recall", "0.9"),
            ("unstable_references", "none"),
        ):
            with self.subTest(key=key):
                with tempfile.TemporaryDirectory() as tmp:
                    validation = pathlib.Path(tmp) / "render-validation.json"
                    validation.write_text(
                        json.dumps({"summary": {key: value}}),
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        ValueError,
                        f"summary value is invalid: {key}",
                    ):
                        release_manifest.report_summary(validation)

    def test_report_summary_rejects_non_canonical_summary_keys(self):
        for key in ("mean recall", "\u8a08"):
            with self.subTest(key=key):
                with tempfile.TemporaryDirectory() as tmp:
                    validation = pathlib.Path(tmp) / "render-validation.json"
                    validation.write_text(
                        json.dumps({"summary": {key: 1}}),
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        ValueError,
                        f"summary key is invalid: {key}",
                    ):
                        release_manifest.report_summary(validation)

    def test_report_summary_rejects_non_finite_gate_values(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1},
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "mean_recall",
                                    "op": ">=",
                                    "threshold": float("nan"),
                                    "actual": 1.0,
                                    "passed": True,
                                }
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "gate check threshold is not a finite number",
            ):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_non_canonical_gate_keys(self):
        for key in ("mean recall", "\u8a08"):
            with self.subTest(key=key):
                with tempfile.TemporaryDirectory() as tmp:
                    validation = pathlib.Path(tmp) / "render-validation.json"
                    validation.write_text(
                        json.dumps(
                            {
                                "summary": {"documents": 1},
                                "gate": {"passed": True, "checks": [], key: 1},
                            }
                        ),
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        ValueError,
                        f"gate key is invalid: {key}",
                    ):
                        release_manifest.report_summary(validation)

    def test_report_summary_rejects_unexpected_gate_keys(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1},
                        "gate": {"passed": True, "checks": [], "note": "extra"},
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "gate key is invalid: note"):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_non_object_gate(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps({"summary": {"documents": 1}, "gate": []}),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "gate is not a JSON object"):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_non_boolean_gate_passed(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps({"summary": {"documents": 1}, "gate": {"passed": "yes"}}),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "gate passed is not a boolean"):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_non_list_gate_checks(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1},
                        "gate": {"passed": True, "checks": {}},
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "gate checks is not a list"):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_non_object_gate_checks(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1},
                        "gate": {"passed": True, "checks": ["mean_recall"]},
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "gate check is not a JSON object"):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_non_boolean_gate_check_passed(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1},
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "mean_recall",
                                    "op": ">=",
                                    "threshold": 0.9,
                                    "actual": 1.0,
                                    "passed": "yes",
                                }
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "gate check passed is not a boolean"):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_gate_checks_missing_required_fields(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1},
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "op": ">=",
                                    "threshold": 0.9,
                                    "actual": 1.0,
                                    "passed": True,
                                }
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "gate check missing required field: metric",
            ):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_non_canonical_gate_check_keys(self):
        for key in ("actual value", "\u8a08"):
            with self.subTest(key=key):
                with tempfile.TemporaryDirectory() as tmp:
                    validation = pathlib.Path(tmp) / "render-validation.json"
                    validation.write_text(
                        json.dumps(
                            {
                                "summary": {"documents": 1},
                                "gate": {
                                    "passed": True,
                                    "checks": [
                                        {
                                            "metric": "mean_recall",
                                            "op": ">=",
                                            "threshold": 0.9,
                                            "actual": 1.0,
                                            "passed": True,
                                            key: 1,
                                        }
                                    ],
                                },
                            }
                        ),
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        ValueError,
                        f"gate check key is invalid: {key}",
                    ):
                        release_manifest.report_summary(validation)

    def test_report_summary_rejects_unexpected_gate_check_keys(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1},
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "mean_recall",
                                    "op": ">=",
                                    "threshold": 0.9,
                                    "actual": 1.0,
                                    "passed": True,
                                    "note": "extra",
                                }
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "gate check key is invalid: note",
            ):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_non_string_gate_check_fields(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1},
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": 1,
                                    "op": ">=",
                                    "threshold": 0.9,
                                    "actual": 1.0,
                                    "passed": True,
                                }
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "gate check metric is not a string"):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_empty_or_padded_gate_check_metrics(self):
        for metric in ("", " mean_recall", "mean recall", "\u8a08"):
            with self.subTest(metric=metric):
                with tempfile.TemporaryDirectory() as tmp:
                    validation = pathlib.Path(tmp) / "render-validation.json"
                    validation.write_text(
                        json.dumps(
                            {
                                "summary": {"documents": 1},
                                "gate": {
                                    "passed": True,
                                    "checks": [
                                        {
                                            "metric": metric,
                                            "op": ">=",
                                            "threshold": 0.9,
                                            "actual": 1.0,
                                            "passed": True,
                                        }
                                    ],
                                },
                            }
                        ),
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(ValueError, "gate check metric is invalid"):
                        release_manifest.report_summary(validation)

    def test_report_summary_rejects_duplicate_gate_check_metric_operator(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1},
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "mean_recall",
                                    "op": ">=",
                                    "threshold": 0.9,
                                    "actual": 1.0,
                                    "passed": True,
                                },
                                {
                                    "metric": "mean_recall",
                                    "op": ">=",
                                    "threshold": 0.95,
                                    "actual": 1.0,
                                    "passed": True,
                                },
                            ],
                        },
                        "rows": [
                            {
                                "document": "a.docx",
                                "status": "pass",
                                "recall": 0.97,
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "duplicate gate check: mean_recall >="):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_unsupported_gate_check_operator(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1},
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "mean_recall",
                                    "op": "==",
                                    "threshold": 0.9,
                                    "actual": 1.0,
                                    "passed": True,
                                }
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "unsupported gate check operator: =="):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_non_numeric_gate_check_values(self):
        for field in ("threshold", "actual"):
            with self.subTest(field=field):
                with tempfile.TemporaryDirectory() as tmp:
                    validation = pathlib.Path(tmp) / "render-validation.json"
                    check = {
                        "metric": "mean_recall",
                        "op": ">=",
                        "threshold": 0.9,
                        "actual": 1.0,
                        "passed": True,
                    }
                    check[field] = "0.9"
                    validation.write_text(
                        json.dumps(
                            {
                                "summary": {"documents": 1},
                                "gate": {"passed": True, "checks": [check]},
                            }
                        ),
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        ValueError,
                        f"gate check {field} is not a finite number",
                    ):
                        release_manifest.report_summary(validation)

    def test_report_summary_accepts_null_gate_check_actual(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 0},
                        "gate": {
                            "passed": False,
                            "checks": [
                                {
                                    "metric": "mean_recall",
                                    "op": ">=",
                                    "threshold": 0.9,
                                    "actual": None,
                                    "passed": False,
                                }
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )

            report = release_manifest.report_summary(validation)

        self.assertIsNone(report["gate"]["checks"][0]["actual"])

    def test_report_summary_rejects_passed_gate_with_failed_check(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1},
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "mean_recall",
                                    "op": ">=",
                                    "threshold": 0.9,
                                    "actual": 0.5,
                                    "passed": False,
                                }
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "gate passed with failed checks"):
                release_manifest.report_summary(validation)

    def test_report_summary_rejects_failed_gate_without_failed_checks(self):
        with tempfile.TemporaryDirectory() as tmp:
            validation = pathlib.Path(tmp) / "render-validation.json"
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1},
                        "gate": {
                            "passed": False,
                            "checks": [
                                {
                                    "metric": "mean_recall",
                                    "op": ">=",
                                    "threshold": 0.9,
                                    "actual": 1.0,
                                    "passed": True,
                                }
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(ValueError, "gate failed without failed checks"):
                release_manifest.report_summary(validation)

    def test_cli_writes_manifest_json(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.wasm"
            corpus = root / "MANIFEST.tsv"
            output = root / "manifest.json"
            artifact.write_bytes(b"wasm bytes")
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/fields.docx\t6\tUnsupportedFieldEvaluation\n",
                encoding="utf-8",
            )

            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--version",
                    "0.1.0",
                    "--git-rev",
                    "abc123",
                    "--corpus-manifest",
                    str(corpus),
                    "--output",
                    str(output),
                    str(artifact),
                ],
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(completed.returncode, 0, completed.stderr)
            manifest = json.loads(output.read_text(encoding="utf-8"))

        self.assertEqual(manifest["artifacts"][0]["name"], "rwml.wasm")
        self.assertEqual(
            manifest["artifacts"][0]["sha256"],
            hashlib.sha256(b"wasm bytes").hexdigest(),
        )
        self.assertEqual(
            manifest["corpus_manifests"][0]["summary"],
            {
                "documents": 1,
                "numeric_totals": {"fields": 6},
                "warning_counts": {"UnsupportedFieldEvaluation": 1},
            },
        )

    def test_cli_rejects_non_finite_manifest_json(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            validation = root / "render-validation.json"
            output = root / "manifest.json"
            artifact.write_bytes(b"release artifact")
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1, "mean_recall": float("nan")},
                        "gate": {"passed": True, "checks": []},
                    }
                ),
                encoding="utf-8",
            )

            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--validation-report",
                    str(validation),
                    "--output",
                    str(output),
                    str(artifact),
                ],
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertFalse(output.exists())

        self.assertEqual(completed.returncode, 2)
        self.assertIn("summary contains non-finite value", completed.stderr)

    def test_manifest_embeds_public_corpus_manifest_summaries_without_rows(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            corpus = root / "MANIFEST.tsv"
            render = root / "RENDER_MANIFEST.tsv"
            artifact.write_bytes(b"release artifact")
            corpus.write_text(
                "\n".join(
                    [
                        "# path\tcomments\tfields\tfloating_shapes\twarnings",
                        "synthetic/a.docx\t2\t1\t0\tUnsupportedFieldEvaluation",
                        "synthetic/b.docx\t0\t0\t1\t"
                        "FloatingShapePlaceholderOnly|TrackedChangesPresent",
                        "",
                    ]
                ),
                encoding="utf-8",
            )
            render.write_text(
                "\n".join(
                    [
                        "# path\tpages\twarnings",
                        "synthetic/a.docx\t1\t-",
                        "synthetic/b.docx\t3\tFloatingShapePlaceholderOnly",
                        "",
                    ]
                ),
                encoding="utf-8",
            )

            manifest = release_manifest.release_manifest(
                [artifact],
                corpus_manifests=[render, corpus],
            )

        self.assertEqual(
            manifest["corpus_manifests"],
            [
                {
                    "path": corpus.as_posix(),
                    "summary": {
                        "documents": 2,
                        "numeric_totals": {
                            "comments": 2,
                            "fields": 1,
                            "floating_shapes": 1,
                        },
                        "warning_counts": {
                            "FloatingShapePlaceholderOnly": 1,
                            "TrackedChangesPresent": 1,
                            "UnsupportedFieldEvaluation": 1,
                        },
                    },
                },
                {
                    "path": render.as_posix(),
                    "summary": {
                        "documents": 2,
                        "numeric_totals": {"pages": 4},
                        "warning_counts": {"FloatingShapePlaceholderOnly": 1},
                    },
                },
            ],
        )
        self.assertNotIn("rows", manifest["corpus_manifests"][0])

    def test_manifest_rejects_negative_public_corpus_counts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/fields.docx\t-1\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "negative numeric value for fields",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_non_numeric_public_corpus_counts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/fields.docx\tmany\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "non-numeric value for fields: many",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_signed_positive_public_corpus_counts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/fields.docx\t+1\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "non-canonical numeric value for fields: \\+1",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_whitespace_public_corpus_counts(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/fields.docx\t 1\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "whitespace-padded numeric value for fields:  1",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_duplicate_public_corpus_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "\n".join(
                    [
                        "# path\tfields\twarnings",
                        "synthetic/fields.docx\t1\t-",
                        "synthetic/fields.docx\t2\t-",
                        "",
                    ]
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "duplicate document path: synthetic/fields.docx",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_unsafe_public_corpus_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\n../private/source.docx\t1\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "unsafe document path: ../private/source.docx",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_whitespace_public_corpus_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/fields.docx \t1\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "whitespace-padded document path: synthetic/fields.docx ",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_non_canonical_public_corpus_paths(self):
        for document_path in ("synthetic/private source.docx", "synthetic/\u6587\u66f8.docx"):
            with self.subTest(document_path=document_path):
                with tempfile.TemporaryDirectory() as tmp:
                    root = pathlib.Path(tmp)
                    corpus = root / "MANIFEST.tsv"
                    corpus.write_text(
                        f"# path\tfields\twarnings\n{document_path}\t1\t-\n",
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        ValueError,
                        f"unsafe document path: {document_path}",
                    ):
                        release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_duplicate_public_corpus_columns(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\tfields\twarnings\nsynthetic/fields.docx\t1\t2\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "duplicate TSV column: fields",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_empty_public_corpus_columns(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\t\twarnings\nsynthetic/fields.docx\t1\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "empty TSV column",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_missing_public_corpus_warnings_column(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\nsynthetic/fields.docx\t1\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "missing required TSV column: warnings",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_missing_public_corpus_count_columns(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\twarnings\nsynthetic/fields.docx\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "missing TSV count columns",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_whitespace_public_corpus_columns(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\t fields\twarnings\nsynthetic/fields.docx\t1\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "whitespace-padded TSV column:  fields",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_non_canonical_public_corpus_columns(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfield count\twarnings\nsynthetic/fields.docx\t1\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "non-canonical TSV column: field count",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_trailing_whitespace_public_corpus_columns(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings \nsynthetic/fields.docx\t1\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "whitespace-padded TSV column: warnings ",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_repeated_public_corpus_header_rows(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\npath\tfields\twarnings\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "repeated TSV header row",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_repeated_commented_public_corpus_header_rows(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\n# path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "repeated TSV header row",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_indented_commented_repeated_public_corpus_header_rows(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\n  # path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "repeated TSV header row",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_empty_public_corpus_rows(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text("# path\tfields\twarnings\n", encoding="utf-8")

            with self.assertRaisesRegex(
                ValueError,
                "does not contain document rows",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_duplicate_public_corpus_warning_tokens(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\n"
                "synthetic/fields.docx\t1\tUnsupportedFieldEvaluation|UnsupportedFieldEvaluation\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "duplicate warning token: UnsupportedFieldEvaluation",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_empty_public_corpus_warning_tokens(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\n"
                "synthetic/fields.docx\t1\tUnsupportedFieldEvaluation|\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "empty warning token",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_whitespace_public_corpus_warning_tokens(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\n"
                "synthetic/fields.docx\t1\t UnsupportedFieldEvaluation\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "whitespace-padded warning token:  UnsupportedFieldEvaluation",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_non_canonical_public_corpus_warning_tokens(self):
        for warning in ("Unsupported Field", "\u8b66\u544a"):
            with self.subTest(warning=warning):
                with tempfile.TemporaryDirectory() as tmp:
                    root = pathlib.Path(tmp)
                    corpus = root / "MANIFEST.tsv"
                    corpus.write_text(
                        "# path\tfields\twarnings\n"
                        f"synthetic/fields.docx\t1\t{warning}\n",
                        encoding="utf-8",
                    )

                    with self.assertRaisesRegex(
                        ValueError,
                        f"non-canonical warning token: {warning}",
                    ):
                        release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_unknown_public_corpus_warning_tokens(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\n"
                "synthetic/fields.docx\t1\tUnsupportedFieldEvalution\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "unknown warning token: UnsupportedFieldEvalution",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_trailing_public_corpus_warning_token_whitespace(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\n"
                "synthetic/fields.docx\t1\tUnsupportedFieldEvaluation \n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "whitespace-padded warning token",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_rejects_mixed_public_corpus_warning_sentinel(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            corpus = root / "MANIFEST.tsv"
            corpus.write_text(
                "# path\tfields\twarnings\n"
                "synthetic/fields.docx\t1\t-|UnsupportedFieldEvaluation\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "invalid warning token: -",
            ):
                release_manifest.corpus_manifest_summary(corpus)

    def test_manifest_embeds_named_release_policy(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            artifact.write_bytes(b"release artifact")

            manifest = release_manifest.release_manifest(
                [artifact],
                release_policy="public-release",
            )

        self.assertEqual(
            manifest["release_policy"],
            {
                "name": "public-release",
                "required_gates": {
                    "default": [
                        "python3 scripts/public_hygiene_audit.py",
                        "cargo fmt --all -- --check",
                        "cargo clippy --all-targets -- -D warnings",
                        "cargo clippy --all-targets --all-features -- -D warnings",
                        "cargo test --all-targets",
                        "cargo test --no-default-features",
                        "cargo test --doc --all-features",
                        "cargo doc --no-deps --all-features",
                    ],
                    "render": [
                        "cargo test --all-targets --features render",
                    ],
                },
                "optional_local_gates": {
                    "extraction_benchmark": {
                        "min_poi_recall_mean": 0.95,
                        "min_poi_f1_mean": 0.95,
                        "max_errors": 0,
                        "min_scored": 1,
                    },
                    "render_validation": {
                        "recall_min": 0.97,
                        "min_mean_recall": 0.9,
                        "max_skipped": 0,
                    },
                    "public_corpus": {
                        "manifest_match": "exact",
                    },
                },
            },
        )
        self.assertEqual(
            manifest["release_evidence"],
            {
                "policy": "public-release",
                "strict_policy_status": "missing_inputs",
                "strict_policy_enforced": False,
                "strict_policy_inputs_complete": False,
                "strict_missing": [
                    "hygiene report",
                    "validation report",
                    "benchmark report",
                    "corpus manifest",
                ],
                "provided": {
                    "hygiene_report": None,
                    "validation_report": None,
                    "benchmark_reports": [],
                    "corpus_manifests": [],
                },
            },
        )

    def test_cli_writes_named_release_policy(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            output = root / "manifest.json"
            artifact.write_bytes(b"release artifact")

            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--release-policy",
                    "public-release",
                    "--output",
                    str(output),
                    str(artifact),
                ],
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(completed.returncode, 0, completed.stderr)
            manifest = json.loads(output.read_text(encoding="utf-8"))

        self.assertEqual(manifest["release_policy"]["name"], "public-release")
        self.assertEqual(
            manifest["release_evidence"]["strict_policy_status"],
            "missing_inputs",
        )
        self.assertFalse(manifest["release_evidence"]["strict_policy_enforced"])
        self.assertFalse(manifest["release_evidence"]["strict_policy_inputs_complete"])

    def test_release_evidence_status_names_complete_unenforced_inputs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            write_passing_hygiene(hygiene)
            write_passing_validation(validation)
            write_passing_benchmark(benchmark)
            corpus, render_corpus = write_matching_public_manifests(root)

            evidence = release_manifest.release_evidence_summary(
                "public-release",
                enforce_policy_inputs=False,
                hygiene_report=hygiene,
                validation_report=validation,
                benchmark_reports=[benchmark],
                corpus_manifests=[corpus, render_corpus],
            )

        self.assertEqual(
            evidence["strict_policy_status"],
            "inputs_complete_not_enforced",
        )
        self.assertFalse(evidence["strict_policy_enforced"])
        self.assertTrue(evidence["strict_policy_inputs_complete"])
        self.assertEqual(evidence["strict_missing"], [])

    def test_release_evidence_marks_missing_provided_policy_files_missing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            evidence = release_manifest.release_evidence_summary(
                "public-release",
                enforce_policy_inputs=False,
                hygiene_report=root / "public-hygiene.json",
                validation_report=root / "render-validation.json",
                benchmark_reports=[root / "extract-benchmark.json"],
                corpus_manifests=[
                    root / "MANIFEST.tsv",
                    root / "RENDER_MANIFEST.tsv",
                ],
            )

        self.assertEqual(evidence["strict_policy_status"], "missing_inputs")
        self.assertFalse(evidence["strict_policy_inputs_complete"])
        self.assertEqual(
            evidence["strict_missing"],
            [
                "valid public corpus manifests",
                "passing hygiene report",
                "policy-strength validation report",
                "policy-strength benchmark report",
            ],
        )

    def test_release_evidence_marks_mismatched_corpus_manifest_paths_missing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            corpus = root / "MANIFEST.tsv"
            render_corpus = root / "RENDER_MANIFEST.tsv"
            write_passing_hygiene(hygiene)
            write_passing_validation(validation)
            write_passing_benchmark(benchmark)
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n",
                encoding="utf-8",
            )
            render_corpus.write_text(
                "# path\tpages\twarnings\nsynthetic/b.docx\t1\t-\n",
                encoding="utf-8",
            )

            evidence = release_manifest.release_evidence_summary(
                "public-release",
                enforce_policy_inputs=False,
                hygiene_report=hygiene,
                validation_report=validation,
                benchmark_reports=[benchmark],
                corpus_manifests=[corpus, render_corpus],
            )

        self.assertEqual(evidence["strict_policy_status"], "missing_inputs")
        self.assertFalse(evidence["strict_policy_inputs_complete"])
        self.assertEqual(
            evidence["strict_missing"],
            ["matching public corpus manifest documents"],
        )

    def test_release_evidence_marks_missing_public_corpus_documents_missing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            corpus = root / "MANIFEST.tsv"
            render_corpus = root / "RENDER_MANIFEST.tsv"
            write_passing_hygiene(hygiene)
            write_passing_validation(validation)
            write_passing_benchmark(benchmark)
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/missing.docx\t0\t-\n",
                encoding="utf-8",
            )
            render_corpus.write_text(
                "# path\tpages\twarnings\nsynthetic/missing.docx\t1\t-\n",
                encoding="utf-8",
            )

            evidence = release_manifest.release_evidence_summary(
                "public-release",
                enforce_policy_inputs=False,
                hygiene_report=hygiene,
                validation_report=validation,
                benchmark_reports=[benchmark],
                corpus_manifests=[corpus, render_corpus],
            )

        self.assertEqual(evidence["strict_policy_status"], "missing_inputs")
        self.assertFalse(evidence["strict_policy_inputs_complete"])
        self.assertEqual(
            evidence["strict_missing"],
            ["existing public corpus documents"],
        )

    def test_release_evidence_marks_validation_report_not_covering_public_corpus_missing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            write_passing_hygiene(hygiene)
            write_passing_validation(validation, documents=("other.docx",))
            write_passing_benchmark(benchmark)
            corpus, render_corpus = write_matching_public_manifests(root)

            evidence = release_manifest.release_evidence_summary(
                "public-release",
                enforce_policy_inputs=False,
                hygiene_report=hygiene,
                validation_report=validation,
                benchmark_reports=[benchmark],
                corpus_manifests=[corpus, render_corpus],
            )

        self.assertEqual(evidence["strict_policy_status"], "missing_inputs")
        self.assertFalse(evidence["strict_policy_inputs_complete"])
        self.assertEqual(
            evidence["strict_missing"],
            ["validation report covering public corpus"],
        )

    def test_release_evidence_marks_invalid_corpus_manifests_missing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            corpus = root / "MANIFEST.tsv"
            render_corpus = root / "RENDER_MANIFEST.tsv"
            write_passing_hygiene(hygiene)
            write_passing_validation(validation)
            write_passing_benchmark(benchmark)
            corpus.write_text(
                "# fields\tpath\twarnings\n0\tsynthetic/a.docx\t-\n",
                encoding="utf-8",
            )
            render_corpus.write_text(
                "# path\tpages\twarnings\nsynthetic/a.docx\t1\t-\n",
                encoding="utf-8",
            )

            evidence = release_manifest.release_evidence_summary(
                "public-release",
                enforce_policy_inputs=False,
                hygiene_report=hygiene,
                validation_report=validation,
                benchmark_reports=[benchmark],
                corpus_manifests=[corpus, render_corpus],
            )

        self.assertEqual(evidence["strict_policy_status"], "missing_inputs")
        self.assertFalse(evidence["strict_policy_inputs_complete"])
        self.assertEqual(evidence["strict_missing"], ["valid public corpus manifests"])

    def test_release_evidence_marks_failing_hygiene_report_missing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            write_passing_validation(validation)
            write_passing_benchmark(benchmark)
            corpus, render_corpus = write_matching_public_manifests(root)
            hygiene.write_text(
                json.dumps(
                    {
                        "passed": False,
                        "findings": [
                            {
                                "path": "docs/private-note.md",
                                "line": 1,
                                "kind": "private_trace",
                                "detail": "private trace",
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )

            evidence = release_manifest.release_evidence_summary(
                "public-release",
                enforce_policy_inputs=False,
                hygiene_report=hygiene,
                validation_report=validation,
                benchmark_reports=[benchmark],
                corpus_manifests=[corpus, render_corpus],
            )

        self.assertEqual(evidence["strict_policy_status"], "missing_inputs")
        self.assertFalse(evidence["strict_policy_inputs_complete"])
        self.assertEqual(evidence["strict_missing"], ["passing hygiene report"])

    def test_release_evidence_marks_weak_policy_reports_missing(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            hygiene = root / "public-hygiene.json"
            corpus = root / "MANIFEST.tsv"
            render_corpus = root / "RENDER_MANIFEST.tsv"
            write_passing_hygiene(hygiene)
            write_public_corpus_document(root)
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1, "recall_min": 0.9},
                        "gate": {"passed": True, "checks": []},
                    }
                ),
                encoding="utf-8",
            )
            benchmark.write_text(
                json.dumps(
                    {
                        "summary": {
                            "files": 1,
                            "poi_recall_mean": 0.9,
                            "poi_f1_mean": 0.9,
                            "errors": 0,
                        },
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "poi_recall_mean",
                                    "op": ">=",
                                    "threshold": 0.9,
                                    "actual": 0.9,
                                    "passed": True,
                                }
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n",
                encoding="utf-8",
            )
            render_corpus.write_text(
                "# path\tpages\twarnings\nsynthetic/a.docx\t1\t-\n",
                encoding="utf-8",
            )

            evidence = release_manifest.release_evidence_summary(
                "public-release",
                enforce_policy_inputs=False,
                hygiene_report=hygiene,
                validation_report=validation,
                benchmark_reports=[benchmark],
                corpus_manifests=[corpus, render_corpus],
            )

        self.assertEqual(evidence["strict_policy_status"], "missing_inputs")
        self.assertFalse(evidence["strict_policy_inputs_complete"])
        self.assertEqual(
            evidence["strict_missing"],
            [
                "policy-strength validation report",
                "policy-strength benchmark report",
            ],
        )

    def test_enforced_public_release_policy_requires_local_reports(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            artifact.write_bytes(b"release artifact")

            with self.assertRaisesRegex(
                ValueError,
                "public-release requires hygiene report, validation report, benchmark report, corpus manifest",
            ):
                release_manifest.release_manifest(
                    [artifact],
                    release_policy="public-release",
                    enforce_policy_inputs=True,
                )

    def test_enforced_public_release_policy_rejects_missing_provided_evidence_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            artifact.write_bytes(b"release artifact")

            with self.assertRaisesRegex(
                ValueError,
                "public-release requires existing hygiene report, existing validation report, existing benchmark report, valid public corpus manifests",
            ):
                release_manifest.release_manifest(
                    [artifact],
                    release_policy="public-release",
                    enforce_policy_inputs=True,
                    hygiene_report=root / "public-hygiene.json",
                    validation_report=root / "render-validation.json",
                    benchmark_reports=[root / "extract-benchmark.json"],
                    corpus_manifests=[
                        root / "MANIFEST.tsv",
                        root / "RENDER_MANIFEST.tsv",
                    ],
                )

    def test_enforced_policy_inputs_requires_release_policy(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            artifact.write_bytes(b"release artifact")

            with self.assertRaisesRegex(
                ValueError,
                "enforce_policy_inputs requires release policy",
            ):
                release_manifest.release_manifest(
                    [artifact],
                    enforce_policy_inputs=True,
                )

    def test_enforced_public_release_policy_requires_both_public_corpus_manifests(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            corpus = root / "MANIFEST.tsv"
            artifact.write_bytes(b"release artifact")
            hygiene.write_text(
                json.dumps({"passed": True, "findings": []}),
                encoding="utf-8",
            )
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1, "mean_recall": 0.97},
                        "gate": {"passed": True, "checks": []},
                    }
                ),
                encoding="utf-8",
            )
            benchmark.write_text(
                json.dumps(
                    {
                        "summary": {"files": 1, "poi_recall_mean": 0.99},
                        "gate": {"passed": True, "checks": []},
                    }
                ),
                encoding="utf-8",
            )
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "public-release requires corpus manifests exactly MANIFEST.tsv and RENDER_MANIFEST.tsv",
            ):
                release_manifest.release_manifest(
                    [artifact],
                    release_policy="public-release",
                    enforce_policy_inputs=True,
                    hygiene_report=hygiene,
                    validation_report=validation,
                    benchmark_reports=[benchmark],
                    corpus_manifests=[corpus],
                )

    def test_enforced_public_release_policy_rejects_extra_corpus_manifests(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            corpus = root / "MANIFEST.tsv"
            render_corpus = root / "RENDER_MANIFEST.tsv"
            extra_corpus = root / "PRIVATE_MANIFEST.tsv"
            artifact.write_bytes(b"release artifact")
            hygiene.write_text(
                json.dumps({"passed": True, "findings": []}),
                encoding="utf-8",
            )
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1, "mean_recall": 0.97},
                        "gate": {"passed": True, "checks": []},
                    }
                ),
                encoding="utf-8",
            )
            benchmark.write_text(
                json.dumps(
                    {
                        "summary": {"files": 1, "poi_recall_mean": 0.99},
                        "gate": {"passed": True, "checks": []},
                    }
                ),
                encoding="utf-8",
            )
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n",
                encoding="utf-8",
            )
            render_corpus.write_text(
                "# path\tpages\twarnings\nsynthetic/a.docx\t1\t-\n",
                encoding="utf-8",
            )
            extra_corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/private.docx\t0\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "public-release requires corpus manifests exactly MANIFEST.tsv and RENDER_MANIFEST.tsv",
            ):
                release_manifest.release_manifest(
                    [artifact],
                    release_policy="public-release",
                    enforce_policy_inputs=True,
                    hygiene_report=hygiene,
                    validation_report=validation,
                    benchmark_reports=[benchmark],
                    corpus_manifests=[corpus, render_corpus, extra_corpus],
                )

    def test_enforced_public_release_policy_requires_matching_corpus_manifest_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            corpus = root / "MANIFEST.tsv"
            render_corpus = root / "RENDER_MANIFEST.tsv"
            write_passing_hygiene(hygiene)
            write_passing_validation(validation)
            write_passing_benchmark(benchmark)
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n",
                encoding="utf-8",
            )
            render_corpus.write_text(
                "# path\tpages\twarnings\nsynthetic/b.docx\t1\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "requires matching corpus manifest document paths",
            ):
                release_manifest.check_required_policy_inputs(
                    "public-release",
                    hygiene_report=hygiene,
                    validation_report=validation,
                    benchmark_reports=[benchmark],
                    corpus_manifests=[corpus, render_corpus],
                )

    def test_enforced_public_release_policy_requires_existing_corpus_documents(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            corpus = root / "MANIFEST.tsv"
            render_corpus = root / "RENDER_MANIFEST.tsv"
            write_passing_hygiene(hygiene)
            write_passing_validation(validation)
            write_passing_benchmark(benchmark)
            corpus.write_text(
                "# path\tfields\twarnings\nsynthetic/missing.docx\t0\t-\n",
                encoding="utf-8",
            )
            render_corpus.write_text(
                "# path\tpages\twarnings\nsynthetic/missing.docx\t1\t-\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "public-release requires existing public corpus documents",
            ):
                release_manifest.check_required_policy_inputs(
                    "public-release",
                    hygiene_report=hygiene,
                    validation_report=validation,
                    benchmark_reports=[benchmark],
                    corpus_manifests=[corpus, render_corpus],
                )

    def test_enforced_public_release_policy_requires_validation_report_covering_public_corpus(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            artifact.write_bytes(b"release artifact")
            write_passing_hygiene(hygiene)
            write_passing_validation(validation, documents=("other.docx",))
            write_passing_benchmark(benchmark)
            corpus, render_corpus = write_matching_public_manifests(root)

            with self.assertRaisesRegex(
                ValueError,
                "public-release requires validation report covering public corpus",
            ):
                release_manifest.release_manifest(
                    [artifact],
                    release_policy="public-release",
                    enforce_policy_inputs=True,
                    hygiene_report=hygiene,
                    validation_report=validation,
                    benchmark_reports=[benchmark],
                    corpus_manifests=[corpus, render_corpus],
                )

    def test_cli_enforced_policy_rejects_failing_gate_report(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            corpus = root / "MANIFEST.tsv"
            render_corpus = root / "RENDER_MANIFEST.tsv"
            artifact.write_bytes(b"release artifact")
            hygiene.write_text(
                json.dumps({"passed": True, "findings": []}),
                encoding="utf-8",
            )
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1, "mean_recall": 0.5},
                        "gate": {"passed": False, "checks": []},
                    }
                ),
                encoding="utf-8",
            )
            benchmark.write_text(
                json.dumps(
                    {
                        "summary": {"files": 1, "poi_recall_mean": 1.0},
                        "gate": {"passed": True, "checks": []},
                    }
                ),
                encoding="utf-8",
            )
            write_public_corpus_document(root)
            corpus.write_text("# path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n", encoding="utf-8")
            render_corpus.write_text("# path\tpages\twarnings\nsynthetic/a.docx\t1\t-\n", encoding="utf-8")

            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--release-policy",
                    "public-release",
                    "--enforce-policy-inputs",
                    "--hygiene-report",
                    str(hygiene),
                    "--validation-report",
                    str(validation),
                    "--benchmark-report",
                    str(benchmark),
                    "--corpus-manifest",
                    str(corpus),
                    "--corpus-manifest",
                    str(render_corpus),
                    str(artifact),
                ],
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(completed.returncode, 2)
            self.assertIn(
                "public-release validation report gate did not pass",
                completed.stderr,
            )

    def test_enforced_public_release_policy_rejects_weaker_validation_thresholds(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            corpus = root / "MANIFEST.tsv"
            render_corpus = root / "RENDER_MANIFEST.tsv"
            artifact.write_bytes(b"release artifact")
            hygiene.write_text(
                json.dumps({"passed": True, "findings": []}),
                encoding="utf-8",
            )
            validation.write_text(
                json.dumps(
                    {
                        "summary": {"documents": 1, "recall_min": 0.8, "mean_recall": 0.97},
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "mean_recall",
                                    "op": ">=",
                                    "threshold": 0.8,
                                    "actual": 0.97,
                                    "passed": True,
                                },
                                {
                                    "metric": "skipped",
                                    "op": "<=",
                                    "threshold": 0,
                                    "actual": 0,
                                    "passed": True,
                                },
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )
            benchmark.write_text(
                json.dumps(
                    {
                        "schema": "rwml.benchmark-report.v1",
                        "benchmark": "extract-vs-mature",
                        "summary": {
                            "files": 1,
                            "poi_recall_mean": 1.0,
                            "poi_f1_mean": 1.0,
                            "errors": 0,
                        },
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "poi_recall_mean",
                                    "op": ">=",
                                    "threshold": 0.95,
                                    "actual": 1.0,
                                    "passed": True,
                                },
                                {
                                    "metric": "poi_f1_mean",
                                    "op": ">=",
                                    "threshold": 0.95,
                                    "actual": 1.0,
                                    "passed": True,
                                },
                                {
                                    "metric": "errors",
                                    "op": "<=",
                                    "threshold": 0,
                                    "actual": 0,
                                    "passed": True,
                                },
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )
            write_public_corpus_document(root)
            corpus.write_text("# path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n", encoding="utf-8")
            render_corpus.write_text("# path\tpages\twarnings\nsynthetic/a.docx\t1\t-\n", encoding="utf-8")

            with self.assertRaisesRegex(
                ValueError,
                "public-release validation report summary recall_min must be at least 0.97",
            ):
                release_manifest.release_manifest(
                    [artifact],
                    release_policy="public-release",
                    enforce_policy_inputs=True,
                    hygiene_report=hygiene,
                    validation_report=validation,
                    benchmark_reports=[benchmark],
                    corpus_manifests=[corpus, render_corpus],
                )

    def test_enforced_public_release_policy_rejects_missing_benchmark_threshold(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            corpus = root / "MANIFEST.tsv"
            render_corpus = root / "RENDER_MANIFEST.tsv"
            artifact.write_bytes(b"release artifact")
            hygiene.write_text(
                json.dumps({"passed": True, "findings": []}),
                encoding="utf-8",
            )
            validation.write_text(
                json.dumps(
                    {
                        "summary": {
                            "documents": 1,
                            "recall_min": 0.97,
                            "below_recall_min": 0,
                            "mean_recall": 0.97,
                            "skipped": 0,
                        },
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "below_recall_min",
                                    "op": "<=",
                                    "threshold": 0,
                                    "actual": 0,
                                    "passed": True,
                                },
                                {
                                    "metric": "mean_recall",
                                    "op": ">=",
                                    "threshold": 0.9,
                                    "actual": 0.97,
                                    "passed": True,
                                },
                                {
                                    "metric": "skipped",
                                    "op": "<=",
                                    "threshold": 0,
                                    "actual": 0,
                                    "passed": True,
                                },
                            ],
                        },
                        "rows": [
                            {
                                "document": "a.docx",
                                "status": "pass",
                                "recall": 0.97,
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            benchmark.write_text(
                json.dumps(
                    {
                        "schema": "rwml.benchmark-report.v1",
                        "benchmark": "extract-vs-mature",
                        "summary": {
                            "files": 1,
                            "poi_recall_mean": 1.0,
                            "poi_f1_mean": 1.0,
                            "errors": 0,
                        },
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "poi_recall_mean",
                                    "op": ">=",
                                    "threshold": 0.95,
                                    "actual": 1.0,
                                    "passed": True,
                                },
                                {
                                    "metric": "errors",
                                    "op": "<=",
                                    "threshold": 0,
                                    "actual": 0,
                                    "passed": True,
                                },
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )
            write_public_corpus_document(root)
            corpus.write_text("# path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n", encoding="utf-8")
            render_corpus.write_text("# path\tpages\twarnings\nsynthetic/a.docx\t1\t-\n", encoding="utf-8")

            with self.assertRaisesRegex(
                ValueError,
                "public-release benchmark report gate must include poi_f1_mean >= 0.95",
            ):
                release_manifest.release_manifest(
                    [artifact],
                    release_policy="public-release",
                    enforce_policy_inputs=True,
                    hygiene_report=hygiene,
                    validation_report=validation,
                    benchmark_reports=[benchmark],
                    corpus_manifests=[corpus, render_corpus],
                )

    def test_enforced_public_release_policy_rejects_missing_benchmark_scored_threshold(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            artifact.write_bytes(b"release artifact")
            write_passing_hygiene(hygiene)
            write_passing_validation(validation)
            corpus, render_corpus = write_matching_public_manifests(root)
            benchmark.write_text(
                json.dumps(
                    {
                        "schema": "rwml.benchmark-report.v1",
                        "benchmark": "extract-vs-mature",
                        "summary": {
                            "files": 1,
                            "scored": 1,
                            "poi_recall_mean": 1.0,
                            "poi_f1_mean": 1.0,
                            "errors": 0,
                        },
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "poi_recall_mean",
                                    "op": ">=",
                                    "threshold": 0.95,
                                    "actual": 1.0,
                                    "passed": True,
                                },
                                {
                                    "metric": "poi_f1_mean",
                                    "op": ">=",
                                    "threshold": 0.95,
                                    "actual": 1.0,
                                    "passed": True,
                                },
                                {
                                    "metric": "errors",
                                    "op": "<=",
                                    "threshold": 0,
                                    "actual": 0,
                                    "passed": True,
                                },
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "public-release benchmark report gate must include scored >= 1",
            ):
                release_manifest.release_manifest(
                    [artifact],
                    release_policy="public-release",
                    enforce_policy_inputs=True,
                    hygiene_report=hygiene,
                    validation_report=validation,
                    benchmark_reports=[benchmark],
                    corpus_manifests=[corpus, render_corpus],
                )

    def test_enforced_public_release_policy_rejects_wrong_benchmark_identity(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            artifact.write_bytes(b"release artifact")
            write_passing_hygiene(hygiene)
            write_passing_validation(validation)
            corpus, render_corpus = write_matching_public_manifests(root)
            benchmark.write_text(
                json.dumps(
                    {
                        "schema": "rwml.benchmark-report.v1",
                        "benchmark": "other-benchmark",
                        "summary": {
                            "files": 1,
                            "poi_recall_mean": 1.0,
                            "poi_f1_mean": 1.0,
                            "errors": 0,
                        },
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "poi_recall_mean",
                                    "op": ">=",
                                    "threshold": 0.95,
                                    "actual": 1.0,
                                    "passed": True,
                                },
                                {
                                    "metric": "poi_f1_mean",
                                    "op": ">=",
                                    "threshold": 0.95,
                                    "actual": 1.0,
                                    "passed": True,
                                },
                                {
                                    "metric": "errors",
                                    "op": "<=",
                                    "threshold": 0,
                                    "actual": 0,
                                    "passed": True,
                                },
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )

            with self.assertRaisesRegex(
                ValueError,
                "public-release benchmark report benchmark must be extract-vs-mature",
            ):
                release_manifest.release_manifest(
                    [artifact],
                    release_policy="public-release",
                    enforce_policy_inputs=True,
                    hygiene_report=hygiene,
                    validation_report=validation,
                    benchmark_reports=[benchmark],
                    corpus_manifests=[corpus, render_corpus],
                )

    def test_enforced_public_release_policy_rejects_weak_benchmark_summary(self):
        report = {
            "summary": {"poi_recall_mean": 0.5, "poi_f1_mean": 1.0, "errors": 0, "scored": 1},
            "gate": {
                "passed": True,
                "checks": [
                    {
                        "metric": "poi_recall_mean",
                        "op": ">=",
                        "threshold": 0.95,
                        "actual": 0.99,
                        "passed": True,
                    },
                    {
                        "metric": "poi_f1_mean",
                        "op": ">=",
                        "threshold": 0.95,
                        "actual": 1.0,
                        "passed": True,
                    },
                    {
                        "metric": "errors",
                        "op": "<=",
                        "threshold": 0,
                        "actual": 0,
                        "passed": True,
                    },
                    {
                        "metric": "scored",
                        "op": ">=",
                        "threshold": 1,
                        "actual": 1,
                        "passed": True,
                    },
                ],
            },
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release benchmark report summary poi_recall_mean must be at least 0.95",
        ):
            release_manifest.require_public_release_report_thresholds(
                "public-release",
                report,
                "benchmark",
            )

    def test_enforced_public_release_policy_rejects_empty_benchmark_summary(self):
        report = {
            "summary": {
                "files": 1,
                "scored": 0,
                "poi_recall_mean": 1.0,
                "poi_f1_mean": 1.0,
                "errors": 0,
            },
            "gate": {
                "passed": True,
                "checks": [
                    {
                        "metric": "poi_recall_mean",
                        "op": ">=",
                        "threshold": 0.95,
                        "actual": 1.0,
                        "passed": True,
                    },
                    {
                        "metric": "poi_f1_mean",
                        "op": ">=",
                        "threshold": 0.95,
                        "actual": 1.0,
                        "passed": True,
                    },
                    {
                        "metric": "errors",
                        "op": "<=",
                        "threshold": 0,
                        "actual": 0,
                        "passed": True,
                    },
                    {
                        "metric": "scored",
                        "op": ">=",
                        "threshold": 1,
                        "actual": 1,
                        "passed": True,
                    },
                ],
            },
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release benchmark report summary scored must be at least 1",
        ):
            release_manifest.require_public_release_report_thresholds(
                "public-release",
                report,
                "benchmark",
            )

    def test_enforced_public_release_policy_rejects_weak_validation_summary(self):
        report = {
            "summary": {
                "recall_min": 0.97,
                "below_recall_min": 0,
                "mean_recall": 0.5,
                "skipped": 0,
            },
            "gate": {
                "passed": True,
                "checks": [
                    {
                        "metric": "below_recall_min",
                        "op": "<=",
                        "threshold": 0,
                        "actual": 0,
                        "passed": True,
                    },
                    {
                        "metric": "mean_recall",
                        "op": ">=",
                        "threshold": 0.9,
                        "actual": 0.99,
                        "passed": True,
                    },
                    {
                        "metric": "skipped",
                        "op": "<=",
                        "threshold": 0,
                        "actual": 0,
                        "passed": True,
                    },
                ],
            },
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release validation report summary mean_recall must be at least 0.9",
        ):
            release_manifest.require_public_release_report_thresholds(
                "public-release",
                report,
                "validation",
            )

    def test_enforced_public_release_policy_rejects_boolean_summary_metric(self):
        report = {
            "summary": {"poi_recall_mean": True},
            "gate": {"passed": True, "checks": []},
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release benchmark report summary poi_recall_mean must be at least 0.95",
        ):
            release_manifest.require_summary_threshold_at_least(
                "public-release",
                report,
                "benchmark",
                "poi_recall_mean",
                0.95,
            )

    def test_enforced_public_release_policy_rejects_negative_summary_count_metric(self):
        report = {
            "summary": {"skipped": -1},
            "gate": {"passed": True, "checks": []},
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release validation report summary skipped must not be negative",
        ):
            release_manifest.require_summary_threshold_at_most(
                "public-release",
                report,
                "validation",
                "skipped",
                0,
            )

    def test_enforced_public_release_policy_rejects_summary_score_above_one(self):
        report = {
            "summary": {"mean_recall": 1.1},
            "gate": {"passed": True, "checks": []},
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release validation report summary mean_recall must not be above one",
        ):
            release_manifest.require_summary_threshold_at_least(
                "public-release",
                report,
                "validation",
                "mean_recall",
                0.9,
            )

    def test_enforced_public_release_policy_rejects_boolean_gate_threshold(self):
        report = {
            "summary": {"documents": 1, "recall_min": 0.97, "mean_recall": 1.0},
            "gate": {
                "passed": True,
                "checks": [
                    {
                        "metric": "mean_recall",
                        "op": ">=",
                        "threshold": True,
                        "actual": True,
                        "passed": True,
                    }
                ],
            },
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release validation report gate must include mean_recall >= 0.9",
        ):
            release_manifest.require_gate_check_threshold(
                "public-release",
                report,
                "validation",
                "mean_recall",
                ">=",
                0.9,
            )

    def test_enforced_public_release_policy_rejects_negative_gate_count_threshold(self):
        report = {
            "summary": {"documents": 1, "recall_min": 0.97, "skipped": 0},
            "gate": {
                "passed": True,
                "checks": [
                    {
                        "metric": "skipped",
                        "op": "<=",
                        "threshold": -1,
                        "actual": 0,
                        "passed": True,
                    }
                ],
            },
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release validation report gate check threshold must not be negative: skipped",
        ):
            release_manifest.require_gate_check_threshold(
                "public-release",
                report,
                "validation",
                "skipped",
                "<=",
                0,
            )

    def test_enforced_public_release_policy_rejects_negative_gate_count_actual(self):
        report = {
            "summary": {"documents": 1, "recall_min": 0.97, "skipped": -1},
            "gate": {
                "passed": True,
                "checks": [
                    {
                        "metric": "skipped",
                        "op": "<=",
                        "threshold": 0,
                        "actual": -1,
                        "passed": True,
                    }
                ],
            },
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release validation report gate check actual must not be negative: skipped",
        ):
            release_manifest.require_gate_check_threshold(
                "public-release",
                report,
                "validation",
                "skipped",
                "<=",
                0,
            )

    def test_enforced_public_release_policy_rejects_gate_score_threshold_above_one(self):
        report = {
            "summary": {"documents": 1, "recall_min": 0.97, "mean_recall": 1.0},
            "gate": {
                "passed": True,
                "checks": [
                    {
                        "metric": "mean_recall",
                        "op": ">=",
                        "threshold": 1.1,
                        "actual": 1.0,
                        "passed": True,
                    }
                ],
            },
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release validation report gate check threshold must not be above one: mean_recall",
        ):
            release_manifest.require_gate_check_threshold(
                "public-release",
                report,
                "validation",
                "mean_recall",
                ">=",
                0.9,
            )

    def test_enforced_public_release_policy_rejects_gate_score_actual_above_one(self):
        report = {
            "summary": {"documents": 1, "recall_min": 0.97, "mean_recall": 1.1},
            "gate": {
                "passed": True,
                "checks": [
                    {
                        "metric": "mean_recall",
                        "op": ">=",
                        "threshold": 0.9,
                        "actual": 1.1,
                        "passed": True,
                    }
                ],
            },
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release validation report gate check actual must not be above one: mean_recall",
        ):
            release_manifest.require_gate_check_threshold(
                "public-release",
                report,
                "validation",
                "mean_recall",
                ">=",
                0.9,
            )

    def test_enforced_public_release_policy_rejects_nan_summary_metric(self):
        report = {
            "summary": {"poi_recall_mean": float("nan")},
            "gate": {"passed": True, "checks": []},
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release benchmark report summary poi_recall_mean must be at least 0.95",
        ):
            release_manifest.require_summary_threshold_at_least(
                "public-release",
                report,
                "benchmark",
                "poi_recall_mean",
                0.95,
            )

    def test_enforced_public_release_policy_rejects_infinite_gate_actual(self):
        report = {
            "summary": {"documents": 1, "recall_min": 0.97, "mean_recall": 1.0},
            "gate": {
                "passed": True,
                "checks": [
                    {
                        "metric": "mean_recall",
                        "op": ">=",
                        "threshold": 0.9,
                        "actual": float("inf"),
                        "passed": True,
                    }
                ],
            },
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release validation report gate check actual failed policy threshold: mean_recall >= 0.9",
        ):
            release_manifest.require_gate_check_threshold(
                "public-release",
                report,
                "validation",
                "mean_recall",
                ">=",
                0.9,
            )

    def test_enforced_public_release_policy_rejects_failed_policy_threshold_check(self):
        report = {
            "summary": {"documents": 1, "recall_min": 0.97, "mean_recall": 0.50},
            "gate": {
                "passed": True,
                "checks": [
                    {
                        "metric": "mean_recall",
                        "op": ">=",
                        "threshold": 0.90,
                        "actual": 0.50,
                        "passed": False,
                    }
                ],
            },
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release validation report gate check did not pass: mean_recall >= 0.9",
        ):
            release_manifest.require_gate_check_threshold(
                "public-release",
                report,
                "validation",
                "mean_recall",
                ">=",
                0.90,
            )

    def test_enforced_public_release_policy_rejects_inconsistent_policy_threshold_actual(self):
        report = {
            "summary": {"documents": 1, "recall_min": 0.97, "mean_recall": 0.50},
            "gate": {
                "passed": True,
                "checks": [
                    {
                        "metric": "mean_recall",
                        "op": ">=",
                        "threshold": 0.90,
                        "actual": 0.50,
                        "passed": True,
                    }
                ],
            },
        }

        with self.assertRaisesRegex(
            ValueError,
            "public-release validation report gate check actual failed policy threshold: mean_recall >= 0.9",
        ):
            release_manifest.require_gate_check_threshold(
                "public-release",
                report,
                "validation",
                "mean_recall",
                ">=",
                0.90,
            )

    def test_enforced_public_release_policy_accepts_passing_local_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            artifact = root / "rwml.tar.gz"
            hygiene = root / "public-hygiene.json"
            validation = root / "render-validation.json"
            benchmark = root / "extract-benchmark.json"
            corpus = root / "MANIFEST.tsv"
            render_corpus = root / "RENDER_MANIFEST.tsv"
            artifact.write_bytes(b"release artifact")
            hygiene.write_text(
                json.dumps(
                    {
                        "schema": "rwml.public-hygiene-audit.v1",
                        "passed": True,
                        "findings": [],
                    }
                ),
                encoding="utf-8",
            )
            validation.write_text(
                json.dumps(
                    {
                        "summary": {
                            "documents": 1,
                            "recall_min": 0.97,
                            "below_recall_min": 0,
                            "mean_recall": 0.97,
                            "skipped": 0,
                        },
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "below_recall_min",
                                    "op": "<=",
                                    "threshold": 0,
                                    "actual": 0,
                                    "passed": True,
                                },
                                {
                                    "metric": "mean_recall",
                                    "op": ">=",
                                    "threshold": 0.9,
                                    "actual": 0.97,
                                    "passed": True,
                                },
                                {
                                    "metric": "skipped",
                                    "op": "<=",
                                    "threshold": 0,
                                    "actual": 0,
                                    "passed": True,
                                },
                            ],
                        },
                        "rows": [
                            {
                                "document": "a.docx",
                                "status": "pass",
                                "recall": 0.97,
                            }
                        ],
                    }
                ),
                encoding="utf-8",
            )
            benchmark.write_text(
                json.dumps(
                    {
                        "schema": "rwml.benchmark-report.v1",
                        "benchmark": "extract-vs-mature",
                        "summary": {
                            "files": 1,
                            "scored": 1,
                            "poi_recall_mean": 0.99,
                            "poi_f1_mean": 0.99,
                            "errors": 0,
                        },
                        "gate": {
                            "passed": True,
                            "checks": [
                                {
                                    "metric": "poi_recall_mean",
                                    "op": ">=",
                                    "threshold": 0.95,
                                    "actual": 0.99,
                                    "passed": True,
                                },
                                {
                                    "metric": "poi_f1_mean",
                                    "op": ">=",
                                    "threshold": 0.95,
                                    "actual": 0.99,
                                    "passed": True,
                                },
                                {
                                    "metric": "scored",
                                    "op": ">=",
                                    "threshold": 1,
                                    "actual": 1,
                                    "passed": True,
                                },
                                {
                                    "metric": "errors",
                                    "op": "<=",
                                    "threshold": 0,
                                    "actual": 0,
                                    "passed": True,
                                },
                            ],
                        },
                    }
                ),
                encoding="utf-8",
            )
            write_public_corpus_document(root)
            corpus.write_text("# path\tfields\twarnings\nsynthetic/a.docx\t0\t-\n", encoding="utf-8")
            render_corpus.write_text("# path\tpages\twarnings\nsynthetic/a.docx\t1\t-\n", encoding="utf-8")

            manifest = release_manifest.release_manifest(
                [artifact],
                release_policy="public-release",
                enforce_policy_inputs=True,
                hygiene_report=hygiene,
                validation_report=validation,
                benchmark_reports=[benchmark],
                corpus_manifests=[corpus, render_corpus],
            )

        self.assertEqual(manifest["release_policy"]["name"], "public-release")
        self.assertEqual(
            manifest["release_evidence"],
            {
                "policy": "public-release",
                "strict_policy_status": "enforced",
                "strict_policy_enforced": True,
                "strict_policy_inputs_complete": True,
                "strict_missing": [],
                "provided": {
                    "hygiene_report": hygiene.as_posix(),
                    "validation_report": validation.as_posix(),
                    "benchmark_reports": [benchmark.as_posix()],
                    "corpus_manifests": [corpus.as_posix(), render_corpus.as_posix()],
                },
            },
        )
        self.assertEqual(
            manifest["hygiene"],
            {
                "path": hygiene.as_posix(),
                "gate": {"passed": True, "findings": 0},
            },
        )
        self.assertEqual(manifest["validation"]["gate"]["passed"], True)
        self.assertEqual(manifest["benchmarks"][0]["gate"]["passed"], True)
        self.assertEqual(
            [item["summary"]["documents"] for item in manifest["corpus_manifests"]],
            [1, 1],
        )


if __name__ == "__main__":
    unittest.main()
