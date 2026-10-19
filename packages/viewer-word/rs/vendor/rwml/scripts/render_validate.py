#!/usr/bin/env python3
"""Multi-metric validation of rwml's PDF renderer against LibreOffice.

For each input `.doc`/`.docx`, render it two ways and compare:

  * rwml        — `cargo run --features render --example to_pdf -- IN OUT`
  * LibreOffice — `soffice --headless --convert-to pdf` (the reference oracle)

and report complementary metrics per document:

  * text recall   — fraction of the reference's whitespace-normalized tokens that
                    also appear in rwml's text layer, after dropping volatile
                    LibreOffice-only field text such as local file paths and
                    missing-reference placeholders, plus known fallback shape
                    placeholders and joined tracked-change/footnote markers
                    when rwml's report proves that context.
  * page ratio    — rwml page count / reference page count (≈ 1.0 is good).
  * legacy aHash  — average-hash Hamming similarity of page 1 at 72 DPI
                    (0..1; retained unchanged for historical trend continuity).
  * page aHash    — mean average-hash similarity across every matched page up to
                    a configurable hard cap at a reported fixed DPI.
  * ink IoU       — foreground-pixel intersection-over-union across those pages;
                    canvases are white-padded, never stretched, before comparison.
  * warnings      — rwml `RenderReport` warning count/kinds for trend tracking.

This is a developer tool, not part of the crate. It needs PyMuPDF (`pip install
pymupdf`), Pillow, and either a local `soffice` or the `lo-cli` Docker image.
By default, `--soffice auto` prefers local `soffice` when present and falls back
to Docker.

  python scripts/render_validate.py corpus/public/**/*.docx
  python scripts/render_validate.py --manifest corpus/public/RENDER_MANIFEST.tsv
  python scripts/render_validate.py --soffice docker corpus/*.doc
  python scripts/render_validate.py --json corpus/public/**/*.docx > render-report.json
  python scripts/render_validate.py --json --manifest corpus/public/RENDER_MANIFEST.tsv > render-report.json
  python scripts/render_validate.py --json --min-mean-recall 0.90 --max-skipped 0 corpus/public/**/*.docx > render-report.json
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import json
import math
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import asdict, dataclass
from pathlib import Path

with contextlib.redirect_stdout(sys.stderr):
    try:
        import pymupdf as fitz
    except ImportError:
        try:
            import fitz  # type: ignore[no-redef]  # Legacy PyMuPDF.
        except ImportError:
            fitz = None
try:
    from PIL import Image, ImageChops
except ImportError:
    Image = None
    ImageChops = None


DEFAULT_RASTER_DPI = 110
DEFAULT_PAGE_CAP = 32
DEFAULT_FOREGROUND_THRESHOLD = 245
DEFAULT_AHASH_SIZE = 16
DEFAULT_FONT_MODE = "fixed-noto-subsets"
MAX_RASTER_DPI = 600
MAX_PAGE_CAP = 256
MAX_AHASH_SIZE = 64
MAX_RASTER_PAGE_PIXELS = 40_000_000
MAX_NORMALIZED_CANVAS_PIXELS = 50_000_000
MAX_BUFFERED_RASTER_PIXELS = 100_000_000
MAX_VOLATILE_REFERENCE_PATH_TOKENS = 8
OFFICE_DOCUMENT_EXTENSIONS = (".doc", ".docx", ".docm", ".dot", ".dotx", ".rtf")

COUNT_THRESHOLD_METRICS = {
    "below_recall_min",
    "skipped",
    "unmatched_candidate_pages",
    "unmatched_reference_pages",
}
SCORE_THRESHOLD_METRICS = {
    "mean_recall",
    "mean_page_ratio",
    "mean_ahash_similarity",
    "mean_page_ahash_similarity",
    "mean_foreground_ink_iou",
}
BOUNDED_SCORE_THRESHOLD_METRICS = {
    "mean_recall",
    "mean_ahash_similarity",
    "mean_page_ahash_similarity",
    "mean_foreground_ink_iou",
}
VALID_RENDER_WARNING_KINDS = {
    "UnsupportedFieldEvaluation",
    "FloatingShapePlaceholderOnly",
    "ChartsPreservedButNotModeled",
    "OleObjectsPreservedButNotModeled",
    "UnsupportedMetafileImages",
    "MissingImageBytes",
    "UndecodableRasterImages",
}
UNSUPPORTED_OBJECT_WARNING_KINDS = {
    "FloatingShapePlaceholderOnly",
    "ChartsPreservedButNotModeled",
    "OleObjectsPreservedButNotModeled",
    "UnsupportedMetafileImages",
}


class RenderDependencyError(RuntimeError):
    """A selected render backend executable is unavailable."""


class VisualMetricError(RuntimeError):
    """A PDF page could not be rasterized or compared deterministically."""


@dataclass
class ValidationRow:
    document: str
    status: str
    recall: float | None = None
    rwml_pages: int | None = None
    reference_pages: int | None = None
    page_ratio: float | None = None
    ahash_similarity: float | None = None
    mean_page_ahash_similarity: float | None = None
    foreground_ink_iou: float | None = None
    compared_pages: int | None = None
    unmatched_candidate_pages: int | None = None
    unmatched_reference_pages: int | None = None
    capped_matched_pages: int | None = None
    render_warnings: int | None = None
    render_warning_kinds: list[str] | None = None
    reason: str | None = None


@dataclass(frozen=True)
class VisualMetrics:
    mean_page_ahash_similarity: float | None
    foreground_ink_iou: float | None
    compared_pages: int
    unmatched_candidate_pages: int
    unmatched_reference_pages: int
    capped_matched_pages: int


def is_finite_number(value: object) -> bool:
    return (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(value)
    )


def require_pdf_deps() -> None:
    missing = []
    if fitz is None:
        missing.append("PyMuPDF (pip install pymupdf)")
    if Image is None or ImageChops is None:
        missing.append("Pillow (pip install pillow)")
    if missing:
        sys.exit("PDF validation dependencies required: " + ", ".join(missing))


def resolve_soffice_mode(mode: str) -> str:
    if mode != "auto":
        return mode
    if shutil.which("soffice") is not None:
        return "local"
    if shutil.which("docker") is not None:
        return "docker"
    raise RenderDependencyError(
        "LibreOffice validation dependency required: neither soffice nor docker "
        "executable found; install LibreOffice or Docker"
    )


def mean(values: list[float]) -> float | None:
    if not values:
        return None
    return round(sum(values) / len(values), 4)


def row_dict(row: ValidationRow) -> dict:
    return {k: v for k, v in asdict(row).items() if v is not None}


def validate_visual_settings(settings: dict | None = None) -> dict[str, int | str]:
    defaults = {
        "dpi": DEFAULT_RASTER_DPI,
        "page_cap": DEFAULT_PAGE_CAP,
        "foreground_threshold": DEFAULT_FOREGROUND_THRESHOLD,
        "ahash_size": DEFAULT_AHASH_SIZE,
        "font_mode": DEFAULT_FONT_MODE,
    }
    if settings is None:
        return defaults
    if not isinstance(settings, dict):
        raise ValueError("visual settings must be an object")
    unknown = set(settings) - set(defaults)
    if unknown:
        raise ValueError(f"unknown visual setting: {sorted(unknown)[0]}")
    values = defaults | settings
    for name in ("dpi", "page_cap", "foreground_threshold", "ahash_size"):
        value = values[name]
        if not isinstance(value, int) or isinstance(value, bool):
            raise ValueError(f"visual setting is invalid: {name}")
    if values["font_mode"] not in {"fixed-noto-subsets", "system"}:
        raise ValueError(
            f"visual setting is out of range: font_mode={values['font_mode']}"
        )
    if not 1 <= values["dpi"] <= MAX_RASTER_DPI:
        raise ValueError(f"visual setting is out of range: dpi={values['dpi']}")
    if not 1 <= values["page_cap"] <= MAX_PAGE_CAP:
        raise ValueError(
            f"visual setting is out of range: page_cap={values['page_cap']}"
        )
    if not 0 <= values["foreground_threshold"] <= 255:
        raise ValueError(
            "visual setting is out of range: "
            f"foreground_threshold={values['foreground_threshold']}"
        )
    if not 1 <= values["ahash_size"] <= MAX_AHASH_SIZE:
        raise ValueError(
            f"visual setting is out of range: ahash_size={values['ahash_size']}"
        )
    return values


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
    if op == ">=" and metric in SCORE_THRESHOLD_METRICS and threshold < 0:
        raise ValueError(f"negative score threshold for {metric}: {threshold}")
    if metric in BOUNDED_SCORE_THRESHOLD_METRICS and threshold > 1:
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


def validation_gate(summary: dict, thresholds: dict | None = None) -> dict:
    thresholds = thresholds or {}
    checks = []
    add_threshold_check(
        checks,
        "below_recall_min",
        summary.get("below_recall_min"),
        "<=",
        0,
    )
    add_threshold_check(
        checks,
        "mean_recall",
        summary.get("mean_recall"),
        ">=",
        thresholds.get("min_mean_recall"),
    )
    add_threshold_check(
        checks,
        "mean_page_ratio",
        summary.get("mean_page_ratio"),
        ">=",
        thresholds.get("min_mean_page_ratio"),
    )
    add_threshold_check(
        checks,
        "mean_page_ratio",
        summary.get("mean_page_ratio"),
        "<=",
        thresholds.get("max_mean_page_ratio"),
    )
    add_threshold_check(
        checks,
        "mean_ahash_similarity",
        summary.get("mean_ahash_similarity"),
        ">=",
        thresholds.get("min_mean_ahash_similarity"),
    )
    add_threshold_check(
        checks,
        "mean_page_ahash_similarity",
        summary.get("mean_page_ahash_similarity"),
        ">=",
        thresholds.get("min_mean_page_ahash_similarity"),
    )
    add_threshold_check(
        checks,
        "mean_foreground_ink_iou",
        summary.get("mean_foreground_ink_iou"),
        ">=",
        thresholds.get("min_mean_foreground_ink_iou"),
    )
    add_threshold_check(
        checks,
        "mean_render_warnings",
        summary.get("mean_render_warnings"),
        "<=",
        thresholds.get("max_mean_render_warnings"),
    )
    add_threshold_check(
        checks,
        "skipped",
        summary.get("skipped"),
        "<=",
        thresholds.get("max_skipped"),
    )
    add_threshold_check(
        checks,
        "unmatched_candidate_pages",
        summary.get("unmatched_candidate_pages"),
        "<=",
        thresholds.get("max_unmatched_candidate_pages"),
    )
    add_threshold_check(
        checks,
        "unmatched_reference_pages",
        summary.get("unmatched_reference_pages"),
        "<=",
        thresholds.get("max_unmatched_reference_pages"),
    )
    return {"passed": all(check["passed"] for check in checks), "checks": checks}


def reference_page_digests(pdf: Path, *, dpi: int, page_cap: int) -> list[str] | None:
    """Per-page raster digests of a rendered reference, for a stability probe."""
    try:
        images, _ = rasterize_pdf_pages(pdf, dpi=dpi, page_cap=page_cap)
    except Exception:
        return None
    digests = []
    for image in images:
        try:
            digests.append(hashlib.sha256(image.tobytes()).hexdigest())
        except Exception:
            return None
    return digests


def oracle_stability_verdict(
    first: list[str] | None, second: list[str] | None
) -> bool | None:
    """Whether two renders of the same reference document match.

    `None` when there is nothing to compare, so an unavailable probe is never
    read as a failure.
    """
    if not first or not second:
        return None
    return list(first) == list(second)


def validation_report(
    rows: list[ValidationRow],
    recall_min: float,
    thresholds: dict | None = None,
    visual_settings: dict | None = None,
    reference_stable: bool | None = None,
    unstable_references: list[str] | None = None,
) -> dict:
    for row in rows:
        if not isinstance(row.document, str):
            raise ValueError("document must be a string")
        if not row.document.strip():
            raise ValueError("document must not be empty")
        if row.document != row.document.strip():
            raise ValueError(
                f"document must not have surrounding whitespace: {row.document}"
            )
        if "/" in row.document or "\\" in row.document:
            raise ValueError(f"document path is invalid: {row.document}")
        if row.status not in {"pass", "fail", "skip"}:
            raise ValueError(f"status is invalid: {row.status}")
        for metric in (
            "recall",
            "page_ratio",
            "ahash_similarity",
            "mean_page_ahash_similarity",
            "foreground_ink_iou",
            "render_warnings",
        ):
            value = getattr(row, metric)
            if value is not None and not is_finite_number(value):
                raise ValueError(f"metric is invalid: {metric}")
            if value is not None and metric in {
                "recall",
                "ahash_similarity",
                "mean_page_ahash_similarity",
                "foreground_ink_iou",
            }:
                if not 0 <= value <= 1:
                    raise ValueError(f"metric is out of range: {metric}")
        for metric in (
            "rwml_pages",
            "reference_pages",
            "render_warnings",
            "compared_pages",
            "unmatched_candidate_pages",
            "unmatched_reference_pages",
            "capped_matched_pages",
        ):
            value = getattr(row, metric)
            if value is not None and (
                not isinstance(value, int) or isinstance(value, bool) or value < 0
            ):
                raise ValueError(f"count is invalid: {metric}")
        if row.render_warning_kinds is not None:
            if not isinstance(row.render_warning_kinds, list):
                raise ValueError("render warning kinds must be a list")
            if (
                row.render_warnings is not None
                and row.render_warnings != len(row.render_warning_kinds)
            ):
                raise ValueError("render warning count mismatch")
            row_warnings = set()
            for warning in row.render_warning_kinds:
                if (
                    not isinstance(warning, str)
                    or not warning
                    or warning != warning.strip()
                    or not warning.isascii()
                    or not warning.isidentifier()
                ):
                    raise ValueError(f"render warning kind is invalid: {warning}")
                if warning not in VALID_RENDER_WARNING_KINDS:
                    raise ValueError(f"unknown render warning kind: {warning}")
                if warning in row_warnings:
                    raise ValueError(f"duplicate render warning kind: {warning}")
                row_warnings.add(warning)
        if row.status == "skip" and any(
            getattr(row, metric) is not None
            for metric in (
                "recall",
                "rwml_pages",
                "reference_pages",
                "page_ratio",
                "ahash_similarity",
                "mean_page_ahash_similarity",
                "foreground_ink_iou",
                "compared_pages",
                "unmatched_candidate_pages",
                "unmatched_reference_pages",
                "capped_matched_pages",
                "render_warnings",
                "render_warning_kinds",
            )
        ):
            raise ValueError("skipped row has metrics")
        if row.status != "skip" and row.recall is None:
            raise ValueError("non-skip row is missing recall")
    if not is_finite_number(recall_min):
        raise ValueError(f"non-finite recall threshold: {recall_min}")
    if recall_min < 0:
        raise ValueError(f"negative recall threshold: {recall_min}")
    if recall_min > 1:
        raise ValueError(f"recall threshold above one: {recall_min}")
    visual_settings = validate_visual_settings(visual_settings)
    measured = [r for r in rows if r.recall is not None]
    summary = {
        "documents": len(rows),
        "measured": len(measured),
        "skipped": sum(1 for r in rows if r.status == "skip"),
        "below_recall_min": sum(
            1 for r in measured if r.recall is not None and r.recall < recall_min
        ),
        "recall_min": recall_min,
        # Whether the reference renderer reproduced itself. When false, the
        # visual metrics below are not comparable across runs; text recall and
        # the page-count ratio still are.
        "reference_stable": reference_stable,
        "unstable_references": sorted(unstable_references or []),
        "mean_recall": mean([r.recall for r in measured if r.recall is not None]),
        "mean_page_ratio": mean(
            [r.page_ratio for r in measured if r.page_ratio is not None]
        ),
        "mean_ahash_similarity": mean(
            [
                r.ahash_similarity
                for r in measured
                if r.ahash_similarity is not None
            ]
        ),
        "mean_page_ahash_similarity": mean(
            [
                r.mean_page_ahash_similarity
                for r in measured
                if r.mean_page_ahash_similarity is not None
            ]
        ),
        "mean_foreground_ink_iou": mean(
            [
                r.foreground_ink_iou
                for r in measured
                if r.foreground_ink_iou is not None
            ]
        ),
        "compared_pages": sum(
            r.compared_pages for r in measured if r.compared_pages is not None
        ),
        "unmatched_candidate_pages": sum(
            r.unmatched_candidate_pages
            for r in measured
            if r.unmatched_candidate_pages is not None
        ),
        "unmatched_reference_pages": sum(
            r.unmatched_reference_pages
            for r in measured
            if r.unmatched_reference_pages is not None
        ),
        "capped_matched_pages": sum(
            r.capped_matched_pages
            for r in measured
            if r.capped_matched_pages is not None
        ),
        "mean_render_warnings": mean(
            [
                r.render_warnings
                for r in measured
                if r.render_warnings is not None
            ]
        ),
    }
    return {
        "visual_comparison": visual_settings,
        "summary": summary,
        "gate": validation_gate(summary, thresholds),
        "rows": [row_dict(r) for r in rows],
    }


def json_report_payload(report: dict) -> str:
    return json.dumps(report, ensure_ascii=False, indent=2, allow_nan=False)


def warning_kinds(report: dict | None) -> list[str] | None:
    if report is None:
        return None
    warnings = report.get("warnings", [])
    if not isinstance(warnings, list):
        return None
    kinds = []
    for warning in warnings:
        if not isinstance(warning, dict):
            return None
        kind = warning.get("kind")
        if (
            not isinstance(kind, str)
            or not kind
            or kind != kind.strip()
            or not kind.isascii()
            or not kind.isidentifier()
            or kind not in VALID_RENDER_WARNING_KINDS
        ):
            return None
        if kind in kinds:
            return None
        kinds.append(kind)
    return kinds


def render_rwml(
    src: Path,
    out: Path,
    report_out: Path | None = None,
    *,
    fixed_fonts: bool = True,
) -> dict | None:
    """Render via the crate's to_pdf example and return its JSON report."""
    cmd = [
        "cargo",
        "run",
        "--quiet",
        "--features",
        "render",
        "--example",
        "to_pdf",
        "--",
        str(src),
        str(out),
    ]
    if fixed_fonts:
        cmd.append("--fixed-fonts")
    if report_out is not None:
        cmd.extend(["--report-json", str(report_out)])
    r = subprocess.run(
        cmd,
        capture_output=True,
    )
    if not (r.returncode == 0 and out.exists() and out.stat().st_size > 0):
        return None
    if report_out is not None and report_out.exists():
        try:
            return json.loads(report_out.read_text(encoding="utf-8"))
        except json.JSONDecodeError:
            return None
    return {}


