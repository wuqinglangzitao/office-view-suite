#!/usr/bin/env python3
"""Fail-closed python-docx validation for package-preserving edit outputs.

The input inventory comes from ``MANIFEST.tsv`` when present and otherwise from
a recursive ``*.docx`` scan. Both output trees must contain exactly that same
inventory:

* ``pass/`` documents must open and retain byte-identical ZIP part payloads;
* ``bimg/`` documents must open and expose at least one inline image.
"""

from __future__ import annotations

import argparse
import pathlib
import sys
import zipfile
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from typing import Any


MANIFEST_NAME = "MANIFEST.tsv"


@dataclass
class ValidationSummary:
    expected: int = 0
    pass_open: int = 0
    pass_open_fail: int = 0
    pass_byte_stable: int = 0
    pass_drift: int = 0
    pass_compare_fail: int = 0
    image_open: int = 0
    image_open_fail: int = 0
    image_present: int = 0
    image_missing: int = 0
    missing_outputs: int = 0
    unexpected_outputs: int = 0

    def passed(self) -> bool:
        return (
            self.expected > 0
            and self.pass_open == self.expected
            and self.pass_byte_stable == self.expected
            and self.image_open == self.expected
            and self.image_present == self.expected
            and self.pass_open_fail == 0
            and self.pass_drift == 0
            and self.pass_compare_fail == 0
            and self.image_open_fail == 0
            and self.image_missing == 0
            and self.missing_outputs == 0
            and self.unexpected_outputs == 0
        )


def safe_relative_docx(raw: str) -> pathlib.PurePosixPath:
    relative = pathlib.PurePosixPath(raw)
    if (
        not raw
        or "\\" in raw
        or ":" in raw
        or relative.is_absolute()
        or relative.suffix != ".docx"
        or any(part in {"", ".", ".."} for part in relative.parts)
    ):
        raise ValueError(f"invalid DOCX path: {raw!r}")
    return relative


def recursive_docx(root: pathlib.Path) -> set[pathlib.PurePosixPath]:
    return {
        pathlib.PurePosixPath(path.relative_to(root).as_posix())
        for path in root.rglob("*.docx")
        if path.is_file()
    }


def expected_docx(root: pathlib.Path) -> list[pathlib.PurePosixPath]:
    if not root.is_dir():
        raise ValueError(f"input directory does not exist: {root}")
    discovered = recursive_docx(root)
    if not discovered:
        raise ValueError(f"no DOCX inputs found under {root}")

    manifest = root / MANIFEST_NAME
    if not manifest.is_file():
        return sorted(discovered, key=str)

    listed: list[pathlib.PurePosixPath] = []
    seen: set[pathlib.PurePosixPath] = set()
    for line_number, line in enumerate(
        manifest.read_text(encoding="utf-8").splitlines(), start=1
    ):
        line = line.strip()
        if not line or line.startswith("#") or line.startswith("path\t"):
            continue
        relative = safe_relative_docx(line.split("\t", 1)[0])
        if relative in seen:
            raise ValueError(
                f"{manifest}:{line_number} repeats DOCX path: {relative}"
            )
        if not (root / pathlib.Path(*relative.parts)).is_file():
            raise ValueError(
                f"{manifest}:{line_number} references missing DOCX: {relative}"
            )
        seen.add(relative)
        listed.append(relative)
    if not listed:
        raise ValueError(f"{manifest} contains no DOCX inputs")
    if seen != discovered:
        missing = sorted((str(path) for path in seen - discovered))
        unlisted = sorted((str(path) for path in discovered - seen))
        raise ValueError(
            f"{manifest} does not exactly match the recursive DOCX inventory; "
            f"missing={missing} unlisted={unlisted}"
        )
    return sorted(listed, key=str)


def zip_parts(path: pathlib.Path) -> dict[str, bytes]:
    with zipfile.ZipFile(path) as archive:
        return {name: archive.read(name) for name in archive.namelist()}


def output_inventory(root: pathlib.Path, kind: str) -> set[pathlib.PurePosixPath]:
    directory = root / kind
    if not directory.is_dir():
        return set()
    return recursive_docx(directory)


