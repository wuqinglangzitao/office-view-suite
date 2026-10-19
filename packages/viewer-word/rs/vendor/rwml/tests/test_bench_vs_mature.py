import importlib.util
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "scripts" / "bench_vs_mature.py"
SPEC = importlib.util.spec_from_file_location("bench_vs_mature", SCRIPT)
bench_vs_mature = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = bench_vs_mature
SPEC.loader.exec_module(bench_vs_mature)


class BenchVsMatureReportTests(unittest.TestCase):
    def test_cli_help_is_ascii_safe_under_cp949(self):
        env = os.environ.copy()
        env["PYTHONIOENCODING"] = "cp949"

        completed = subprocess.run(
            [sys.executable, str(SCRIPT), "--help"],
            check=False,
            capture_output=True,
            text=True,
            env=env,
        )

        self.assertEqual(completed.returncode, 0, completed.stderr)
        completed.stdout.encode("ascii")

    def test_explicit_windows_extract_binary_is_revision_bound(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            repo = root / "repo"
            binary = (
                root
                / "preflight"
                / "cargo-target"
                / "release"
                / "examples"
                / "extract.exe"
            )
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"exact binary")

            resolved = bench_vs_mature.resolve_extract_binary(repo, binary)

            self.assertEqual(resolved, binary)

    def test_extract_binary_follows_cargo_target_dir_on_windows(self):
        with tempfile.TemporaryDirectory() as tmp:
            repo = pathlib.Path(tmp)
            binary = (
                repo
                / "custom-target"
                / "release"
                / "examples"
                / "extract.exe"
            )
            binary.parent.mkdir(parents=True)
            binary.write_bytes(b"target binary")

            resolved = bench_vs_mature.resolve_extract_binary(
                repo,
                None,
                environ={"CARGO_TARGET_DIR": "custom-target"},
                platform_name="nt",
            )

            self.assertEqual(resolved, binary)

    def test_clean_golden_removes_bom_and_logging_noise(self):
        text = (
            "\ufeffFirst token\n"
            "ERROR StatusLogger Log4j2 could not find a logging implementation\n"
            "Second token\n"
        )

        self.assertEqual(
            bench_vs_mature.clean_golden(text),
            "First token\nSecond token",
        )

    def test_benchmark_report_summarizes_rows_with_release_metadata(self):
        rows = [
            {
                "file": "alpha",
                "poi_recall": 1.0,
                "poi_prec": 0.8,
                "poi_f1": 0.888888,
                "lo_recall": 0.75,
                "lo_prec": 0.6,
                "lo_f1": 0.666666,
            },
            {"file": "broken", "rwml": "ERROR"},
            {
                "file": "beta",
                "poi_recall": 0.5,
                "poi_prec": 1.0,
                "poi_f1": 0.666666,
            },
        ]

        report = bench_vs_mature.benchmark_report(
            rows,
            version="0.1.0",
            git_rev="abc123",
        )

        self.assertEqual(report["schema"], "rwml.benchmark-report.v1")
        self.assertEqual(report["benchmark"], "extract-vs-mature")
        self.assertEqual(report["version"], "0.1.0")
        self.assertEqual(report["git_rev"], "abc123")
        self.assertEqual(report["summary"]["files"], 3)
        self.assertEqual(report["summary"]["scored"], 2)
        self.assertEqual(report["summary"]["errors"], 1)
        self.assertEqual(report["summary"]["poi_recall_mean"], 0.75)
        self.assertEqual(report["summary"]["poi_recall_median"], 0.75)
        self.assertEqual(report["summary"]["poi_f1_mean"], 0.7778)
        self.assertEqual(report["summary"]["lo_scored"], 1)
        self.assertEqual(report["summary"]["lo_recall_mean"], 0.75)
        self.assertEqual(report["rows"], rows)
        self.assertNotIn("corpus", report)

    def test_benchmark_report_rejects_malformed_release_metadata(self):
        rows = [{"file": "alpha", "poi_recall": 1.0, "poi_f1": 1.0}]
        cases = [
            ("version", {"version": 1}, "version must be a string"),
            ("version", {"version": ""}, "version must not be empty"),
            ("version", {"version": "0.1.0 beta"}, "version must not contain whitespace"),
            ("git_rev", {"git_rev": " abc123"}, "git_rev must not have surrounding whitespace"),
        ]
        for label, kwargs, message in cases:
            with self.subTest(label=label, message=message):
                with self.assertRaisesRegex(ValueError, message):
                    bench_vs_mature.benchmark_report(rows, **kwargs)

    def test_benchmark_report_rejects_file_paths(self):
        rows = [{"file": "private" + "/alpha", "poi_recall": 1.0, "poi_f1": 1.0}]

        with self.assertRaisesRegex(ValueError, "file path is invalid"):
            bench_vs_mature.benchmark_report(rows)

    def test_benchmark_report_rejects_missing_files(self):
        rows = [{"poi_recall": 1.0, "poi_f1": 1.0}]

        with self.assertRaisesRegex(ValueError, "file is required"):
            bench_vs_mature.benchmark_report(rows)

    def test_benchmark_report_rejects_empty_or_padded_files(self):
        cases = [
            ("", "file must not be empty"),
            (" alpha", "file must not have surrounding whitespace"),
        ]
        for file_name, message in cases:
            with self.subTest(file=file_name):
                rows = [{"file": file_name, "poi_recall": 1.0, "poi_f1": 1.0}]

                with self.assertRaisesRegex(ValueError, message):
                    bench_vs_mature.benchmark_report(rows)

    def test_benchmark_report_rejects_non_string_files(self):
        rows = [{"file": 1, "poi_recall": 1.0, "poi_f1": 1.0}]

        try:
            bench_vs_mature.benchmark_report(rows)
        except ValueError as exc:
            self.assertRegex(str(exc), "file must be a string")
        except Exception as exc:
            self.fail(f"expected ValueError, got {type(exc).__name__}: {exc}")
        else:
            self.fail("ValueError not raised")

    def test_benchmark_report_rejects_non_numeric_scores(self):
        rows = [{"file": "alpha", "poi_recall": "1.0", "poi_f1": 1.0}]

        try:
            bench_vs_mature.benchmark_report(rows)
        except ValueError as exc:
            self.assertRegex(str(exc), "score is invalid: poi_recall")
        except Exception as exc:
            self.fail(f"expected ValueError, got {type(exc).__name__}: {exc}")
        else:
            self.fail("ValueError not raised")

    def test_benchmark_report_rejects_out_of_range_scores(self):
        rows = [{"file": "alpha", "poi_recall": 1.1, "poi_f1": 1.0}]

        with self.assertRaisesRegex(ValueError, "score is out of range: poi_recall"):
            bench_vs_mature.benchmark_report(rows)

    def test_benchmark_report_rejects_invalid_rwml_markers(self):
        rows = [{"file": "broken", "rwml": "FAIL"}]

        with self.assertRaisesRegex(ValueError, "rwml marker is invalid"):
            bench_vs_mature.benchmark_report(rows)

    def test_benchmark_report_rejects_scored_error_rows(self):
        rows = [{"file": "broken", "rwml": "ERROR", "poi_recall": 1.0}]

        with self.assertRaisesRegex(ValueError, "error row has scores"):
            bench_vs_mature.benchmark_report(rows)

    def test_benchmark_report_rejects_unclassified_rows(self):
        rows = [{"file": "empty"}]

        with self.assertRaisesRegex(ValueError, "row has no score or error marker"):
            bench_vs_mature.benchmark_report(rows)

    def test_benchmark_report_evaluates_release_thresholds(self):
        rows = [
            {
                "file": "alpha",
                "poi_recall": 1.0,
                "poi_prec": 0.8,
                "poi_f1": 0.888888,
                "lo_recall": 0.75,
                "lo_prec": 0.6,
                "lo_f1": 0.666666,
            },
            {"file": "broken", "rwml": "ERROR"},
            {
                "file": "beta",
                "poi_recall": 0.5,
                "poi_prec": 1.0,
                "poi_f1": 0.666666,
            },
        ]

        report = bench_vs_mature.benchmark_report(
            rows,
            thresholds={
                "min_poi_recall_mean": 0.8,
                "min_poi_f1_mean": 0.8,
                "min_lo_recall_mean": 0.7,
                "max_errors": 0,
                "min_scored": 3,
                "max_scored": 3,
                "min_lo_scored": 2,
                "max_lo_scored": 2,
            },
        )

        self.assertFalse(report["gate"]["passed"])
        checks = {
            (check["metric"], check["op"]): check
            for check in report["gate"]["checks"]
        }
        self.assertEqual(checks[("poi_recall_mean", ">=")]["actual"], 0.75)
        self.assertEqual(checks[("poi_recall_mean", ">=")]["threshold"], 0.8)
        self.assertFalse(checks[("poi_recall_mean", ">=")]["passed"])
        self.assertEqual(checks[("poi_f1_mean", ">=")]["actual"], 0.7778)
        self.assertFalse(checks[("poi_f1_mean", ">=")]["passed"])
        self.assertEqual(checks[("lo_recall_mean", ">=")]["actual"], 0.75)
        self.assertTrue(checks[("lo_recall_mean", ">=")]["passed"])
        self.assertEqual(checks[("errors", "<=")]["actual"], 1)
        self.assertFalse(checks[("errors", "<=")]["passed"])
        self.assertEqual(checks[("scored", ">=")]["actual"], 2)
        self.assertFalse(checks[("scored", ">=")]["passed"])
        self.assertTrue(checks[("scored", "<=")]["passed"])
        self.assertEqual(checks[("lo_scored", ">=")]["actual"], 1)
        self.assertFalse(checks[("lo_scored", ">=")]["passed"])
        self.assertTrue(checks[("lo_scored", "<=")]["passed"])

    def test_benchmark_gate_rejects_non_finite_thresholds(self):
        with self.assertRaisesRegex(ValueError, "non-finite threshold"):
            bench_vs_mature.benchmark_gate(
                {"poi_recall_mean": 1.0},
                {"min_poi_recall_mean": float("nan")},
            )

    def test_benchmark_gate_rejects_negative_count_thresholds(self):
        with self.assertRaisesRegex(ValueError, "negative count threshold"):
            bench_vs_mature.benchmark_gate({"scored": 0}, {"min_scored": -1})

    def test_benchmark_gate_rejects_negative_score_thresholds(self):
        with self.assertRaisesRegex(ValueError, "negative score threshold"):
            bench_vs_mature.benchmark_gate(
                {"poi_recall_mean": 1.0},
                {"min_poi_recall_mean": -0.1},
            )

    def test_benchmark_gate_rejects_score_thresholds_above_one(self):
        with self.assertRaisesRegex(ValueError, "score threshold above one"):
            bench_vs_mature.benchmark_gate(
                {"poi_f1_mean": 1.0},
                {"min_poi_f1_mean": 1.1},
            )

    def test_exact_three_oracle_gate_rejects_corpus_growth_without_policy_update(self):
        gate = bench_vs_mature.benchmark_gate(
            {"scored": 4, "lo_scored": 4},
            {
                "min_scored": 3,
                "max_scored": 3,
                "min_lo_scored": 3,
                "max_lo_scored": 3,
            },
        )

        self.assertFalse(gate["passed"])
        failed = {
            (check["metric"], check["op"])
            for check in gate["checks"]
            if not check["passed"]
        }
        self.assertEqual(failed, {("scored", "<="), ("lo_scored", "<=")})

    def test_write_json_report_rejects_non_finite_values(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = pathlib.Path(tmp) / "benchmark.json"

            with self.assertRaisesRegex(ValueError, "Out of range float values"):
                bench_vs_mature.write_json_report(
                    {"summary": {"poi_recall_mean": float("nan")}},
                    output,
                )

            self.assertFalse(output.exists())

    @staticmethod
    def write_legacy_corpus(root, names=("alpha", "beta", "gamma")):
        for directory in ("sample", "sample-poi", "sample-lo"):
            (root / directory).mkdir(parents=True, exist_ok=True)
        rows = ["# path\tparagraphs\ttables\tfigures\ttext_chars\twarnings"]
        for name in names:
            (root / "sample" / f"{name}.doc").write_bytes(b"doc")
            (root / "sample-poi" / f"{name}.poi.txt").write_text(
                name, encoding="utf-8"
            )
            (root / "sample-lo" / f"{name}.txt").write_text(
                name, encoding="utf-8"
            )
            rows.append(f"sample/{name}.doc\t1\t0\t0\t{len(name)}\tPackageReadOnly")
        (root / "LEGACY_MANIFEST.tsv").write_text(
            "\n".join(rows) + "\n", encoding="utf-8"
        )

    def test_public_legacy_manifest_resolves_exactly_three_complete_inputs(self):
        corpus = (
            pathlib.Path(__file__).resolve().parents[1]
            / "corpus"
            / "public"
            / "benchmark"
        )

        inputs = bench_vs_mature.legacy_benchmark_inputs(corpus)

        self.assertEqual(len(inputs), 3)
        self.assertEqual(
            {item.name for item in inputs},
            {"floating_text_bearing", "floating_wrap_policy", "nested_tables"},
        )

    def test_legacy_manifest_accepts_complete_exact_inventory(self):
        with tempfile.TemporaryDirectory() as tmp:
            corpus = pathlib.Path(tmp)
            self.write_legacy_corpus(corpus)

            inputs = bench_vs_mature.legacy_benchmark_inputs(corpus)

            self.assertEqual([item.name for item in inputs], ["alpha", "beta", "gamma"])

    def test_legacy_manifest_rejects_missing_source_poi_or_libreoffice_input(self):
        cases = [
            ("DOC", pathlib.Path("sample/alpha.doc")),
            ("Apache POI", pathlib.Path("sample-poi/alpha.poi.txt")),
            ("LibreOffice", pathlib.Path("sample-lo/alpha.txt")),
        ]
        for label, relative in cases:
            with self.subTest(label=label), tempfile.TemporaryDirectory() as tmp:
                corpus = pathlib.Path(tmp)
                self.write_legacy_corpus(corpus)
                (corpus / relative).unlink()

                with self.assertRaisesRegex(ValueError, f"{label} inventory mismatch"):
                    bench_vs_mature.legacy_benchmark_inputs(corpus)

    def test_legacy_manifest_rejects_unexpected_source_or_golden(self):
        cases = [
            ("DOC", pathlib.Path("sample/extra.doc")),
            ("Apache POI", pathlib.Path("sample-poi/extra.poi.txt")),
            ("LibreOffice", pathlib.Path("sample-lo/extra.txt")),
        ]
        for label, relative in cases:
            with self.subTest(label=label), tempfile.TemporaryDirectory() as tmp:
                corpus = pathlib.Path(tmp)
                self.write_legacy_corpus(corpus)
                (corpus / relative).write_bytes(b"extra")

                with self.assertRaisesRegex(ValueError, f"{label} inventory mismatch"):
                    bench_vs_mature.legacy_benchmark_inputs(corpus)


if __name__ == "__main__":
    unittest.main()
