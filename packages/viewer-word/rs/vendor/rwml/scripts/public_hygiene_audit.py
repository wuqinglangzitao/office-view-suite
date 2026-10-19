#!/usr/bin/env python3
"""Static public-release hygiene audit for the rwml repository.

The audit is intentionally conservative and local: it scans committed plus
currently untracked, non-ignored files for release blockers that should never
reach a public crate snapshot. Office package member paths and textual parts are
scanned too, so `.docx`/`.xlsx`/`.pptx` internal names, metadata,
relationships, OOXML bodies, and embedded Office workbook/package XML are not
treated as opaque binaries. Legacy `.doc` files are not parsed as CFB/OLE2, but
bounded decoded byte views are scanned for high-risk release blockers. It does
not inspect git history; the public history rewrite remains a separate release
step.
"""

from __future__ import annotations

import argparse
import io
import json
import re
import subprocess
import sys
import zipfile
from dataclasses import dataclass
from pathlib import Path


REPO = Path(__file__).resolve().parents[1]

SKIP_DIRS = {
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    "target",
    "__pycache__",
}

BINARY_SUFFIXES = {
    ".bin",
    ".doc",
    ".docx",
    ".emf",
    ".gz",
    ".ico",
    ".jpeg",
    ".jpg",
    ".pdf",
    ".png",
    ".wasm",
    ".wmf",
    ".zip",
}

PROVENANCED_BINARY_FILES = {
    "rwml-fonts/fonts/NotoSansKR-rwml-subset.ttf": "rwml-fonts/PROVENANCE.md",
    "rwml-fonts/fonts/NotoSansKR-rwml-subset-full.ttf": "rwml-fonts/PROVENANCE.md",
    "rwml-fonts/fonts/NotoSansArabic-rwml-subset.ttf": "rwml-fonts/PROVENANCE.md",
    "rwml-fonts/fonts/NotoSansHebrew-rwml-subset.ttf": "rwml-fonts/PROVENANCE.md",
}

SECRET_PATTERNS = [
    ("openai_api_key", re.compile(r"\bsk-(?:proj-)?[A-Za-z0-9_-]{20,}\b")),
    ("github_token", re.compile(r"\b(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_]{30,}\b")),
    ("github_pat", re.compile(r"\bgithub_pat_[A-Za-z0-9_]{20,}\b")),
    ("slack_token", re.compile(r"\bxox[baprs]-[A-Za-z0-9-]{20,}\b")),
    ("aws_access_key", re.compile(r"\bAKIA[0-9A-Z]{16}\b")),
    (
        "private_key",
        re.compile(
            r"-----BEGIN (?:RSA |DSA |EC |OPENSSH |ENCRYPTED )?PRIVATE KEY-----"
        ),
    ),
]

DOMAIN_TRACE_PATTERNS = [
    ("kr_bid_trace", re.compile(r"(?i)\bkr[-_\s]?bid\b")),
    (
        "korean_bid_trace",
        re.compile(r"(\uCF00\uC774\uC54C\s*\uBE44\uB4DC|\uC785\uCC30|\uB098\uB77C\uC7A5\uD130|\uC870\uB2EC\uCCAD)"),
    ),
]

LOCAL_PATH_PATTERNS = [
    ("mac_home_path", re.compile(r"(?<![A-Za-z]:)/Users/[A-Za-z0-9._-]+/")),
    ("linux_home_path", re.compile(r"/home/[A-Za-z0-9._-]+/")),
    ("shell_home_path", re.compile(r"\$(?:HOME|\{HOME\})[/\\]")),
    ("windows_home_path", re.compile(r"[A-Za-z]:[/\\]Users[/\\][^/\\\s]+[/\\]")),
    (
        "windows_drive_path",
        re.compile(r"(?<![A-Za-z])[A-Za-z]:[/\\](?!Users[/\\])", re.IGNORECASE),
    ),
    (
        "windows_unc_path",
        re.compile(r"(?<!\\)\\\\[A-Za-z0-9._-]{2,}\\[^\\\s]{2,}\\"),
    ),
    (
        "windows_profile_env_path",
        re.compile(r"%(?:USERPROFILE|APPDATA|LOCALAPPDATA)%[/\\]", re.IGNORECASE),
    ),
    (
        "powershell_profile_env_path",
        re.compile(r"\$env:(?:USERPROFILE|APPDATA|LOCALAPPDATA)[/\\]", re.IGNORECASE),
    ),
]

OFFICE_TEXT_PART_SUFFIXES = (".xml", ".rels")
OFFICE_PACKAGE_SUFFIXES = (
    ".docx",
    ".docm",
    ".dotx",
    ".dotm",
    ".pptx",
    ".pptm",
    ".ppsx",
    ".ppsm",
    ".xlsx",
    ".xlsm",
    ".xltx",
    ".xltm",
)
MAX_OFFICE_TEXT_PART_BYTES = 4 * 1024 * 1024
MAX_EMBEDDED_OFFICE_PACKAGE_BYTES = 16 * 1024 * 1024
MAX_NESTED_OFFICE_PACKAGE_DEPTH = 2
BINARY_TEXT_SCAN_SUFFIXES = {".doc"}
BINARY_TEXT_SCAN_ENCODINGS = ("utf-8", "cp949", "utf-16le", "utf-16be")
MAX_BINARY_TEXT_SCAN_BYTES = 16 * 1024 * 1024