def validate_outputs(
    input_dir: pathlib.Path,
    output_dir: pathlib.Path,
    document_loader: Callable[[pathlib.Path], Any],
) -> ValidationSummary:
    expected = expected_docx(input_dir)
    expected_set = set(expected)
    summary = ValidationSummary(expected=len(expected))

    for kind in ("pass", "bimg"):
        actual = output_inventory(output_dir, kind)
        missing = expected_set - actual
        unexpected = actual - expected_set
        summary.missing_outputs += len(missing)
        summary.unexpected_outputs += len(unexpected)
        for relative in sorted(missing, key=str):
            print(f"{kind.upper()}-MISSING {relative}", file=sys.stderr)
        for relative in sorted(unexpected, key=str):
            print(f"{kind.upper()}-UNEXPECTED {relative}", file=sys.stderr)

    for relative in expected:
        native_relative = pathlib.Path(*relative.parts)
        source = input_dir / native_relative
        passthrough = output_dir / "pass" / native_relative
        image_edit = output_dir / "bimg" / native_relative

        if passthrough.is_file():
            try:
                document_loader(passthrough)
                summary.pass_open += 1
            except Exception as error:
                summary.pass_open_fail += 1
                print(f"PASS-OPEN-FAIL {relative}: {error}", file=sys.stderr)
            try:
                original_parts = zip_parts(source)
                saved_parts = zip_parts(passthrough)
                if original_parts == saved_parts:
                    summary.pass_byte_stable += 1
                else:
                    summary.pass_drift += 1
                    differing = sorted(
                        set(original_parts).symmetric_difference(saved_parts)
                        | {
                            name
                            for name in set(original_parts).intersection(saved_parts)
                            if original_parts[name] != saved_parts[name]
                        }
                    )
                    print(
                        f"PASS-DRIFT {relative}: {differing[:6]}",
                        file=sys.stderr,
                    )
            except Exception as error:
                summary.pass_compare_fail += 1
                print(f"PASS-COMPARE-FAIL {relative}: {error}", file=sys.stderr)

        if image_edit.is_file():
            try:
                document = document_loader(image_edit)
                summary.image_open += 1
                if len(document.inline_shapes) >= 1:
                    summary.image_present += 1
                else:
                    summary.image_missing += 1
                    print(f"BIMG-NO-IMAGE {relative}", file=sys.stderr)
            except Exception as error:
                summary.image_open_fail += 1
                print(f"BIMG-OPEN-FAIL {relative}: {error}", file=sys.stderr)

    return summary


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input_dir", type=pathlib.Path)
    parser.add_argument("output_dir", type=pathlib.Path)
    return parser.parse_args(argv)


def print_summary(summary: ValidationSummary) -> None:
    print("--- PASSTHROUGH ---")
    print(
        f"expected={summary.expected} open_ok={summary.pass_open} "
        f"open_fail={summary.pass_open_fail} byte_stable={summary.pass_byte_stable} "
        f"drift={summary.pass_drift} compare_fail={summary.pass_compare_fail}"
    )
    print("--- TREE-EDIT IMAGE ---")
    print(
        f"expected={summary.expected} open_ok={summary.image_open} "
        f"open_fail={summary.image_open_fail} inline_image={summary.image_present} "
        f"missing_image={summary.image_missing}"
    )
    print(
        f"missing_outputs={summary.missing_outputs} "
        f"unexpected_outputs={summary.unexpected_outputs}"
    )


def main(
    argv: Sequence[str] | None = None,
    *,
    document_loader: Callable[[pathlib.Path], Any] | None = None,
) -> int:
    args = parse_args(argv)
    if document_loader is None:
        try:
            from docx import Document
        except ImportError as error:
            print(
                "validate_edit_check: python-docx is required "
                "(release pin: python-docx==1.2.0)",
                file=sys.stderr,
            )
            return 2
        document_loader = Document
    try:
        summary = validate_outputs(args.input_dir, args.output_dir, document_loader)
    except (OSError, ValueError, zipfile.BadZipFile) as error:
        print(f"validate_edit_check: {error}", file=sys.stderr)
        return 2
    print_summary(summary)
    return 0 if summary.passed() else 1


if __name__ == "__main__":
    raise SystemExit(main())
