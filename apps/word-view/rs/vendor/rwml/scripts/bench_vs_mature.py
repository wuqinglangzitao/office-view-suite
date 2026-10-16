#!/usr/bin/env python3
"""Head-to-head: rwml vs mature extractors on a local .doc corpus.

Compares rwml's plain-text extraction against two mature references on the same
`.doc` sample:

  * Apache POI  — `WordExtractor.getText()` golden (sample-poi/*.poi.txt)
  * LibreOffice — `--convert-to txt` golden       (sample-lo/*.txt)

For each file it reports whitespace-token set recall (fraction of the reference's
tokens rwml also produced), precision, and F1 — apples-to-apples because all three
are tokenized identically. The script reports measurements only; it does not
decide which extractor is "better".

  python scripts/bench_vs_mature.py --corpus DIR [--limit N] [--json]
  python scripts/bench_vs_mature.py --corpus corpus/public/benchmark --json --version 0.1.1 \
    --git-rev "$(git rev-parse HEAD)" --min-poi-recall-mean 0.95 \
    --min-poi-f1-mean 0.95 --min-lo-recall-mean 0.95 --max-errors 0 \
    --min-scored 3 --max-scored 3 --min-lo-scored 3 --max-lo-scored 3 \
    --output dist/extract-benchmark.json

The corpus directory must contain an authoritative ``LEGACY_MANIFEST.tsv`` and:
  sample-poi/*.poi.txt        Apache POI golden output
  sample-lo/*.txt             LibreOffice golden output
  sample/*.doc or govdocs/files/*.doc

You can also set RWML_BENCH_CORPUS instead of passing --corpus. Needs the release
`extract` example built:
  cargo build --release --example extract
"""

from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from pathlib import PurePosixPath


SCHEMA = "rwml.benchmark-report.v1"
BENCHMARK = "extract-vs-mature"
COUNT_THRESHOLD_METRICS = {"errors", "scored", "lo_scored"}
SCORE_THRESHOLD_METRICS = {"poi_recall_mean", "poi_f1_mean", "lo_recall_mean"}


@dataclass(frozen=True)
class LegacyBenchmarkInput:
    name: str
    document: Path
    poi_golden: Path
    libreoffice_golden: Path


def safe_manifest_doc(raw: str) -> PurePosixPath:
    relative = PurePosixPath(raw)
    if (
        not raw
        or "\\" in raw
        or ":" in raw
        or relative.is_absolute()
        or relative.suffix != ".doc"
        or any(part in {"", ".", ".."} for part in relative.parts)
    ):
        raise ValueError(f"invalid legacy DOC manifest path: {raw!r}")
    return relative


def require_exact_inventory(label: str, expected: set[str], actual: set[str]) -> None:
    if expected == actual:
        return
    missing = sorted(expected - actual)
    unexpected = sorted(actual - expected)
    raise ValueError(
        f"legacy benchmark {label} inventory mismatch: "
        f"missing={missing} unexpected={unexpected}"
    )


def legacy_benchmark_inputs(corpus: Path) -> list[LegacyBenchmarkInput]:
    if not corpus.is_dir():
        raise ValueError(f"legacy benchmark corpus does not exist: {corpus}")
    manifest = corpus / "LEGACY_MANIFEST.tsv"
    if not manifest.is_file():
        raise ValueError(f"legacy benchmark manifest is missing: {manifest}")

    documents: list[PurePosixPath] = []
    seen_paths: set[PurePosixPath] = set()
    seen_names: set[str] = set()
    for line_number, line in enumerate(
        manifest.read_text(encoding="utf-8").splitlines(), start=1
    ):
        line = line.strip()
        if not line or line.startswith("#") or line.startswith("path\t"):
            continue
        relative = safe_manifest_doc(line.split("\t", 1)[0])
        if relative in seen_paths:
            raise ValueError(
                f"{manifest}:{line_number} repeats DOC path: {relative}"
            )
        if relative.stem in seen_names:
            raise ValueError(
                f"{manifest}:{line_number} repeats DOC basename: {relative.stem}"
            )
        seen_paths.add(relative)
        seen_names.add(relative.stem)
        documents.append(relative)
    if not documents:
        raise ValueError(f"legacy benchmark manifest contains no DOC inputs: {manifest}")

    expected_documents = {str(path) for path in documents}
    actual_documents = {
        path.relative_to(corpus).as_posix()
        for path in corpus.rglob("*.doc")
        if path.is_file()
    }
    require_exact_inventory("DOC", expected_documents, actual_documents)

    expected_poi = {f"{path.stem}.poi.txt" for path in documents}
    poi_dir = corpus / "sample-poi"
    actual_poi = (
        {
            path.relative_to(poi_dir).as_posix()
            for path in poi_dir.rglob("*.poi.txt")
            if path.is_file()
        }
        if poi_dir.is_dir()
        else set()
    )
    require_exact_inventory("Apache POI", expected_poi, actual_poi)

    expected_lo = {f"{path.stem}.txt" for path in documents}
    lo_dir = corpus / "sample-lo"
    actual_lo = (
        {
            path.relative_to(lo_dir).as_posix()
            for path in lo_dir.rglob("*.txt")
            if path.is_file()
        }
        if lo_dir.is_dir()
        else set()
    )
    require_exact_inventory("LibreOffice", expected_lo, actual_lo)

    return [
        LegacyBenchmarkInput(
            name=relative.stem,
            document=corpus / Path(*relative.parts),
            poi_golden=poi_dir / f"{relative.stem}.poi.txt",
            libreoffice_golden=lo_dir / f"{relative.stem}.txt",
        )
        for relative in sorted(documents, key=str)
    ]