@dataclass(frozen=True)
class Finding:
    path: str
    line: int | None
    kind: str
    detail: str

    def as_dict(self) -> dict[str, object]:
        return {
            "path": self.path,
            "line": self.line,
            "kind": self.kind,
            "detail": self.detail,
        }


def git_files() -> list[Path]:
    completed = subprocess.run(
        ["git", "ls-files", "-co", "--exclude-standard", "-z"],
        cwd=REPO,
        check=True,
        capture_output=True,
    )
    files = []
    for raw in completed.stdout.split(b"\0"):
        if raw:
            files.append(REPO / raw.decode("utf-8", "surrogateescape"))
    return sorted(files)


def should_skip(path: Path) -> bool:
    rel = path.relative_to(REPO)
    if any(part in SKIP_DIRS for part in rel.parts):
        return True
    provenance = PROVENANCED_BINARY_FILES.get(rel.as_posix())
    if provenance is not None and path.suffix.lower() == ".ttf":
        return (REPO / provenance).is_file()
    if path.suffix.lower() in BINARY_SUFFIXES:
        return True
    return not path.is_file()


def relpath(path: Path) -> str:
    return path.relative_to(REPO).as_posix()


def is_top_level_office_package(path: Path) -> bool:
    return path.suffix.lower() in OFFICE_PACKAGE_SUFFIXES


def is_binary_text_scan_file(path: Path) -> bool:
    return path.suffix.lower() in BINARY_TEXT_SCAN_SUFFIXES


def audit_paths(paths: list[Path]) -> list[Finding]:
    findings: list[Finding] = []
    for path in paths:
        rel = relpath(path)
        parts = Path(rel).parts
        if parts and parts[0] == "corpus" and not rel.startswith("corpus/public/"):
            findings.append(
                Finding(
                    rel,
                    None,
                    "non_public_corpus_file",
                    "corpus files must live under corpus/public/",
                )
            )
        for kind, pattern in DOMAIN_TRACE_PATTERNS:
            if pattern.search(rel):
                findings.append(
                    Finding(rel, None, kind, "domain-specific release trace in path")
                )
    return findings


def audit_text_file(path: Path) -> list[Finding]:
    rel = relpath(path)
    try:
        text = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return [Finding(rel, None, "non_utf8_text", "text file is not valid UTF-8")]
    return audit_text_lines(rel, text.splitlines())


def audit_text_lines(path: str, lines: list[str]) -> list[Finding]:
    findings: list[Finding] = []
    for line_no, line in enumerate(lines, start=1):
        for kind, pattern in SECRET_PATTERNS:
            if pattern.search(line):
                findings.append(Finding(path, line_no, kind, "secret-like token literal"))
        for kind, pattern in DOMAIN_TRACE_PATTERNS:
            if pattern.search(line):
                findings.append(
                    Finding(path, line_no, kind, "domain-specific release trace")
                )
        for kind, pattern in LOCAL_PATH_PATTERNS:
            if pattern.search(line):
                findings.append(
                    Finding(path, line_no, kind, "absolute local filesystem path")
                )
        if re.search(r"RWML_(?:BENCH_CORPUS|RENDER_CORPUS|PRIVATE_FIXTURES)\s*[:=]", line):
            findings.append(
                Finding(
                    path,
                    line_no,
                    "private_corpus_default",
                    "private corpus env vars must not be assigned in committed files",
                )
            )
    return findings


def audit_text_blob(path: str, text: str, detail_context: str) -> list[Finding]:
    findings: list[Finding] = []
    for kind, pattern in SECRET_PATTERNS:
        if pattern.search(text):
            findings.append(
                Finding(
                    path,
                    None,
                    kind,
                    f"secret-like token literal in {detail_context}",
                )
            )
    for kind, pattern in DOMAIN_TRACE_PATTERNS:
        if pattern.search(text):
            findings.append(
                Finding(
                    path,
                    None,
                    kind,
                    f"domain-specific release trace in {detail_context}",
                )
            )
    for kind, pattern in LOCAL_PATH_PATTERNS:
        if pattern.search(text):
            findings.append(
                Finding(
                    path,
                    None,
                    kind,
                    f"absolute local filesystem path in {detail_context}",
                )
            )
    if re.search(r"RWML_(?:BENCH_CORPUS|RENDER_CORPUS|PRIVATE_FIXTURES)\s*[:=]", text):
        findings.append(
            Finding(
                path,
                None,
                "private_corpus_default",
                f"private corpus env var assignment in {detail_context}",
            )
        )
    return findings


def decode_binary_text_views(data: bytes) -> list[str]:
    views: list[str] = []
    for encoding in BINARY_TEXT_SCAN_ENCODINGS:
        views.append(data.decode(encoding, errors="ignore"))
    return views