def render_libreoffice(src: Path, outdir: Path, mode: str) -> Path | None:
    """Render via LibreOffice (`local` soffice or `docker` lo-cli)."""
    if mode == "docker":
        # Docker Desktop wants forward-slash host paths.
        d = src.parent.resolve().as_posix()
        o = Path(outdir).resolve().as_posix()
        cmd = [
            "docker", "run", "--rm", "-v", f"{d}:/data:ro", "-v", f"{o}:/out",
            "lo-cli", "soffice", "--headless", "--convert-to", "pdf",
            "--outdir", "/out", f"/data/{src.name}",
        ]
    else:
        cmd = [
            "soffice", "--headless", "--convert-to", "pdf",
            "--outdir", str(outdir), str(src),
        ]
    try:
        r = subprocess.run(cmd, capture_output=True)
    except FileNotFoundError as exc:
        if mode == "docker":
            raise RenderDependencyError(
                "LibreOffice validation dependency required: docker executable "
                "not found; install Docker or pass --soffice local"
            ) from exc
        raise RenderDependencyError(
            "LibreOffice validation dependency required: soffice executable "
            "not found; install LibreOffice or pass --soffice docker"
        ) from exc
    out = outdir / (src.stem + ".pdf")
    return out if (r.returncode == 0 and out.exists()) else None