def resolve_extract_binary(
    repo: Path,
    explicit: Path | None,
    *,
    environ: dict[str, str] | None = None,
    platform_name: str | None = None,
) -> Path:
    environ = os.environ if environ is None else environ
    platform_name = os.name if platform_name is None else platform_name
    if explicit is not None:
        candidate = explicit if explicit.is_absolute() else repo / explicit
        if not candidate.is_file():
            raise ValueError(f"extract binary does not exist: {candidate}")
        return candidate

    target_dir = Path(environ.get("CARGO_TARGET_DIR", "target"))
    if not target_dir.is_absolute():
        target_dir = repo / target_dir
    base = target_dir / "release" / "examples" / "extract"
    candidates = (
        [base.with_suffix(".exe"), base]
        if platform_name == "nt"
        else [base, base.with_suffix(".exe")]
    )
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    raise ValueError(
        "extract binary does not exist under the active Cargo target directory; "
        "build it with `cargo build --release --example extract --locked` or pass "
        "--extract-bin"
    )


def is_finite_number(value: object) -> bool:
    return (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(value)
    )


def clean_golden(s: str) -> str:
    """Strip extractor artifacts that aren't document text: a UTF-8/UTF-16 BOM
    (LibreOffice prepends one, merging with the first token) and Apache POI's
    Log4j2 'could not find a logging implementation' startup lines."""
    s = s.replace("﻿", "")
    keep = [
        ln
        for ln in s.splitlines()
        if "Log4j" not in ln and "StatusLogger" not in ln
    ]
    return "\n".join(keep)


def toks(s: str) -> set[str]:
    return set(clean_golden(s).split())


def prf(ref: set[str], got: set[str]) -> tuple[float, float, float]:
    if not ref:
        return (1.0, 1.0, 1.0)
    inter = len(ref & got)
    recall = inter / len(ref)
    precision = inter / len(got) if got else 0.0
    f1 = (2 * recall * precision / (recall + precision)) if (recall + precision) else 0.0
    return (recall, precision, f1)


def rwml_text(extract_bin: Path, doc: Path) -> str | None:
    r = subprocess.run([str(extract_bin), str(doc)], capture_output=True)
    if r.returncode != 0:
        return None
    return r.stdout.decode("utf-8", "replace")


def mean_metric(rows: list[dict], key: str) -> float:
    values = [r[key] for r in rows if key in r]
    return round(sum(values) / len(values), 4) if values else 0


def median_metric(rows: list[dict], key: str) -> float:
    values = sorted(r[key] for r in rows if key in r)
    if not values:
        return 0
    mid = len(values) // 2
    if len(values) % 2:
        return round(values[mid], 4)
    return round((values[mid - 1] + values[mid]) / 2, 4)


def benchmark_summary(rows: list[dict]) -> dict:
    ok = [r for r in rows if "poi_recall" in r]
    return {
        "files": len(rows),
        "scored": len(ok),
        "errors": sum(1 for r in rows if r.get("rwml") == "ERROR"),
        "poi_recall_mean": mean_metric(ok, "poi_recall"),
        "poi_f1_mean": mean_metric(ok, "poi_f1"),
        "poi_recall_median": median_metric(ok, "poi_recall"),
        "lo_scored": sum(1 for r in ok if "lo_recall" in r),
        "lo_recall_mean": mean_metric(ok, "lo_recall"),
    }