def audit_binary_document_file(path: Path) -> list[Finding]:
    rel = relpath(path)
    size = path.stat().st_size
    if size > MAX_BINARY_TEXT_SCAN_BYTES:
        return [
            Finding(
                rel,
                None,
                "binary_text_scan_too_large",
                "legacy binary document exceeds bounded text-view hygiene scan limit",
            )
        ]

    findings: list[Finding] = []
    seen: set[tuple[str, str, str]] = set()
    data = path.read_bytes()
    for text in decode_binary_text_views(data):
        for finding in audit_text_blob(rel, text, "legacy binary document text view"):
            key = (finding.path, finding.kind, finding.detail)
            if key not in seen:
                seen.add(key)
                findings.append(finding)
    return findings


def is_office_text_part(name: str) -> bool:
    lowered = name.lower()
    return name == "[Content_Types].xml" or lowered.endswith(OFFICE_TEXT_PART_SUFFIXES)


def is_office_package_part(name: str) -> bool:
    return name.lower().endswith(OFFICE_PACKAGE_SUFFIXES)


def decode_office_text_part(data: bytes) -> str | None:
    for encoding in ("utf-8-sig", "utf-16"):
        try:
            return data.decode(encoding)
        except UnicodeDecodeError:
            continue
    return None


def audit_office_zip(
    archive: zipfile.ZipFile,
    display_prefix: str,
    *,
    depth: int = 0,
) -> list[Finding]:
    findings: list[Finding] = []
    for info in archive.infolist():
        if info.is_dir():
            continue
        display_path = f"{display_prefix}::{info.filename}"
        findings.extend(
            audit_text_blob(display_path, info.filename, "Office package member path")
        )
        if is_office_text_part(info.filename):
            if info.file_size > MAX_OFFICE_TEXT_PART_BYTES:
                findings.append(
                    Finding(
                        display_path,
                        None,
                        "office_text_part_too_large",
                        "textual Office package part is too large for hygiene scanning",
                    )
                )
                continue
            text = decode_office_text_part(archive.read(info))
            if text is None:
                findings.append(
                    Finding(
                        display_path,
                        None,
                        "non_utf8_office_text_part",
                        "textual Office package part is not valid UTF-8 or UTF-16",
                    )
                )
                continue
            findings.extend(audit_text_lines(display_path, text.splitlines()))
            continue
        if depth < MAX_NESTED_OFFICE_PACKAGE_DEPTH and is_office_package_part(info.filename):
            if info.file_size > MAX_EMBEDDED_OFFICE_PACKAGE_BYTES:
                findings.append(
                    Finding(
                        display_path,
                        None,
                        "embedded_office_package_too_large",
                        "embedded Office package is too large for hygiene scanning",
                    )
                )
                continue
            try:
                data = archive.read(info)
                with zipfile.ZipFile(io.BytesIO(data)) as nested:
                    findings.extend(audit_office_zip(nested, display_path, depth=depth + 1))
            except zipfile.BadZipFile:
                findings.append(
                    Finding(
                        display_path,
                        None,
                        "invalid_embedded_office_package",
                        "embedded Office package is not a valid ZIP package",
                    )
                )
    return findings


def audit_office_file(path: Path) -> list[Finding]:
    rel = relpath(path)
    try:
        with zipfile.ZipFile(path) as archive:
            findings = audit_office_zip(archive, rel)
    except zipfile.BadZipFile:
        findings = [
            Finding(
                rel,
                None,
                "invalid_office_package",
                "Office package is not a valid ZIP package",
            )
        ]
    return findings


def audit_docx_file(path: Path) -> list[Finding]:
    return audit_office_file(path)


def audit() -> list[Finding]:
    files = git_files()
    findings = audit_paths(files)
    for path in files:
        if is_top_level_office_package(path) and path.is_file():
            findings.extend(audit_office_file(path))
        elif is_binary_text_scan_file(path) and path.is_file():
            findings.extend(audit_binary_document_file(path))
        elif not should_skip(path):
            findings.extend(audit_text_file(path))
    return sorted(findings, key=lambda item: (item.path, item.line or 0, item.kind))


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--json",
        action="store_true",
        help="emit machine-readable JSON instead of text",
    )
    return parser.parse_args(argv)


def json_payload(payload: dict) -> str:
    return json.dumps(
        payload,
        ensure_ascii=False,
        indent=2,
        sort_keys=True,
        allow_nan=False,
    )


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    findings = audit()
    if args.json:
        print(
            json_payload(
                {
                    "schema": "rwml.public-hygiene-audit.v1",
                    "passed": not findings,
                    "findings": [finding.as_dict() for finding in findings],
                }
            )
        )
    elif findings:
        print("public_hygiene_audit: release blockers found", file=sys.stderr)
        for finding in findings:
            location = finding.path
            if finding.line is not None:
                location += f":{finding.line}"
            print(f"{location}: {finding.kind}: {finding.detail}", file=sys.stderr)
    return 1 if findings else 0


if __name__ == "__main__":
    raise SystemExit(main())