def resolve_input_paths(inputs: list[Path], manifest: Path | None) -> list[Path]:
    if manifest is None:
        return inputs
    if inputs:
        raise ValueError("--manifest cannot be combined with positional inputs")
    return manifest_document_inputs(manifest)


def manifest_document_inputs(manifest: Path) -> list[Path]:
    header = None
    documents = []
    seen = set()
    for line in manifest.read_text(encoding="utf-8").splitlines():
        trimmed = line.strip()
        if not trimmed:
            continue
        if header is None:
            if not line.startswith("#"):
                raise ValueError(f"{manifest} does not start with a TSV path header")
            header = line[1:].lstrip(" ").split("\t")
            if not header or header[0] != "path":
                raise ValueError(f"{manifest} does not start with a TSV path header")
            continue
        if trimmed.startswith("#"):
            continue
        cols = line.split("\t")
        if len(cols) != len(header):
            raise ValueError(f"{manifest} row has {len(cols)} columns: {line}")
        document_path = cols[0]
        if unsafe_manifest_document_path(document_path):
            raise ValueError(f"{manifest} has unsafe document path: {document_path}")
        if document_path in seen:
            raise ValueError(f"{manifest} has duplicate document path: {document_path}")
        seen.add(document_path)
        document = manifest.parent / document_path
        if not document.is_file():
            raise ValueError(f"{manifest} document does not exist: {document_path}")
        documents.append(document)
    if header is None:
        raise ValueError(f"{manifest} is empty")
    if not documents:
        raise ValueError(f"{manifest} does not contain document rows")
    return documents


def unsafe_manifest_document_path(document_path: str) -> bool:
    return (
        not document_path
        or document_path != document_path.strip()
        or document_path.startswith(("/", "\\"))
        or "\\" in document_path
        or ":" in document_path
        or any(part in {"", ".", ".."} for part in document_path.split("/"))
        or any(char.isspace() for char in document_path)
    )