def add_threshold_check(
    checks: list[dict],
    metric: str,
    actual: float | int | None,
    op: str,
    threshold: float | int | None,
) -> None:
    if threshold is None:
        return
    if not is_finite_number(threshold):
        raise ValueError(f"non-finite threshold for {metric}: {threshold}")
    if metric in COUNT_THRESHOLD_METRICS and threshold < 0:
        raise ValueError(f"negative count threshold for {metric}: {threshold}")
    if metric in SCORE_THRESHOLD_METRICS and threshold < 0:
        raise ValueError(f"negative score threshold for {metric}: {threshold}")
    if metric in SCORE_THRESHOLD_METRICS and threshold > 1:
        raise ValueError(f"score threshold above one for {metric}: {threshold}")
    if actual is None:
        passed = False
    elif op == ">=":
        passed = actual >= threshold
    elif op == "<=":
        passed = actual <= threshold
    else:
        raise ValueError(f"unsupported threshold operator: {op}")
    checks.append(
        {
            "metric": metric,
            "op": op,
            "threshold": threshold,
            "actual": actual,
            "passed": passed,
        }
    )


def benchmark_gate(summary: dict, thresholds: dict | None = None) -> dict | None:
    thresholds = thresholds or {}
    checks = []
    add_threshold_check(
        checks,
        "poi_recall_mean",
        summary.get("poi_recall_mean"),
        ">=",
        thresholds.get("min_poi_recall_mean"),
    )
    add_threshold_check(
        checks,
        "poi_f1_mean",
        summary.get("poi_f1_mean"),
        ">=",
        thresholds.get("min_poi_f1_mean"),
    )
    add_threshold_check(
        checks,
        "lo_recall_mean",
        summary.get("lo_recall_mean"),
        ">=",
        thresholds.get("min_lo_recall_mean"),
    )
    add_threshold_check(
        checks,
        "errors",
        summary.get("errors"),
        "<=",
        thresholds.get("max_errors"),
    )
    add_threshold_check(
        checks,
        "scored",
        summary.get("scored"),
        ">=",
        thresholds.get("min_scored"),
    )
    add_threshold_check(
        checks,
        "scored",
        summary.get("scored"),
        "<=",
        thresholds.get("max_scored"),
    )
    add_threshold_check(
        checks,
        "lo_scored",
        summary.get("lo_scored"),
        ">=",
        thresholds.get("min_lo_scored"),
    )
    add_threshold_check(
        checks,
        "lo_scored",
        summary.get("lo_scored"),
        "<=",
        thresholds.get("max_lo_scored"),
    )
    if not checks:
        return None
    return {"passed": all(check["passed"] for check in checks), "checks": checks}


def benchmark_report(
    rows: list[dict],
    *,
    version: str | None = None,
    git_rev: str | None = None,
    thresholds: dict | None = None,
) -> dict:
    for label, value in (("version", version), ("git_rev", git_rev)):
        if value is not None and not isinstance(value, str):
            raise ValueError(f"{label} must be a string")
        if value is not None and not value.strip():
            raise ValueError(f"{label} must not be empty")
        if value is not None and value != value.strip():
            raise ValueError(f"{label} must not have surrounding whitespace")
        if value is not None and any(char.isspace() for char in value):
            raise ValueError(f"{label} must not contain whitespace")
    for row in rows:
        if "file" not in row:
            raise ValueError("file is required")
        file_name = row["file"]
        if not isinstance(file_name, str):
            raise ValueError("file must be a string")
        if not file_name.strip():
            raise ValueError("file must not be empty")
        if file_name != file_name.strip():
            raise ValueError(f"file must not have surrounding whitespace: {file_name}")
        if "/" in file_name or "\\" in file_name:
            raise ValueError(f"file path is invalid: {file_name}")
        if "rwml" in row and row["rwml"] != "ERROR":
            raise ValueError("rwml marker is invalid")
        for metric in (
            "poi_recall",
            "poi_prec",
            "poi_f1",
            "lo_recall",
            "lo_prec",
            "lo_f1",
        ):
            if row.get("rwml") == "ERROR" and metric in row:
                raise ValueError("error row has scores")
            if metric in row and not is_finite_number(row[metric]):
                raise ValueError(f"score is invalid: {metric}")
            if metric in row and not 0 <= row[metric] <= 1:
                raise ValueError(f"score is out of range: {metric}")
        if row.get("rwml") != "ERROR" and "poi_recall" not in row:
            raise ValueError("row has no score or error marker")
    summary = benchmark_summary(rows)
    report = {
        "schema": SCHEMA,
        "benchmark": BENCHMARK,
        "summary": summary,
        "rows": rows,
    }
    if version is not None:
        report["version"] = version
    if git_rev is not None:
        report["git_rev"] = git_rev
    gate = benchmark_gate(summary, thresholds)
    if gate is not None:
        report["gate"] = gate
    return report


def write_json_report(report: dict, output: Path | None) -> None:
    payload = json.dumps(report, ensure_ascii=False, indent=2, allow_nan=False) + "\n"
    if output is None:
        sys.stdout.write(payload)
    else:
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(payload, encoding="utf-8")


