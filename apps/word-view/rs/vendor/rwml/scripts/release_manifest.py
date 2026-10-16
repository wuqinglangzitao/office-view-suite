#!/usr/bin/env python3
"""Create a deterministic release artifact manifest with SHA-256 checksums.

This is a packaging helper for release automation. It does not build artifacts;
it records the artifacts a release job already produced and, optionally, embeds
the summary and compact gate section from a validation report such as
`scripts/render_validate.py --json`. Extraction benchmark reports from
`scripts/bench_vs_mature.py --json` can be attached the same way; only their
summaries and gate metadata are embedded. Public hygiene audit reports from
`scripts/public_hygiene_audit.py --json` and public corpus TSV manifests can also
be summarized without copying row data. When a release policy is named, the
manifest also records whether strict local policy evidence was enforced.

Example:

  python scripts/release_manifest.py \
    --version 0.1.1 \
    --git-rev "$(git rev-parse HEAD)" \
    --release-policy public-release \
    --enforce-policy-inputs \
    --corpus-manifest corpus/public/MANIFEST.tsv \
    --corpus-manifest corpus/public/RENDER_MANIFEST.tsv \
    --hygiene-report public-hygiene.json \
    --validation-report render-report.json \
    --benchmark-report extract-benchmark.json \
    --output dist/rwml-release-manifest.json \
    dist/rwml-aarch64-apple-darwin.tar.gz dist/rwml.wasm
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
import math
import sys
from pathlib import Path, PurePosixPath
from typing import Any


SCHEMA = "rwml.release-manifest.v1"
PUBLIC_RELEASE_CORPUS_MANIFESTS = ("MANIFEST.tsv", "RENDER_MANIFEST.tsv")
PUBLIC_RELEASE_BENCHMARK_SCHEMA = "rwml.benchmark-report.v1"
PUBLIC_RELEASE_BENCHMARK_NAME = "extract-vs-mature"
COUNT_POLICY_METRICS = {"below_recall_min", "skipped", "errors", "scored"}
BOUNDED_SCORE_POLICY_METRICS = {"recall_min", "mean_recall", "poi_recall_mean", "poi_f1_mean"}
KNOWN_WARNING_TOKENS = {
    "UnsupportedFieldEvaluation",
    "TrackedChangesPresent",
    "IncompleteRevisionView",
    "FloatingShapePlaceholderOnly",
    "ChartsPreservedButNotModeled",
    "OleObjectsPreservedButNotModeled",
    "UnsupportedMetafileImages",
    "LegacyDocFlattenedSubdocuments",
    "PackageReadOnly",
    "MissingImageBytes",
    "UndecodableRasterImages",
}
BOOLEAN_SUMMARY_KEYS = {"reference_stable"}
STRING_LIST_SUMMARY_KEYS = {"unstable_references"}
RELEASE_POLICIES: dict[str, dict[str, Any]] = {
    "public-release": {
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
                "min_mean_recall": 0.90,
                "max_skipped": 0,
            },
            "public_corpus": {
                "manifest_match": "exact",
            },
        },
    }
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def artifact_record(path: Path) -> dict[str, Any]:
    stat = path.stat()
    return {
        "name": path.name,
        "path": path.as_posix(),
        "bytes": stat.st_size,
        "sha256": sha256_file(path),
    }


def path_sort_key(path: Path) -> str:
    return path.as_posix()


def load_json(path: Path) -> Any:
    try:
        payload = path.read_text(encoding="utf-8")
    except UnicodeDecodeError as error:
        raise ValueError(f"{path}: invalid UTF-8 JSON") from error
    try:
        return json.loads(payload)
    except json.JSONDecodeError as error:
        raise ValueError(
            f"{path}: invalid JSON at line {error.lineno} column {error.colno}: "
            f"{error.msg}"
        ) from error


def require_unique_paths(label: str, paths: list[Path] | None) -> None:
    seen: set[Path] = set()
    for path in paths or []:
        key = path.resolve()
        if key in seen:
            raise ValueError(f"duplicate {label} path: {path.as_posix()}")
        seen.add(key)


def is_number(value: Any) -> bool:
    return (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(value)
    )


def report_summary(path: Path) -> dict[str, Any]:
    data = load_json(path)
    if not isinstance(data, dict):
        raise ValueError(f"{path} does not contain a JSON object")
    summary = data.get("summary")
    if not isinstance(summary, dict):
        raise ValueError(f"{path} does not contain a JSON object field named 'summary'")
    if not summary:
        raise ValueError(f"{path} summary must not be empty")
    for key in summary:
        if not key or not key.isascii() or not key.isidentifier():
            raise ValueError(f"{path} summary key is invalid: {key}")
    try:
        json.dumps(summary, allow_nan=False)
    except ValueError as error:
        raise ValueError(f"{path} summary contains non-finite value") from error
    for key, value in summary.items():
        if value is None:
            continue
        if key in BOOLEAN_SUMMARY_KEYS:
            if not isinstance(value, bool):
                raise ValueError(f"{path} summary value is invalid: {key}")
        elif key in STRING_LIST_SUMMARY_KEYS:
            if not isinstance(value, list) or any(
                not isinstance(item, str) for item in value
            ):
                raise ValueError(f"{path} summary value is invalid: {key}")
        elif not is_number(value):
            raise ValueError(f"{path} summary value is invalid: {key}")
    report = {"path": path.as_posix(), "summary": summary}
    gate = data.get("gate")
    if gate is not None and not isinstance(gate, dict):
        raise ValueError(f"{path} gate is not a JSON object")
    if gate is not None:
        gate_fields = ("passed", "checks")
        for key in gate:
            if (
                not key
                or not key.isascii()
                or not key.isidentifier()
                or key not in gate_fields
            ):
                raise ValueError(f"{path} gate key is invalid: {key}")
        if not isinstance(gate.get("passed"), bool):
            raise ValueError(f"{path} gate passed is not a boolean")
        if not isinstance(gate.get("checks"), list):
            raise ValueError(f"{path} gate checks is not a list")
        if any(not isinstance(check, dict) for check in gate["checks"]):
            raise ValueError(f"{path} gate check is not a JSON object")
        gate_check_fields = ("metric", "op", "threshold", "actual", "passed")
        seen_gate_checks: set[tuple[str, str]] = set()
        for check in gate["checks"]:
            for key in check:
                if (
                    not key
                    or not key.isascii()
                    or not key.isidentifier()
                    or key not in gate_check_fields
                ):
                    raise ValueError(f"{path} gate check key is invalid: {key}")
            for field in gate_check_fields:
                if field not in check:
                    raise ValueError(
                        f"{path} gate check missing required field: {field}"
                    )
            if not isinstance(check["metric"], str):
                raise ValueError(f"{path} gate check metric is not a string")
            if (
                not check["metric"]
                or check["metric"] != check["metric"].strip()
                or not check["metric"].isascii()
                or not check["metric"].isidentifier()
            ):
                raise ValueError(f"{path} gate check metric is invalid")
            if not isinstance(check["op"], str):
                raise ValueError(f"{path} gate check op is not a string")
            if check["op"] not in {">=", "<="}:
                raise ValueError(f"{path} unsupported gate check operator: {check['op']}")
            gate_check_key = (check["metric"], check["op"])
            if gate_check_key in seen_gate_checks:
                raise ValueError(
                    f"{path} duplicate gate check: {check['metric']} {check['op']}"
                )
            seen_gate_checks.add(gate_check_key)
            if not is_number(check["threshold"]):
                raise ValueError(f"{path} gate check threshold is not a finite number")
            if check["actual"] is not None and not is_number(check["actual"]):
                raise ValueError(f"{path} gate check actual is not a finite number")
        if any(not isinstance(check.get("passed"), bool) for check in gate["checks"]):
            raise ValueError(f"{path} gate check passed is not a boolean")
        if gate["passed"] and any(not check["passed"] for check in gate["checks"]):
            raise ValueError(f"{path} gate passed with failed checks")
        if (
            not gate["passed"]
            and gate["checks"]
            and all(check["passed"] for check in gate["checks"])
        ):
            raise ValueError(f"{path} gate failed without failed checks")
        try:
            json.dumps(gate, allow_nan=False)
        except ValueError as error:
            raise ValueError(f"{path} gate contains non-finite value") from error
        report["gate"] = gate
    return report


def require_public_release_benchmark_identity(policy: str, path: Path) -> None:
    if policy != "public-release":
        return
    data = load_json(path)
    if not isinstance(data, dict):
        raise ValueError(f"{path} does not contain a JSON object")
    if data.get("schema") != PUBLIC_RELEASE_BENCHMARK_SCHEMA:
        raise ValueError(
            f"{policy} benchmark report schema must be {PUBLIC_RELEASE_BENCHMARK_SCHEMA}"
        )
    if data.get("benchmark") != PUBLIC_RELEASE_BENCHMARK_NAME:
        raise ValueError(
            f"{policy} benchmark report benchmark must be {PUBLIC_RELEASE_BENCHMARK_NAME}"
        )


def validation_summary(path: Path | None) -> dict[str, Any] | None:
    if path is None:
        return None
    return report_summary(path)


def hygiene_summary(path: Path | None) -> dict[str, Any] | None:
    if path is None:
        return None
    data = load_json(path)
    if not isinstance(data, dict):
        raise ValueError(f"{path} does not contain a JSON object")
    passed = data.get("passed")
    findings = data.get("findings")
    if not isinstance(passed, bool):
        raise ValueError(f"{path} does not contain a boolean field named 'passed'")
    if not isinstance(findings, list):
        raise ValueError(f"{path} does not contain a list field named 'findings'")
    if any(not isinstance(finding, dict) for finding in findings):
        raise ValueError(f"{path} hygiene finding is not an object")
    if passed and findings:
        raise ValueError(f"{path} cannot pass with hygiene findings")
    if not passed and not findings:
        raise ValueError(f"{path} cannot fail without hygiene findings")
    finding_fields = ("path", "line", "kind", "detail")
    seen_findings: set[tuple[str, int | None, str, str]] = set()
    for finding in findings:
        for field in finding_fields:
            if field not in finding:
                raise ValueError(
                    f"{path} hygiene finding missing required field: {field}"
                )
        for field in finding:
            if field not in finding_fields:
                raise ValueError(f"{path} hygiene finding key is invalid: {field}")
        if not isinstance(finding["path"], str):
            raise ValueError(f"{path} hygiene finding path is invalid")
        if not finding["path"] or finding["path"] != finding["path"].strip():
            raise ValueError(f"{path} hygiene finding path is invalid")
        if (
            finding["path"].startswith(("/", "\\"))
            or "\\" in finding["path"]
            or (
                len(finding["path"]) >= 3
                and finding["path"][1] == ":"
                and finding["path"][2] == "/"
            )
        ):
            raise ValueError(f"{path} hygiene finding path is invalid")
        if not (
            finding["line"] is None
            or (
                isinstance(finding["line"], int)
                and not isinstance(finding["line"], bool)
                and finding["line"] > 0
            )
        ):
            raise ValueError(f"{path} hygiene finding line is invalid")
        if not isinstance(finding["kind"], str):
            raise ValueError(f"{path} hygiene finding kind is invalid")
        if (
            not finding["kind"]
            or finding["kind"] != finding["kind"].strip()
            or not finding["kind"].isascii()
            or not finding["kind"].isidentifier()
        ):
            raise ValueError(f"{path} hygiene finding kind is invalid")
        if not isinstance(finding["detail"], str):
            raise ValueError(f"{path} hygiene finding detail is invalid")
        if not finding["detail"] or finding["detail"] != finding["detail"].strip():
            raise ValueError(f"{path} hygiene finding detail is invalid")
        finding_key = (
            finding["path"],
            finding["line"],
            finding["kind"],
            finding["detail"],
        )
        if finding_key in seen_findings:
            raise ValueError(f"{path} duplicate hygiene finding")
        seen_findings.add(finding_key)
    return {
        "path": path.as_posix(),
        "gate": {
            "passed": passed,
            "findings": len(findings),
        },
    }


def benchmark_summaries(paths: list[Path] | None) -> list[dict[str, Any]]:
    return [report_summary(path) for path in sorted(paths or [], key=path_sort_key)]


def release_policy_summary(name: str | None) -> dict[str, Any] | None:
    if name is None:
        return None
    try:
        return copy.deepcopy(RELEASE_POLICIES[name])
    except KeyError as error:
        raise ValueError(f"unknown release policy: {name}") from error


def check_required_policy_inputs(
    name: str | None,
    *,
    hygiene_report: Path | None,
    validation_report: Path | None,
    benchmark_reports: list[Path] | None,
    corpus_manifests: list[Path] | None,
) -> None:
    if name is None:
        return
    if name not in RELEASE_POLICIES:
        raise ValueError(f"unknown release policy: {name}")
    if name != "public-release":
        return

    missing = []
    if hygiene_report is None:
        missing.append("hygiene report")
    if validation_report is None:
        missing.append("validation report")
    if not benchmark_reports:
        missing.append("benchmark report")
    if not corpus_manifests:
        missing.append("corpus manifest")
    if missing:
        raise ValueError(f"{name} requires {', '.join(missing)}")

    if not public_release_corpus_manifest_pair_matches(corpus_manifests or []):
        required = " and ".join(PUBLIC_RELEASE_CORPUS_MANIFESTS)
        raise ValueError(f"{name} requires corpus manifests exactly {required}")

    if hygiene_report is not None and not hygiene_report.is_file():
        missing.append("existing hygiene report")
    if validation_report is not None and not validation_report.is_file():
        missing.append("existing validation report")
    if benchmark_reports is not None and not all(
        path.is_file() for path in benchmark_reports
    ):
        missing.append("existing benchmark report")
    if corpus_manifests is not None and not all(
        path.is_file() for path in corpus_manifests
    ):
        missing.append("valid public corpus manifests")
    if missing:
        raise ValueError(f"{name} requires {', '.join(missing)}")
    require_public_release_corpus_manifest_pair(name, corpus_manifests or [])


def public_release_policy_input_gaps(
    *,
    hygiene_report: Path | None,
    validation_report: Path | None,
    benchmark_reports: list[Path] | None,
    corpus_manifests: list[Path] | None,
) -> list[str]:
    missing = []
    corpus_ready_for_validation = False
    if hygiene_report is None:
        missing.append("hygiene report")
    if validation_report is None:
        missing.append("validation report")
    if not benchmark_reports:
        missing.append("benchmark report")
    if not corpus_manifests:
        missing.append("corpus manifest")
    elif not public_release_corpus_manifest_pair_matches(corpus_manifests):
        missing.append("exact public corpus manifest pair")
    elif not all(path.is_file() for path in corpus_manifests):
        missing.append("valid public corpus manifests")
    elif all(path.is_file() for path in corpus_manifests):
        try:
            if not public_release_corpus_manifest_documents_match(corpus_manifests):
                missing.append("matching public corpus manifest documents")
            elif not public_release_corpus_manifest_document_files_exist(
                corpus_manifests
            ):
                missing.append("existing public corpus documents")
            else:
                corpus_ready_for_validation = True
        except (OSError, UnicodeDecodeError, ValueError):
            missing.append("valid public corpus manifests")
    report_strength_missing = public_release_report_strength_gaps(
        hygiene_report=hygiene_report,
        validation_report=validation_report,
        benchmark_reports=benchmark_reports,
    )
    missing.extend(report_strength_missing)
    if (
        corpus_ready_for_validation
        and validation_report is not None
        and validation_report.is_file()
        and "policy-strength validation report" not in report_strength_missing
    ):
        try:
            require_public_release_validation_report_coverage(
                "public-release",
                validation_report,
                corpus_manifests or [],
            )
        except (OSError, UnicodeDecodeError, json.JSONDecodeError, ValueError):
            missing.append("validation report covering public corpus")
    return missing


def public_release_report_strength_gaps(
    *,
    hygiene_report: Path | None,
    validation_report: Path | None,
    benchmark_reports: list[Path] | None,
) -> list[str]:
    missing = []
    if hygiene_report is not None:
        if not hygiene_report.is_file():
            missing.append("passing hygiene report")
        else:
            try:
                hygiene = hygiene_summary(hygiene_report)
                require_report_gate_passed("public-release", hygiene, "hygiene")
            except (OSError, json.JSONDecodeError, ValueError):
                missing.append("passing hygiene report")
    if validation_report is not None:
        if not validation_report.is_file():
            missing.append("policy-strength validation report")
        else:
            try:
                validation = validation_summary(validation_report)
                require_report_gate_passed("public-release", validation, "validation")
                require_public_release_report_thresholds(
                    "public-release",
                    validation,
                    "validation",
                )
            except (OSError, json.JSONDecodeError, ValueError):
                missing.append("policy-strength validation report")
    benchmark_strength_missing = False
    for benchmark_report in benchmark_reports or []:
        if not benchmark_report.is_file():
            benchmark_strength_missing = True
            continue
        try:
            require_public_release_benchmark_identity(
                "public-release", benchmark_report
            )
            benchmark = report_summary(benchmark_report)
            require_report_gate_passed("public-release", benchmark, "benchmark")
            require_public_release_report_thresholds(
                "public-release",
                benchmark,
                "benchmark",
            )
        except (OSError, json.JSONDecodeError, ValueError):
            benchmark_strength_missing = True
    if benchmark_strength_missing:
        missing.append("policy-strength benchmark report")
    return missing


def public_release_corpus_manifest_pair_matches(corpus_manifests: list[Path]) -> bool:
    provided = [path.name for path in corpus_manifests]
    return len(provided) == len(PUBLIC_RELEASE_CORPUS_MANIFESTS) and set(provided) == set(
        PUBLIC_RELEASE_CORPUS_MANIFESTS
    )


def public_release_corpus_manifest_documents_match(corpus_manifests: list[Path]) -> bool:
    by_name = {path.name: path for path in corpus_manifests}
    manifest_paths = corpus_manifest_document_paths(by_name["MANIFEST.tsv"])
    render_manifest_paths = corpus_manifest_document_paths(by_name["RENDER_MANIFEST.tsv"])
    return manifest_paths == render_manifest_paths


def public_release_corpus_manifest_document_files_exist(
    corpus_manifests: list[Path],
) -> bool:
    for manifest in corpus_manifests:
        for document_path in corpus_manifest_document_paths(manifest):
            if not (manifest.parent / document_path).is_file():
                return False
    return True


def public_release_render_manifest_document_names(
    corpus_manifests: list[Path],
) -> set[str]:
    by_name = {path.name: path for path in corpus_manifests}
    document_names = [
        PurePosixPath(document_path).name
        for document_path in corpus_manifest_document_paths(
            by_name["RENDER_MANIFEST.tsv"]
        )
    ]
    if len(set(document_names)) != len(document_names):
        raise ValueError(
            "public-release requires unique public render manifest document names"
        )
    return set(document_names)


def validation_report_document_names(path: Path) -> set[str]:
    data = load_json(path)
    if not isinstance(data, dict):
        raise ValueError(f"{path} does not contain a JSON object")
    rows = data.get("rows")
    if not isinstance(rows, list):
        raise ValueError(f"{path} validation report rows are required")
    if not rows:
        raise ValueError(f"{path} validation report rows must not be empty")

    document_names: set[str] = set()
    for row in rows:
        if not isinstance(row, dict):
            raise ValueError(f"{path} validation report row is not a JSON object")
        document = row.get("document")
        if not isinstance(document, str):
            raise ValueError(f"{path} validation report row document is invalid")
        if (
            not document
            or document != document.strip()
            or "/" in document
            or "\\" in document
            or ":" in document
            or any(char.isspace() for char in document)
        ):
            raise ValueError(f"{path} validation report row document is invalid")
        if document in document_names:
            raise ValueError(f"{path} duplicate validation report document: {document}")
        document_names.add(document)
    return document_names


def public_release_validation_report_documents_match(
    validation_report: Path,
    corpus_manifests: list[Path],
) -> bool:
    return validation_report_document_names(
        validation_report
    ) == public_release_render_manifest_document_names(corpus_manifests)


def require_public_release_validation_report_coverage(
    name: str,
    validation_report: Path,
    corpus_manifests: list[Path],
) -> None:
    try:
        documents_match = public_release_validation_report_documents_match(
            validation_report,
            corpus_manifests,
        )
    except (OSError, UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        raise ValueError(
            f"{name} requires validation report covering public corpus"
        ) from error
    if not documents_match:
        raise ValueError(f"{name} requires validation report covering public corpus")


def require_public_release_corpus_manifest_pair(name: str, corpus_manifests: list[Path]) -> None:
    required = " and ".join(PUBLIC_RELEASE_CORPUS_MANIFESTS)
    if not public_release_corpus_manifest_pair_matches(corpus_manifests):
        raise ValueError(f"{name} requires corpus manifests exactly {required}")
    if not public_release_corpus_manifest_documents_match(corpus_manifests):
        raise ValueError(f"{name} requires matching corpus manifest document paths")
    if not public_release_corpus_manifest_document_files_exist(corpus_manifests):
        raise ValueError(f"{name} requires existing public corpus documents")


def require_report_gate_passed(policy: str, report: dict[str, Any], label: str) -> None:
    gate = report.get("gate")
    if not isinstance(gate, dict):
        raise ValueError(f"{policy} {label} report does not contain a gate result")
    if gate.get("passed") is not True:
        raise ValueError(f"{policy} {label} report gate did not pass")


def require_public_release_report_thresholds(
    policy: str,
    report: dict[str, Any],
    label: str,
) -> None:
    if policy != "public-release":
        return
    if label == "validation":
        thresholds = RELEASE_POLICIES[policy]["optional_local_gates"]["render_validation"]
        require_summary_threshold_at_least(
            policy,
            report,
            label,
            "recall_min",
            thresholds["recall_min"],
        )
        require_summary_threshold_at_most(
            policy,
            report,
            label,
            "below_recall_min",
            0,
        )
        require_gate_check_threshold(
            policy,
            report,
            label,
            "below_recall_min",
            "<=",
            0,
        )
        require_gate_check_threshold(
            policy,
            report,
            label,
            "mean_recall",
            ">=",
            thresholds["min_mean_recall"],
        )
        require_summary_threshold_at_least(
            policy,
            report,
            label,
            "mean_recall",
            thresholds["min_mean_recall"],
        )
        require_gate_check_threshold(
            policy,
            report,
            label,
            "skipped",
            "<=",
            thresholds["max_skipped"],
        )
        require_summary_threshold_at_most(
            policy,
            report,
            label,
            "skipped",
            thresholds["max_skipped"],
        )
    elif label == "benchmark":
        thresholds = RELEASE_POLICIES[policy]["optional_local_gates"]["extraction_benchmark"]
        require_gate_check_threshold(
            policy,
            report,
            label,
            "poi_recall_mean",
            ">=",
            thresholds["min_poi_recall_mean"],
        )
        require_summary_threshold_at_least(
            policy,
            report,
            label,
            "poi_recall_mean",
            thresholds["min_poi_recall_mean"],
        )
        require_gate_check_threshold(
            policy,
            report,
            label,
            "poi_f1_mean",
            ">=",
            thresholds["min_poi_f1_mean"],
        )
        require_summary_threshold_at_least(
            policy,
            report,
            label,
            "poi_f1_mean",
            thresholds["min_poi_f1_mean"],
        )
        require_gate_check_threshold(
            policy,
            report,
            label,
            "errors",
            "<=",
            thresholds["max_errors"],
        )
        require_summary_threshold_at_most(
            policy,
            report,
            label,
            "errors",
            thresholds["max_errors"],
        )
        require_gate_check_threshold(
            policy,
            report,
            label,
            "scored",
            ">=",
            thresholds["min_scored"],
        )
        require_summary_threshold_at_least(
            policy,
            report,
            label,
            "scored",
            thresholds["min_scored"],
        )


def require_summary_threshold_at_least(
    policy: str,
    report: dict[str, Any],
    label: str,
    metric: str,
    minimum: float | int,
) -> None:
    summary = report.get("summary")
    if not isinstance(summary, dict):
        raise ValueError(f"{policy} {label} report does not contain a summary")
    actual = summary.get(metric)
    if metric in BOUNDED_SCORE_POLICY_METRICS and is_number(actual) and actual > 1:
        raise ValueError(
            f"{policy} {label} report summary {metric} must not be above one"
        )
    if not is_number(actual) or actual < minimum:
        raise ValueError(
            f"{policy} {label} report summary {metric} must be at least {minimum}"
        )


def require_summary_threshold_at_most(
    policy: str,
    report: dict[str, Any],
    label: str,
    metric: str,
    maximum: float | int,
) -> None:
    summary = report.get("summary")
    if not isinstance(summary, dict):
        raise ValueError(f"{policy} {label} report does not contain a summary")
    actual = summary.get(metric)
    if metric in COUNT_POLICY_METRICS and is_number(actual) and actual < 0:
        raise ValueError(
            f"{policy} {label} report summary {metric} must not be negative"
        )
    if not is_number(actual) or actual > maximum:
        raise ValueError(
            f"{policy} {label} report summary {metric} must be at most {maximum}"
        )


def require_gate_check_threshold(
    policy: str,
    report: dict[str, Any],
    label: str,
    metric: str,
    op: str,
    policy_threshold: float | int,
) -> None:
    gate = report.get("gate")
    checks = gate.get("checks") if isinstance(gate, dict) else None
    if not isinstance(checks, list):
        raise ValueError(f"{policy} {label} report does not contain gate checks")
    for check in checks:
        if not isinstance(check, dict):
            continue
        if check.get("metric") != metric or check.get("op") != op:
            continue
        threshold = check.get("threshold")
        if not is_number(threshold):
            continue
        if metric in COUNT_POLICY_METRICS and threshold < 0:
            raise ValueError(
                f"{policy} {label} report gate check threshold must not be negative: "
                f"{metric}"
            )
        if metric in BOUNDED_SCORE_POLICY_METRICS and threshold > 1:
            raise ValueError(
                f"{policy} {label} report gate check threshold must not be above one: "
                f"{metric}"
            )
        if (op == ">=" and threshold >= policy_threshold) or (
            op == "<=" and threshold <= policy_threshold
        ):
            if check.get("passed") is not True:
                raise ValueError(
                    f"{policy} {label} report gate check did not pass: "
                    f"{metric} {op} {policy_threshold}"
                )
            actual = check.get("actual")
            if metric in COUNT_POLICY_METRICS and is_number(actual) and actual < 0:
                raise ValueError(
                    f"{policy} {label} report gate check actual must not be negative: "
                    f"{metric}"
                )
            if metric in BOUNDED_SCORE_POLICY_METRICS and is_number(actual) and actual > 1:
                raise ValueError(
                    f"{policy} {label} report gate check actual must not be above one: "
                    f"{metric}"
                )
            if not is_number(actual) or not (
                (op == ">=" and actual >= policy_threshold)
                or (op == "<=" and actual <= policy_threshold)
            ):
                raise ValueError(
                    f"{policy} {label} report gate check actual failed policy threshold: "
                    f"{metric} {op} {policy_threshold}"
                )
            return
    raise ValueError(
        f"{policy} {label} report gate must include {metric} {op} {policy_threshold}"
    )


def parse_manifest_header(line: str) -> list[str]:
    if line.startswith("#"):
        line = line[1:]
        if line.startswith(" "):
            line = line[1:]
    return line.split("\t")


def read_corpus_manifest(path: Path) -> tuple[list[str], list[list[str]]]:
    header: list[str] | None = None
    rows: list[list[str]] = []
    seen_paths: set[str] = set()
    for line in path.read_text(encoding="utf-8").splitlines():
        trimmed = line.strip()
        if not trimmed:
            continue
        if header is None:
            header = parse_manifest_header(line)
            if not header or header[0] != "path":
                raise ValueError(f"{path} does not start with a TSV path header")
            seen_columns: set[str] = set()
            for column in header:
                if not column:
                    raise ValueError(f"{path} has empty TSV column")
                if column != column.strip():
                    raise ValueError(f"{path} has whitespace-padded TSV column: {column}")
                if not column.isascii() or not column.isidentifier():
                    raise ValueError(f"{path} has non-canonical TSV column: {column}")
                if column in seen_columns:
                    raise ValueError(f"{path} has duplicate TSV column: {column}")
                seen_columns.add(column)
            if "warnings" not in seen_columns:
                raise ValueError(f"{path} missing required TSV column: warnings")
            if not any(column not in {"path", "warnings"} for column in seen_columns):
                raise ValueError(f"{path} missing TSV count columns")
            continue
        if parse_manifest_header(trimmed) == header:
            raise ValueError(f"{path} has repeated TSV header row")
        if trimmed.startswith("#"):
            continue
        if trimmed.startswith("path\t"):
            raise ValueError(f"{path} has repeated TSV header row")
        cols = line.split("\t")
        if len(cols) != len(header):
            raise ValueError(
                f"{path} row has {len(cols)} columns, expected {len(header)}: {line}"
            )
        document_path = cols[0]
        if (
            not document_path
            or document_path.startswith(("/", "\\"))
            or "\\" in document_path
            or ":" in document_path
            or any(part in {"", ".", ".."} for part in document_path.split("/"))
        ):
            raise ValueError(f"{path} has unsafe document path: {document_path}")
        if document_path != document_path.strip():
            raise ValueError(f"{path} has whitespace-padded document path: {document_path}")
        if not document_path.isascii() or any(char.isspace() for char in document_path):
            raise ValueError(f"{path} has unsafe document path: {document_path}")
        if document_path in seen_paths:
            raise ValueError(f"{path} has duplicate document path: {document_path}")
        seen_paths.add(document_path)
        rows.append(cols)

    if header is None:
        raise ValueError(f"{path} is empty")
    if not rows:
        raise ValueError(f"{path} does not contain document rows")
    return header, rows


def corpus_manifest_document_paths(path: Path) -> list[str]:
    _, rows = read_corpus_manifest(path)
    return [row[0] for row in rows]


def corpus_manifest_summary(path: Path) -> dict[str, Any]:
    header, rows = read_corpus_manifest(path)

    numeric_totals: dict[str, int] = {}
    warning_counts: dict[str, int] = {}
    for index, name in enumerate(header):
        if name in {"path", "warnings"}:
            continue
        total = 0
        for row in rows:
            if row[index] != row[index].strip():
                raise ValueError(
                    f"{path} row has whitespace-padded numeric value for {name}: {row[index]}"
                )
            try:
                value = int(row[index])
            except ValueError:
                raise ValueError(
                    f"{path} row has non-numeric value for {name}: {row[index]}"
                )
            if value >= 0 and str(value) != row[index]:
                raise ValueError(
                    f"{path} row has non-canonical numeric value for {name}: {row[index]}"
                )
            if value < 0:
                raise ValueError(
                    f"{path} row has negative numeric value for {name}: {row[index]}"
                )
            total += value
        numeric_totals[name] = total

    if "warnings" in header:
        warning_index = header.index("warnings")
        for row in rows:
            warnings = row[warning_index]
            if warnings == "-":
                continue
            row_warnings: set[str] = set()
            for warning in warnings.split("|"):
                if not warning.strip():
                    raise ValueError(f"{path} row has empty warning token")
                if warning != warning.strip():
                    raise ValueError(
                        f"{path} row has whitespace-padded warning token: {warning}"
                    )
                if warning == "-":
                    raise ValueError(f"{path} row has invalid warning token: -")
                if not warning.isascii() or not warning.isidentifier():
                    raise ValueError(
                        f"{path} row has non-canonical warning token: {warning}"
                    )
                if warning not in KNOWN_WARNING_TOKENS:
                    raise ValueError(f"{path} row has unknown warning token: {warning}")
                if warning in row_warnings:
                    raise ValueError(
                        f"{path} row has duplicate warning token: {warning}"
                    )
                row_warnings.add(warning)
                warning_counts[warning] = warning_counts.get(warning, 0) + 1

    return {
        "documents": len(rows),
        "numeric_totals": numeric_totals,
        "warning_counts": dict(sorted(warning_counts.items())),
    }


def corpus_manifest_summaries(paths: list[Path] | None) -> list[dict[str, Any]]:
    return [
        {"path": path.as_posix(), "summary": corpus_manifest_summary(path)}
        for path in sorted(paths or [], key=path_sort_key)
    ]


def release_evidence_summary(
    name: str | None,
    *,
    enforce_policy_inputs: bool,
    hygiene_report: Path | None,
    validation_report: Path | None,
    benchmark_reports: list[Path] | None,
    corpus_manifests: list[Path] | None,
) -> dict[str, Any] | None:
    if name is None:
        return None
    if name not in RELEASE_POLICIES:
        raise ValueError(f"unknown release policy: {name}")

    strict_missing: list[str] = []
    if name == "public-release":
        strict_missing = public_release_policy_input_gaps(
            hygiene_report=hygiene_report,
            validation_report=validation_report,
            benchmark_reports=benchmark_reports,
            corpus_manifests=corpus_manifests,
        )
    strict_inputs_complete = not strict_missing
    if enforce_policy_inputs and strict_inputs_complete:
        strict_status = "enforced"
    elif strict_inputs_complete:
        strict_status = "inputs_complete_not_enforced"
    else:
        strict_status = "missing_inputs"

    return {
        "policy": name,
        "strict_policy_status": strict_status,
        "strict_policy_enforced": enforce_policy_inputs,
        "strict_policy_inputs_complete": strict_inputs_complete,
        "strict_missing": strict_missing,
        "provided": {
            "hygiene_report": hygiene_report.as_posix() if hygiene_report else None,
            "validation_report": validation_report.as_posix() if validation_report else None,
            "benchmark_reports": [
                path.as_posix() for path in sorted(benchmark_reports or [], key=path_sort_key)
            ],
            "corpus_manifests": [
                path.as_posix() for path in sorted(corpus_manifests or [], key=path_sort_key)
            ],
        },
    }


def release_manifest(
    artifacts: list[Path],
    *,
    hygiene_report: Path | None = None,
    validation_report: Path | None = None,
    benchmark_reports: list[Path] | None = None,
    corpus_manifests: list[Path] | None = None,
    release_policy: str | None = None,
    enforce_policy_inputs: bool = False,
    version: str | None = None,
    git_rev: str | None = None,
) -> dict[str, Any]:
    if not artifacts:
        raise ValueError("at least one artifact path is required")
    if enforce_policy_inputs and release_policy is None:
        raise ValueError("enforce_policy_inputs requires release policy")
    if enforce_policy_inputs:
        check_required_policy_inputs(
            release_policy,
            hygiene_report=hygiene_report,
            validation_report=validation_report,
            benchmark_reports=benchmark_reports,
            corpus_manifests=corpus_manifests,
        )
    resolved = [path if isinstance(path, Path) else Path(path) for path in artifacts]
    missing = [path.as_posix() for path in resolved if not path.is_file()]
    if missing:
        raise FileNotFoundError("missing artifact(s): " + ", ".join(missing))
    require_unique_paths("artifact", resolved)
    require_unique_paths("benchmark report", benchmark_reports)
    require_unique_paths("corpus manifest", corpus_manifests)
    for label, value in (("version", version), ("git_rev", git_rev)):
        if value is not None and not isinstance(value, str):
            raise ValueError(f"{label} must be a string")
        if value is not None and not value.strip():
            raise ValueError(f"{label} must not be empty")
        if value is not None and value != value.strip():
            raise ValueError(f"{label} must not have surrounding whitespace")
        if value is not None and any(char.isspace() for char in value):
            raise ValueError(f"{label} must not contain whitespace")

    manifest: dict[str, Any] = {
        "schema": SCHEMA,
        "artifacts": [artifact_record(path) for path in sorted(resolved, key=path_sort_key)],
    }
    if version is not None:
        manifest["version"] = version
    if git_rev is not None:
        manifest["git_rev"] = git_rev
    policy = release_policy_summary(release_policy)
    if policy is not None:
        manifest["release_policy"] = policy
        manifest["release_evidence"] = release_evidence_summary(
            release_policy,
            enforce_policy_inputs=enforce_policy_inputs,
            hygiene_report=hygiene_report,
            validation_report=validation_report,
            benchmark_reports=benchmark_reports,
            corpus_manifests=corpus_manifests,
        )
    hygiene = hygiene_summary(hygiene_report)
    if hygiene is not None:
        if enforce_policy_inputs and release_policy is not None:
            require_report_gate_passed(release_policy, hygiene, "hygiene")
        manifest["hygiene"] = hygiene
    validation = validation_summary(validation_report)
    if validation is not None:
        if enforce_policy_inputs and release_policy is not None:
            require_report_gate_passed(release_policy, validation, "validation")
            require_public_release_report_thresholds(
                release_policy, validation, "validation"
            )
        manifest["validation"] = validation
    if enforce_policy_inputs and release_policy is not None:
        for benchmark_report in benchmark_reports or []:
            require_public_release_benchmark_identity(release_policy, benchmark_report)
    benchmarks = benchmark_summaries(benchmark_reports)
    if benchmarks:
        if enforce_policy_inputs and release_policy is not None:
            for benchmark in benchmarks:
                require_report_gate_passed(release_policy, benchmark, "benchmark")
                require_public_release_report_thresholds(
                    release_policy, benchmark, "benchmark"
                )
        manifest["benchmarks"] = benchmarks
    if (
        enforce_policy_inputs
        and release_policy == "public-release"
        and validation_report is not None
        and corpus_manifests is not None
    ):
        require_public_release_validation_report_coverage(
            release_policy,
            validation_report,
            corpus_manifests,
        )
    corpus = corpus_manifest_summaries(corpus_manifests)
    if corpus:
        manifest["corpus_manifests"] = corpus
    return manifest


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifacts", nargs="+", type=Path, help="release artifact files")
    parser.add_argument("--version", help="release version string")
    parser.add_argument("--git-rev", help="git revision included in the release")
    parser.add_argument(
        "--release-policy",
        choices=sorted(RELEASE_POLICIES),
        help="embed the named release validation policy in the manifest",
    )
    parser.add_argument(
        "--hygiene-report",
        type=Path,
        help=(
            "optional JSON report from scripts/public_hygiene_audit.py --json; "
            "only path and compact gate metadata are embedded"
        ),
    )
    parser.add_argument(
        "--validation-report",
        type=Path,
        help="optional JSON validation report; only its summary is embedded",
    )
    parser.add_argument(
        "--benchmark-report",
        action="append",
        type=Path,
        help="optional JSON benchmark report; may be repeated; only summaries are embedded",
    )
    parser.add_argument(
        "--corpus-manifest",
        action="append",
        type=Path,
        help="optional public corpus TSV manifest; may be repeated; only summaries are embedded",
    )
    parser.add_argument(
        "--enforce-policy-inputs",
        action="store_true",
        help=(
            "when --release-policy is set, require that policy's local evidence "
            "reports/manifests and reject hygiene, validation, or benchmark reports whose gates fail; "
            "public-release requires exactly MANIFEST.tsv and RENDER_MANIFEST.tsv corpus manifests"
        ),
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="write manifest JSON to this path instead of stdout",
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    try:
        manifest = release_manifest(
            args.artifacts,
            hygiene_report=args.hygiene_report,
            validation_report=args.validation_report,
            benchmark_reports=args.benchmark_report,
            corpus_manifests=args.corpus_manifest,
            release_policy=args.release_policy,
            enforce_policy_inputs=args.enforce_policy_inputs,
            version=args.version,
            git_rev=args.git_rev,
        )
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"release_manifest: {error}", file=sys.stderr)
        return 2

    try:
        payload = (
            json.dumps(
                manifest,
                ensure_ascii=False,
                indent=2,
                sort_keys=True,
                allow_nan=False,
            )
            + "\n"
        )
    except ValueError as error:
        print(f"release_manifest: {error}", file=sys.stderr)
        return 2
    if args.output is None:
        sys.stdout.write(payload)
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(payload, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