_HEX_STRING = re.compile(rb"<([0-9A-Fa-f\s]*)>")
_CODESPACE = re.compile(
    rb"begincodespacerange(.*?)endcodespacerange", re.S
)
_BFCHAR = re.compile(rb"beginbfchar(.*?)endbfchar", re.S)
_BFRANGE = re.compile(rb"beginbfrange(.*?)endbfrange", re.S)
_ACTUAL_TEXT = re.compile(rb"/ActualText\s*<([0-9A-Fa-f\s]*)>")


def _hex_to_text(raw: bytes) -> str:
    """Decode a CMap destination (UTF-16BE, optionally BOM-prefixed)."""
    digits = b"".join(raw.split())
    if len(digits) % 2:
        return ""
    try:
        data = bytes.fromhex(digits.decode("ascii"))
    except ValueError:
        return ""
    if data[:2] == b"\xfe\xff":
        data = data[2:]
    if len(data) % 2:
        return ""
    try:
        return data.decode("utf-16-be")
    except UnicodeDecodeError:
        return ""


def _hex_to_int(raw: bytes) -> int | None:
    digits = b"".join(raw.split())
    try:
        return int(digits, 16)
    except ValueError:
        return None


def parse_tounicode_cmap(data: bytes) -> dict[str, object]:
    """Read a `ToUnicode` CMap into a code width and a code-to-text map.

    Returns an empty map rather than raising on anything unrecognized: this
    feeds a measurement, so failing to decode must never invent text.
    """
    width = 2
    codespace = _CODESPACE.search(data)
    if codespace:
        first = _HEX_STRING.search(codespace.group(1))
        if first:
            digits = b"".join(first.group(1).split())
            if digits:
                width = max(1, len(digits) // 2)
    mapping: dict[int, str] = {}
    for block in _BFCHAR.findall(data):
        entries = _HEX_STRING.findall(block)
        for index in range(0, len(entries) - 1, 2):
            code = _hex_to_int(entries[index])
            text = _hex_to_text(entries[index + 1])
            if code is not None and text:
                mapping[code] = text
    for block in _BFRANGE.findall(data):
        for low_raw, high_raw, dest in _bfrange_entries(block):
            low = _hex_to_int(low_raw)
            high = _hex_to_int(high_raw)
            if low is None or high is None or high < low or high - low > 0xFFFF:
                continue
            if isinstance(dest, list):
                for offset, item in enumerate(dest):
                    text = _hex_to_text(item)
                    if text:
                        mapping[low + offset] = text
                continue
            base = _hex_to_text(dest)
            if not base:
                continue
            for offset in range(high - low + 1):
                mapping[low + offset] = base[:-1] + chr(ord(base[-1]) + offset)
    return {"width": width, "map": mapping}


def _bfrange_entries(block: bytes):
    """Yield `(low, high, destination)` triples from a `bfrange` block."""
    position = 0
    while True:
        low = _HEX_STRING.search(block, position)
        if not low:
            return
        high = _HEX_STRING.search(block, low.end())
        if not high:
            return
        rest = block[high.end() :]
        array = re.match(rb"\s*\[(.*?)\]", rest, re.S)
        if array:
            yield low.group(1), high.group(1), _HEX_STRING.findall(array.group(1))
            position = high.end() + array.end()
            continue
        dest = _HEX_STRING.search(block, high.end())
        if not dest:
            return
        yield low.group(1), high.group(1), dest.group(1)
        position = dest.end()


_STRING_OPERAND = re.compile(rb"\((?:\\.|[^\\()])*\)|<[0-9A-Fa-f\s]*>", re.S)
_TOKEN = re.compile(
    rb"/[^\s/<>\[\]()]+|<<|>>|\[|\]|\((?:\\.|[^\\()])*\)|<[0-9A-Fa-f\s]*>|[^\s/<>\[\]()]+",
    re.S,
)


def _decode_pdf_string(raw: bytes) -> bytes:
    if raw.startswith(b"<"):
        digits = b"".join(raw[1:-1].split())
        if len(digits) % 2:
            digits += b"0"
        try:
            return bytes.fromhex(digits.decode("ascii"))
        except ValueError:
            return b""
    body = raw[1:-1]
    out = bytearray()
    index = 0
    escapes = {b"n": 10, b"r": 13, b"t": 9, b"b": 8, b"f": 12}
    while index < len(body):
        byte = body[index : index + 1]
        if byte != b"\\":
            out += byte
            index += 1
            continue
        index += 1
        if index >= len(body):
            break
        nxt = body[index : index + 1]
        if nxt in escapes:
            out.append(escapes[nxt])
            index += 1
        elif nxt.isdigit():
            digits = b""
            while index < len(body) and len(digits) < 3 and body[index : index + 1].isdigit():
                digits += body[index : index + 1]
                index += 1
            out.append(int(digits, 8) & 0xFF)
        else:
            out += nxt
            index += 1
    return bytes(out)


def content_stream_text(content: bytes, cmaps: dict) -> str:
    """Reconstruct a page's text the way a conforming reader would.

    Glyph codes are mapped through each font's `ToUnicode`, and any
    `/Span <</ActualText ...>>` marked-content section contributes its declared
    text instead of its glyphs. No separator is invented: word spacing comes
    only from space glyphs the page actually draws.
    """
    pieces: list[str] = []
    font: str | None = None
    span_depth = 0
    pending_actual: str | None = None
    operands: list[bytes] = []
    for match in _TOKEN.finditer(content):
        token = match.group(0)
        if token in (b"Tf",):
            names = [item for item in operands if item.startswith(b"/")]
            font = names[-1][1:].decode("latin-1") if names else font
            operands = []
            continue
        if token == b"BDC":
            joined = b" ".join(operands)
            actual = _ACTUAL_TEXT.search(joined)
            if actual is not None:
                span_depth += 1
                pending_actual = _hex_to_text(actual.group(1))
            operands = []
            continue
        if token in (b"BT", b"ET"):
            # Each text object is a separately positioned run, so it cannot
            # continue a word; shaped clusters always stay inside one object.
            pieces.append(" ")
            operands = []
            continue
        if token == b"EMC":
            if span_depth:
                span_depth -= 1
                if pending_actual:
                    pieces.append(pending_actual)
                pending_actual = None
            operands = []
            continue
        if token in (b"Tj", b"TJ", b"'", b'"'):
            if span_depth == 0:
                entry = cmaps.get(font or "")
                if entry:
                    width = int(entry.get("width", 2)) or 2
                    mapping = entry.get("map", {})
                    for operand in operands:
                        if not _STRING_OPERAND.fullmatch(operand):
                            continue
                        raw = _decode_pdf_string(operand)
                        for index in range(0, len(raw) - width + 1, width):
                            code = int.from_bytes(raw[index : index + width], "big")
                            pieces.append(mapping.get(code, ""))
            operands = []
            continue
        if re.fullmatch(rb"[A-Za-z*'\"]+[01]?", token) and not token.startswith(b"/"):
            operands = []
            continue
        operands.append(token)
    return " ".join("".join(pieces).split())


def _is_rtl_char(ch: str) -> bool:
    code = ord(ch)
    # Hebrew, Arabic, Syriac, Thaana, N'Ko and the Arabic supplements.
    return 0x0590 <= code <= 0x08FF or 0xFB1D <= code <= 0xFDFF or 0xFE70 <= code <= 0xFEFF


def reverse_rtl_runs(text: str) -> str:
    """Put right-to-left runs back into logical order.

    Content streams draw glyphs in visual order. Reversing each maximal
    right-to-left run recovers logical order for runs that carry no embedded
    left-to-right text or digits, which is the case this measurement needs;
    it is a deliberate simplification of the Unicode bidirectional algorithm.
    """
    out: list[str] = []
    run: list[str] = []
    for ch in text:
        if _is_rtl_char(ch):
            run.append(ch)
            continue
        if run:
            out.extend(reversed(run))
            run = []
        out.append(ch)
    out.extend(reversed(run))
    return "".join(out)


def page_tounicode_cmaps(doc, page) -> dict:
    """Map each of a page's font resource names to its decoded `ToUnicode`."""
    cmaps: dict[str, dict] = {}
    try:
        fonts = page.get_fonts(full=True)
    except Exception:
        return cmaps
    for font in fonts:
        try:
            xref = int(font[0])
            resource = str(font[4])
            key = doc.xref_get_key(xref, "ToUnicode")
        except Exception:
            continue
        if not key or key[0] != "xref":
            continue
        try:
            stream = doc.xref_stream(int(str(key[1]).split()[0]))
        except Exception:
            continue
        if stream:
            cmaps[resource] = parse_tounicode_cmap(stream)
    return cmaps


def conforming_tokens(doc) -> list[str]:
    """Tokens a reader that honors `ActualText` recovers from the page text.

    PyMuPDF, like pdfminer.six and pypdf, ignores `ActualText` marked content
    and splits complex-script words apart at every span boundary. Rebuilding the
    text from the content stream recovers what Acrobat or Chrome would copy.
    """
    out: list[str] = []
    for page in doc:
        try:
            content = page.read_contents()
        except Exception:
            continue
        if not content:
            continue
        text = content_stream_text(content, page_tounicode_cmaps(doc, page))
        out.extend(reverse_rtl_runs(text).split())
    return out


def tokens(pdf: Path) -> list[str]:
    require_pdf_deps()
    doc = fitz.open(pdf)
    text = " ".join(p.get_text() for p in doc)
    return text.split()


def candidate_tokens(pdf: Path) -> list[str]:
    """What rwml's PDF yields to any legitimate reader.

    The reference stays whatever a plain reader reports, because it is the
    oracle. For the candidate a token also counts when only an `ActualText`
    aware reader recovers it — that text is genuinely in the PDF, and both
    paths read the file rather than inventing anything.
    """
    require_pdf_deps()
    doc = fitz.open(pdf)
    text = " ".join(p.get_text() for p in doc)
    return text.split() + conforming_tokens(doc)


def reference_recall_tokens(
    raw_tokens: list[str],
    render_warning_kinds: list[str] | None = None,
) -> list[str]:
    tokens = []
    index = 0
    missing_reference = ["Error:", "Reference", "source", "not", "found"]
    while index < len(raw_tokens):
        if raw_tokens[index : index + len(missing_reference)] == missing_reference:
            index += len(missing_reference)
            continue
        path_span = volatile_reference_path_span(raw_tokens, index)
        if path_span:
            index += path_span
            continue
        token = raw_tokens[index]
        if not is_volatile_reference_shape_placeholder_token(
            token, render_warning_kinds
        ):
            tokens.append(token)
        index += 1
    return tokens


def is_volatile_reference_path_token(token: str) -> bool:
    value = normalized_reference_path_token(token)
    if not value:
        return False
    lower = value.lower()
    if is_absolute_reference_path_token(value):
        return True
    if "/" in value and lower.endswith(OFFICE_DOCUMENT_EXTENSIONS):
        return True
    if "\\" in value and lower.endswith(OFFICE_DOCUMENT_EXTENSIONS):
        return True
    return False


def volatile_reference_path_span(raw_tokens: list[str], index: int) -> int:
    value = normalized_reference_path_token(raw_tokens[index])
    if not is_volatile_reference_path_token(value):
        return 0
    if not is_absolute_reference_path_token(value) or is_office_document_token(value):
        return 1

    limit = min(len(raw_tokens), index + MAX_VOLATILE_REFERENCE_PATH_TOKENS)
    for end in range(index + 1, limit):
        continuation = normalized_reference_path_token(raw_tokens[end])
        if not continuation:
            break
        if is_office_document_token(continuation):
            return end - index + 1
        if "/" not in continuation and "\\" not in continuation:
            break
    return 1


def normalized_reference_path_token(token: str) -> str:
    return token.strip(" \t\r\n\"'`.,;:()[]{}<>")


def is_absolute_reference_path_token(value: str) -> bool:
    return value.startswith(("/", "~/", "\\\\")) or (
        len(value) >= 3 and value[1] == ":" and value[2] in {"/", "\\"}
    )


def is_office_document_token(value: str) -> bool:
    return value.lower().endswith(OFFICE_DOCUMENT_EXTENSIONS)


def is_volatile_reference_shape_placeholder_token(
    token: str,
    render_warning_kinds: list[str] | None,
) -> bool:
    if token != "[shape]" or not render_warning_kinds:
        return False
    return any(kind in UNSUPPORTED_OBJECT_WARNING_KINDS for kind in render_warning_kinds)


def token_recall(
    ref_tokens: list[str],
    got_tokens: list[str],
    render_report: dict | None = None,
) -> float:
    if not ref_tokens:
        return 1.0
    got_set = set(got_tokens)
    hit = sum(
        1
        for token in ref_tokens
        if reference_token_recalled(token, got_tokens, got_set, render_report)
    )
    return hit / len(ref_tokens)


def reference_token_recalled(
    token: str,
    got_tokens: list[str],
    got_set: set[str],
    render_report: dict | None,
) -> bool:
    if token in got_set:
        return True
    if report_has_tracked_changes(render_report) and tracked_reference_token_recalled(
        token, got_tokens
    ):
        return True
    if report_unsupported_count(render_report, "footnotes") > 0 and joined_note_marker_recalled(
        token, got_set
    ):
        return True
    if split_rtl_list_marker_recalled(token, got_set):
        return True
    return False


def report_unsupported_count(report: dict | None, key: str) -> int:
    if not isinstance(report, dict):
        return 0
    unsupported = report.get("unsupported")
    if not isinstance(unsupported, dict):
        return 0
    value = unsupported.get(key, 0)
    return value if isinstance(value, int) and not isinstance(value, bool) else 0


def report_has_tracked_changes(report: dict | None) -> bool:
    return any(
        report_unsupported_count(report, key) > 0
        for key in (
            "tracked_insertions",
            "tracked_deletions",
            "tracked_moves",
            "tracked_property_changes",
        )
    )


def tracked_reference_token_recalled(token: str, got_tokens: list[str]) -> bool:
    value = token.strip(" \t\r\n\"'`.,;:()[]{}<>")
    if len(value) < 5:
        return False
    needle = value.lower()
    fragments = []
    for got in got_tokens:
        fragments.extend(visible_token_fragments(got))
    matches = {
        fragment.lower()
        for fragment in fragments
        if len(fragment) >= 4 and fragment.lower() in needle
    }
    if len(matches) >= 2:
        return True
    return any(
        len(fragment) >= 5
        and len(value) > len(fragment)
        and needle.endswith(fragment.lower())
        for fragment in matches
    )


def visible_token_fragments(token: str) -> list[str]:
    value = token.strip(" \t\r\n\"'`.,;:()[]{}<>")
    if not value:
        return []
    return re.findall(r"[A-Z]+(?=[A-Z][a-z]|$)|[A-Z]?[a-z]+|\d+", value)


def joined_note_marker_recalled(token: str, got_set: set[str]) -> bool:
    value = token.strip(" \t\r\n\"'`.,;:()[]{}<>")
    if len(value) < 2:
        return False
    if value[-1].isdigit() and value[:-1] in got_set:
        return True
    return value[0].isdigit() and value[1:] in got_set


def split_rtl_list_marker_recalled(token: str, got_set: set[str]) -> bool:
    """Accept a list period split from an adjacent RTL label word.

    LibreOffice can expose a right-to-left list label as ``.word`` while the
    candidate's ActualText-aware content stream exposes ``word`` and ``.`` as
    separate tokens. Both are the same visible marker/text pair; only accept
    this normalization for RTL words and an explicitly present period.
    """
    value = token.strip(" \t\r\n\"'`(),;:[]{}<>")
    if not value.startswith(".") or len(value) == 1:
        return False
    word = value[1:]
    return "." in got_set and word in got_set and any(_is_rtl_char(ch) for ch in word)


def text_recall(
    ref: Path,
    got: Path,
    render_warning_kinds: list[str] | None = None,
    render_report: dict | None = None,
) -> float:
    ref_tokens = reference_recall_tokens(tokens(ref), render_warning_kinds)
    return token_recall(ref_tokens, candidate_tokens(got), render_report)


def page_count(pdf: Path) -> int:
    require_pdf_deps()
    return fitz.open(pdf).page_count


def opaque_rgb(image):
    if image.mode in {"RGBA", "LA"} or "transparency" in image.info:
        rgba = image.convert("RGBA")
        background = Image.new("RGBA", rgba.size, (255, 255, 255, 255))
        background.alpha_composite(rgba)
        return background.convert("RGB")
    return image.convert("RGB")


def ensure_pixel_budget(width: int, height: int, limit: int, context: str) -> None:
    if width < 1 or height < 1 or width > limit or height > limit or width * height > limit:
        raise VisualMetricError(
            f"{context} exceeds the {limit}-pixel safety limit: {width}x{height}"
        )


def normalize_page_pair(reference, candidate):
    """White-pad both images to one canvas without scaling either page."""
    reference = opaque_rgb(reference)
    candidate = opaque_rgb(candidate)
    width = max(reference.width, candidate.width)
    height = max(reference.height, candidate.height)
    ensure_pixel_budget(width, height, MAX_NORMALIZED_CANVAS_PIXELS, "normalized canvas")
    normalized_reference = Image.new("RGB", (width, height), color="white")
    normalized_candidate = Image.new("RGB", (width, height), color="white")
    normalized_reference.paste(reference, (0, 0))
    normalized_candidate.paste(candidate, (0, 0))
    return normalized_reference, normalized_candidate


def image_ahash(image, size: int = DEFAULT_AHASH_SIZE) -> int:
    if not isinstance(size, int) or isinstance(size, bool) or not 1 <= size <= MAX_AHASH_SIZE:
        raise ValueError(f"aHash size is out of range: {size}")
    grayscale = image.convert("L").resize((size, size))
    pixels = list(grayscale.tobytes())
    average = sum(pixels) / len(pixels)
    bits = 0
    for index, value in enumerate(pixels):
        if value >= average:
            bits |= 1 << index
    return bits


def image_hash_similarity(reference, candidate, size: int = DEFAULT_AHASH_SIZE) -> float:
    reference, candidate = normalize_page_pair(reference, candidate)
    difference = image_ahash(reference, size=size) ^ image_ahash(candidate, size=size)
    return 1.0 - bin(difference).count("1") / (size * size)


def foreground_ink_iou_images(reference, candidate, threshold: int) -> float:
    if (
        not isinstance(threshold, int)
        or isinstance(threshold, bool)
        or not 0 <= threshold <= 255
    ):
        raise ValueError(f"foreground threshold is out of range: {threshold}")
    reference, candidate = normalize_page_pair(reference, candidate)
    ink_lut = [255 if value < threshold else 0 for value in range(256)]
    reference_mask = reference.convert("L").point(ink_lut)
    candidate_mask = candidate.convert("L").point(ink_lut)
    intersection = ImageChops.darker(reference_mask, candidate_mask).histogram()[255]
    union = ImageChops.lighter(reference_mask, candidate_mask).histogram()[255]
    if union == 0:
        return 1.0
    return intersection / union


def compare_page_images(
    reference_pages: list,
    candidate_pages: list,
    *,
    page_cap: int,
    foreground_threshold: int,
    ahash_size: int,
    reference_page_count: int | None = None,
    candidate_page_count: int | None = None,
) -> VisualMetrics:
    settings = validate_visual_settings(
        {
            "page_cap": page_cap,
            "foreground_threshold": foreground_threshold,
            "ahash_size": ahash_size,
        }
    )
    reference_page_count = (
        len(reference_pages) if reference_page_count is None else reference_page_count
    )
    candidate_page_count = (
        len(candidate_pages) if candidate_page_count is None else candidate_page_count
    )
    for name, count, available in (
        ("reference", reference_page_count, len(reference_pages)),
        ("candidate", candidate_page_count, len(candidate_pages)),
    ):
        if (
            not isinstance(count, int)
            or isinstance(count, bool)
            or count < available
        ):
            raise ValueError(f"{name} page count is invalid: {count}")
    compared_pages = min(
        reference_page_count,
        candidate_page_count,
        settings["page_cap"],
        len(reference_pages),
        len(candidate_pages),
    )
    page_hashes = []
    page_ink_ious = []
    for index in range(compared_pages):
        page_hashes.append(
            image_hash_similarity(
                reference_pages[index],
                candidate_pages[index],
                size=settings["ahash_size"],
            )
        )
        page_ink_ious.append(
            foreground_ink_iou_images(
                reference_pages[index],
                candidate_pages[index],
                threshold=settings["foreground_threshold"],
            )
        )
    return visual_metrics_from_scores(
        page_hashes,
        page_ink_ious,
        reference_page_count=reference_page_count,
        candidate_page_count=candidate_page_count,
        page_cap=settings["page_cap"],
    )


def visual_metrics_from_scores(
    page_hashes: list[float],
    page_ink_ious: list[float],
    *,
    reference_page_count: int,
    candidate_page_count: int,
    page_cap: int,
) -> VisualMetrics:
    if len(page_hashes) != len(page_ink_ious):
        raise ValueError("visual page metric count mismatch")
    return VisualMetrics(
        mean_page_ahash_similarity=mean(page_hashes),
        foreground_ink_iou=mean(page_ink_ious),
        compared_pages=len(page_hashes),
        unmatched_candidate_pages=max(0, candidate_page_count - reference_page_count),
        unmatched_reference_pages=max(0, reference_page_count - candidate_page_count),
        capped_matched_pages=max(
            0,
            min(reference_page_count, candidate_page_count) - page_cap,
        ),
    )


def rasterize_pdf_page(document, index: int, *, dpi: int, pdf_name: str):
    page = document[index]
    scale = dpi / 72.0
    predicted_width = max(1, math.ceil(abs(page.rect.width) * scale))
    predicted_height = max(1, math.ceil(abs(page.rect.height) * scale))
    ensure_pixel_budget(
        predicted_width,
        predicted_height,
        MAX_RASTER_PAGE_PIXELS,
        f"raster page {index + 1} of {pdf_name}",
    )
    pixmap = page.get_pixmap(dpi=dpi, alpha=False)
    ensure_pixel_budget(
        pixmap.width,
        pixmap.height,
        MAX_RASTER_PAGE_PIXELS,
        f"raster page {index + 1} of {pdf_name}",
    )
    return Image.frombytes("RGB", (pixmap.width, pixmap.height), pixmap.samples)


def rasterize_pdf_pages(pdf: Path, *, dpi: int, page_cap: int) -> tuple[list, int]:
    if fitz is None or Image is None:
        raise VisualMetricError("PyMuPDF and Pillow are required for page rasterization")
    settings = validate_visual_settings({"dpi": dpi, "page_cap": page_cap})
    try:
        with fitz.open(pdf) as document:
            page_count_value = document.page_count
            pages = []
            buffered_pixels = 0
            for index in range(min(page_count_value, settings["page_cap"])):
                page = rasterize_pdf_page(
                    document,
                    index,
                    dpi=settings["dpi"],
                    pdf_name=pdf.name,
                )
                buffered_pixels += page.width * page.height
                ensure_pixel_budget(
                    buffered_pixels,
                    1,
                    MAX_BUFFERED_RASTER_PIXELS,
                    f"buffered raster pages of {pdf.name}",
                )
                pages.append(page)
            return pages, page_count_value
    except VisualMetricError:
        raise
    except Exception as exc:
        raise VisualMetricError(f"rasterization failed for {pdf.name}: {exc}") from exc


def compare_pdf_visuals(
    reference: Path,
    candidate: Path,
    *,
    dpi: int,
    page_cap: int,
    foreground_threshold: int,
    ahash_size: int,
) -> VisualMetrics:
    if fitz is None or Image is None or ImageChops is None:
        raise VisualMetricError("PyMuPDF and Pillow are required for page rasterization")
    settings = validate_visual_settings(
        {
            "dpi": dpi,
            "page_cap": page_cap,
            "foreground_threshold": foreground_threshold,
            "ahash_size": ahash_size,
        }
    )
    try:
        with fitz.open(reference) as reference_document, fitz.open(
            candidate
        ) as candidate_document:
            reference_page_count = reference_document.page_count
            candidate_page_count = candidate_document.page_count
            compared_pages = min(
                reference_page_count,
                candidate_page_count,
                settings["page_cap"],
            )
            page_hashes = []
            page_ink_ious = []
            for index in range(compared_pages):
                reference_page = rasterize_pdf_page(
                    reference_document,
                    index,
                    dpi=settings["dpi"],
                    pdf_name=reference.name,
                )
                candidate_page = rasterize_pdf_page(
                    candidate_document,
                    index,
                    dpi=settings["dpi"],
                    pdf_name=candidate.name,
                )
                page_hashes.append(
                    image_hash_similarity(
                        reference_page,
                        candidate_page,
                        size=settings["ahash_size"],
                    )
                )
                page_ink_ious.append(
                    foreground_ink_iou_images(
                        reference_page,
                        candidate_page,
                        threshold=settings["foreground_threshold"],
                    )
                )
            return visual_metrics_from_scores(
                page_hashes,
                page_ink_ious,
                reference_page_count=reference_page_count,
                candidate_page_count=candidate_page_count,
                page_cap=settings["page_cap"],
            )
    except VisualMetricError:
        raise
    except Exception as exc:
        raise VisualMetricError(
            f"rasterization failed for {reference.name} / {candidate.name}: {exc}"
        ) from exc


def ahash(pdf: Path, page: int = 0, size: int = 16) -> int:
    require_pdf_deps()
    doc = fitz.open(pdf)
    if page >= doc.page_count:
        return 0
    pix = doc[page].get_pixmap(dpi=72)
    img = Image.frombytes("RGB", (pix.width, pix.height), pix.samples)
    return image_ahash(img, size=size)


def hash_similarity(ref: Path, got: Path, size: int = 16) -> float:
    a, b = ahash(ref, size=size), ahash(got, size=size)
    ham = bin(a ^ b).count("1")
    return 1.0 - ham / (size * size)


def main() -> int:
    # Keep --help ASCII-only so it remains printable on Windows consoles using
    # legacy code pages such as cp949. The module docstring remains the detailed
    # design/reference documentation.
    ap = argparse.ArgumentParser(
        description="Validate rwml PDF output against LibreOffice reference renders."
    )
    ap.add_argument("inputs", nargs="*", type=Path)
    ap.add_argument(
        "--manifest",
        type=Path,
        help="Read input document paths from a public corpus TSV manifest.",
    )
    ap.add_argument(
        "--soffice",
        choices=["auto", "local", "docker"],
        default="auto",
        help="LibreOffice backend; auto prefers local soffice, then Docker lo-cli.",
    )
    ap.add_argument("--recall-min", type=float, default=0.97)
    ap.add_argument("--min-mean-recall", type=float)
    ap.add_argument("--min-mean-page-ratio", type=float)
    ap.add_argument("--max-mean-page-ratio", type=float)
    ap.add_argument("--min-mean-ahash-similarity", type=float)
    ap.add_argument("--min-mean-page-ahash-similarity", type=float)
    ap.add_argument("--min-mean-foreground-ink-iou", type=float)
    ap.add_argument("--max-mean-render-warnings", type=float)
    ap.add_argument("--max-skipped", type=int)
    ap.add_argument("--max-unmatched-candidate-pages", type=int)
    ap.add_argument("--max-unmatched-reference-pages", type=int)
    ap.add_argument(
        "--raster-dpi",
        type=int,
        default=DEFAULT_RASTER_DPI,
        help=f"DPI for multi-page visual metrics (default: {DEFAULT_RASTER_DPI}).",
    )
    ap.add_argument(
        "--page-cap",
        type=int,
        default=DEFAULT_PAGE_CAP,
        help=f"Maximum matched pages rasterized per document (default: {DEFAULT_PAGE_CAP}).",
    )
    ap.add_argument(
        "--foreground-threshold",
        type=int,
        default=DEFAULT_FOREGROUND_THRESHOLD,
        help=(
            "Grayscale values below this are foreground ink "
            f"(default: {DEFAULT_FOREGROUND_THRESHOLD})."
        ),
    )
    ap.add_argument(
        "--ahash-size",
        type=int,
        default=DEFAULT_AHASH_SIZE,
        help=f"Side length for all-page aHash (default: {DEFAULT_AHASH_SIZE}).",
    )
    ap.add_argument(
        "--verify-oracle",
        action="store_true",
        help=(
            "Render every reference document twice and report whether the "
            "reference renderer reproduced itself. Doubles the reference render "
            "cost, so it is off by default."
        ),
    )
    ap.add_argument(
        "--system-fonts",
        action="store_true",
        help="Use host system fonts instead of the deterministic Noto subset set.",
    )
    ap.add_argument(
        "--json",
        action="store_true",
        help="Emit a machine-readable validation report instead of the table.",
    )
    args = ap.parse_args()
    try:
        visual_settings = validate_visual_settings(
            {
                "dpi": args.raster_dpi,
                "page_cap": args.page_cap,
                "foreground_threshold": args.foreground_threshold,
                "ahash_size": args.ahash_size,
                "font_mode": "system" if args.system_fonts else DEFAULT_FONT_MODE,
            }
        )
    except ValueError as exc:
        ap.error(str(exc))
    try:
        inputs = resolve_input_paths(args.inputs, args.manifest)
    except ValueError as exc:
        ap.error(str(exc))
    if not inputs:
        ap.error("the following arguments are required: inputs or --manifest")

    if not args.json:
        print(
            f"{'document':40} {'recall':>8} {'pages':>10} "
            f"{'aHash':>8} {'pageHash':>8} {'inkIoU':>8} {'warn':>5}  result"
        )
        print("-" * 108)
    rows = []
    reference_stable: bool | None = None
    unstable_references: list[str] = []
    try:
        soffice_mode = resolve_soffice_mode(args.soffice)
    except RenderDependencyError as exc:
        sys.exit(str(exc))
    # Temp dir under cwd so Docker Desktop (which can't mount the system temp on
    # Windows) can bind-mount it for the LibreOffice reference render.
    with tempfile.TemporaryDirectory(dir=Path.cwd()) as td:
        tmp = Path(td)
        for src in inputs:
            try:
                ref = render_libreoffice(src, tmp, soffice_mode)
            except RenderDependencyError as exc:
                sys.exit(str(exc))
            if args.verify_oracle and ref is not None:
                # Render the same document a second time. A reference renderer
                # that does not reproduce itself makes the visual metrics
                # incomparable across runs, so every document is checked rather
                # than a sample, which would only give false confidence.
                probe_dir = tmp / "oracle-probe" / src.stem
                probe_dir.mkdir(parents=True, exist_ok=True)
                try:
                    again = render_libreoffice(src, probe_dir, soffice_mode)
                except RenderDependencyError:
                    again = None
                verdict = (
                    oracle_stability_verdict(
                        reference_page_digests(
                            ref, dpi=args.raster_dpi, page_cap=args.page_cap
                        ),
                        reference_page_digests(
                            again, dpi=args.raster_dpi, page_cap=args.page_cap
                        ),
                    )
                    if again is not None
                    else None
                )
                if verdict is False:
                    unstable_references.append(src.name)
                if verdict is not None:
                    reference_stable = (reference_stable is not False) and verdict
            got = tmp / (src.stem + ".rwml.pdf")
            render_report = render_rwml(
                src,
                got,
                tmp / (src.stem + ".rwml.report.json"),
                fixed_fonts=not args.system_fonts,
            )
            if ref is None or render_report is None:
                rows.append(
                    ValidationRow(
                        document=src.name,
                        status="skip",
                        reason="render failed",
                    )
                )
                if not args.json:
                    print(
                        f"{src.name[:40]:40} {'—':>8} {'—':>10} "
                        f"{'—':>8} {'—':>8} {'—':>8} {'—':>5}  SKIP (render failed)"
                    )
                continue
            kinds = warning_kinds(render_report)
            if kinds is None:
                rows.append(
                    ValidationRow(
                        document=src.name,
                        status="skip",
                        reason="render report invalid warnings",
                    )
                )
                if not args.json:
                    print(
                        f"{src.name[:40]:40} {'--':>8} {'--':>10} "
                        f"{'--':>8} {'--':>8} {'--':>8} {'--':>5}  "
                        "SKIP (render report invalid warnings)"
                    )
                continue
            rec = text_recall(ref, got, kinds, render_report)
            got_pages = page_count(got)
            ref_pages = page_count(ref)
            pr = got_pages / max(1, ref_pages)
            sim = hash_similarity(ref, got)
            try:
                visual = compare_pdf_visuals(
                    ref,
                    got,
                    dpi=visual_settings["dpi"],
                    page_cap=visual_settings["page_cap"],
                    foreground_threshold=visual_settings["foreground_threshold"],
                    ahash_size=visual_settings["ahash_size"],
                )
            except VisualMetricError as exc:
                rows.append(
                    ValidationRow(
                        document=src.name,
                        status="skip",
                        reason=str(exc),
                    )
                )
                if not args.json:
                    print(
                        f"{src.name[:40]:40} {'--':>8} {'--':>10} "
                        f"{'--':>8} {'--':>8} {'--':>8} {'--':>5}  "
                        f"SKIP ({exc})"
                    )
                continue
            passed = rec >= args.recall_min
            status = "pass" if passed else "fail"
            rows.append(
                ValidationRow(
                    document=src.name,
                    status=status,
                    recall=round(rec, 4),
                    rwml_pages=got_pages,
                    reference_pages=ref_pages,
                    page_ratio=round(pr, 4),
                    ahash_similarity=round(sim, 4),
                    mean_page_ahash_similarity=visual.mean_page_ahash_similarity,
                    foreground_ink_iou=visual.foreground_ink_iou,
                    compared_pages=visual.compared_pages,
                    unmatched_candidate_pages=visual.unmatched_candidate_pages,
                    unmatched_reference_pages=visual.unmatched_reference_pages,
                    capped_matched_pages=visual.capped_matched_pages,
                    render_warnings=len(kinds) if kinds is not None else None,
                    render_warning_kinds=kinds,
                )
            )
            if not args.json:
                mark = "PASS" if passed else "FAIL"
                warns = len(kinds) if kinds is not None else 0
                print(
                    f"{src.name[:40]:40} {rec:8.3f} "
                    f"{got_pages}/{ref_pages:<7} {sim:8.3f} "
                    f"{(visual.mean_page_ahash_similarity or 0.0):8.3f} "
                    f"{(visual.foreground_ink_iou or 0.0):8.3f} {warns:5}  {mark}"
                )
    thresholds = {
        "min_mean_recall": args.min_mean_recall,
        "min_mean_page_ratio": args.min_mean_page_ratio,
        "max_mean_page_ratio": args.max_mean_page_ratio,
        "min_mean_ahash_similarity": args.min_mean_ahash_similarity,
        "min_mean_page_ahash_similarity": args.min_mean_page_ahash_similarity,
        "min_mean_foreground_ink_iou": args.min_mean_foreground_ink_iou,
        "max_mean_render_warnings": args.max_mean_render_warnings,
        "max_skipped": args.max_skipped,
        "max_unmatched_candidate_pages": args.max_unmatched_candidate_pages,
        "max_unmatched_reference_pages": args.max_unmatched_reference_pages,
    }
    report = validation_report(
        rows,
        args.recall_min,
        thresholds=thresholds,
        visual_settings=visual_settings,
        reference_stable=reference_stable,
        unstable_references=unstable_references,
    )
    if args.json:
        print(json_report_payload(report))
    elif report["summary"]["measured"]:
        mean_warnings = report["summary"]["mean_render_warnings"]
        print("-" * 80)
        print(
            "mean recall "
            f"{report['summary']['mean_recall']:.3f} over "
            f"{report['summary']['measured']} docs, "
            f"{report['summary']['below_recall_min']} below {args.recall_min}; "
            f"mean page ratio {report['summary']['mean_page_ratio']:.3f}; "
            f"legacy aHash {report['summary']['mean_ahash_similarity']:.3f}; "
            "mean page aHash "
            f"{(report['summary']['mean_page_ahash_similarity'] or 0.0):.3f}; "
            "mean ink IoU "
            f"{(report['summary']['mean_foreground_ink_iou'] or 0.0):.3f}; "
            f"mean warnings {(mean_warnings or 0.0):.3f}"
        )
        failures = [check for check in report["gate"]["checks"] if not check["passed"]]
        for check in failures:
            print(
                "threshold failed: "
                f"{check['metric']} {check['op']} {check['threshold']} "
                f"(actual {check['actual']})"
            )
    return 0 if report["gate"]["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