def main() -> int:
    # Keep --help ASCII-only so it is printable on Windows consoles using
    # legacy code pages such as cp949. The module docstring remains the detailed
    # design/reference documentation.
    ap = argparse.ArgumentParser(
        description="Compare rwml legacy DOC extraction with Apache POI and LibreOffice."
    )
    ap.add_argument("--corpus", type=Path, default=None)
    ap.add_argument(
        "--extract-bin",
        type=Path,
        help="exact release-mode rwml extract example to execute",
    )
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--version", help="release version string to include in JSON output")
    ap.add_argument("--git-rev", help="git revision to include in JSON output")
    ap.add_argument("--output", type=Path, help="write JSON report to this path")
    ap.add_argument("--min-poi-recall-mean", type=float)
    ap.add_argument("--min-poi-f1-mean", type=float)
    ap.add_argument("--min-lo-recall-mean", type=float)
    ap.add_argument("--max-errors", type=int)
    ap.add_argument("--min-scored", type=int)
    ap.add_argument("--max-scored", type=int)
    ap.add_argument("--min-lo-scored", type=int)
    ap.add_argument("--max-lo-scored", type=int)
    args = ap.parse_args()
    if args.corpus is None:
        env_corpus = os.environ.get("RWML_BENCH_CORPUS")
        if not env_corpus:
            sys.exit("pass --corpus DIR or set RWML_BENCH_CORPUS")
        args.corpus = Path(env_corpus)

    repo = Path(__file__).resolve().parent.parent
    try:
        extract_bin = resolve_extract_binary(repo, args.extract_bin)
        inputs = legacy_benchmark_inputs(args.corpus)
    except (OSError, ValueError) as error:
        print(f"bench_vs_mature: {error}", file=sys.stderr)
        return 2
    if args.limit:
        inputs = inputs[: args.limit]

    rows = []
    for benchmark_input in inputs:
        got = rwml_text(extract_bin, benchmark_input.document)
        if got is None:
            rows.append({"file": benchmark_input.name, "rwml": "ERROR"})
            continue
        gt = toks(got)
        poi = benchmark_input.poi_golden.read_text(encoding="utf-8", errors="replace")
        rp, pp, fp = prf(toks(poi), gt)
        row = {
            "file": benchmark_input.name,
            "poi_recall": round(rp, 4),
            "poi_prec": round(pp, 4),
            "poi_f1": round(fp, 4),
        }
        lo = benchmark_input.libreoffice_golden.read_text(
            encoding="utf-8", errors="replace"
        )
        rl, pl, fl = prf(toks(lo), gt)
        row.update(
            {
                "lo_recall": round(rl, 4),
                "lo_prec": round(pl, 4),
                "lo_f1": round(fl, 4),
            }
        )
        rows.append(row)

    thresholds = {
        "min_poi_recall_mean": args.min_poi_recall_mean,
        "min_poi_f1_mean": args.min_poi_f1_mean,
        "min_lo_recall_mean": args.min_lo_recall_mean,
        "max_errors": args.max_errors,
        "min_scored": args.min_scored,
        "max_scored": args.max_scored,
        "min_lo_scored": args.min_lo_scored,
        "max_lo_scored": args.max_lo_scored,
    }
    report = benchmark_report(
        rows,
        version=args.version,
        git_rev=args.git_rev,
        thresholds=thresholds,
    )
    summary = report["summary"]

    if args.json or args.output is not None:
        write_json_report(report, args.output)
        return 0 if report.get("gate", {"passed": True})["passed"] else 1

    print(f"{'file':16} {'POI rec':>8} {'POI F1':>8} {'LO rec':>8}")
    print("-" * 46)
    for r in rows:
        if "poi_recall" not in r:
            print(f"{r['file']:16} {'ERROR':>8}")
            continue
        lo = f"{r.get('lo_recall', float('nan')):8.3f}" if "lo_recall" in r else f"{'—':>8}"
        print(f"{r['file']:16} {r['poi_recall']:8.3f} {r['poi_f1']:8.3f} {lo}")
    print("-" * 46)
    print(
        f"rwml vs Apache POI: recall {summary['poi_recall_mean']:.3f} mean / "
        f"{summary['poi_recall_median']:.3f} median, F1 {summary['poi_f1_mean']:.3f} "
        f"over {summary['scored']} files ({summary['errors']} errors)"
    )
    print(f"rwml vs LibreOffice: recall {summary['lo_recall_mean']:.3f} mean")
    failures = [
        check
        for check in report.get("gate", {}).get("checks", [])
        if not check["passed"]
    ]
    for check in failures:
        print(
            "threshold failed: "
            f"{check['metric']} {check['op']} {check['threshold']} "
            f"(actual {check['actual']})"
        )
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
