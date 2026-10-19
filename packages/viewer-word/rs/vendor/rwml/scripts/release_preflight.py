#!/usr/bin/env python3
"""Build and verify a complete local release candidate without external writes.

The preflight runs the same release evidence producers used by the tag workflow,
then assembles both crate archives and the release manifest under ``target/``.
It intentionally has no registry-upload or GitHub-release command path.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import subprocess
import sys
from collections.abc import Sequence


ROOT = pathlib.Path(__file__).resolve().parents[1]
CARGO = os.environ.get("CARGO", "cargo")
PYTHON = os.environ.get("PYTHON", sys.executable)
PYMUPDF_REQUIREMENT = "PyMuPDF==1.28.2"
PILLOW_REQUIREMENT = "Pillow==12.3.0"
PYTHON_DOCX_REQUIREMENT = "python-docx==1.2.0"
PYTHON_TOOL_CHECK = (
    "import docx, pymupdf, PIL; "
    'assert docx.__version__ == "1.2.0"; '
    'assert pymupdf.__version__ == "1.28.2"; '
    'assert PIL.__version__ == "12.3.0"'
)
COMMAND_ENV = os.environ.copy()
if pathlib.Path(CARGO).is_absolute():
    cargo_dir = str(pathlib.Path(CARGO).parent)
    COMMAND_ENV["PATH"] = os.pathsep.join(
        [cargo_dir, COMMAND_ENV.get("PATH", "")]
    )
PACKAGE_COMMAND = "cargo package"
ASSET_NAME_TEMPLATES = (
    "rwml-{version}.crate",
    "rwml-fonts-{version}.crate",
    "public-hygiene.json",
    "render-validation.json",
    "extract-benchmark.json",
    "rwml-release-manifest.json",
)


def run(command: Sequence[str], *, stdout: object | None = None) -> None:
    printable = " ".join(str(part) for part in command)
    print(f"$ {printable}")
    subprocess.run(
        command,
        cwd=ROOT,
        check=True,
        stdout=stdout,
        env=COMMAND_ENV,
    )


def run_to_file(command: Sequence[str], path: pathlib.Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as stream:
        run(command, stdout=stream)


def run_json_to_file(command: Sequence[str], path: pathlib.Path) -> None:
    printable = " ".join(str(part) for part in command)
    print(f"$ {printable}")
    completed = subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        capture_output=True,
        text=True,
        env=COMMAND_ENV,
    )
    if completed.stderr:
        sys.stderr.write(completed.stderr)
    if completed.returncode != 0:
        raise subprocess.CalledProcessError(
            completed.returncode,
            command,
            output=completed.stdout,
            stderr=completed.stderr,
        )
    payload_start = completed.stdout.find("{")
    if payload_start < 0:
        raise RuntimeError(f"JSON-producing command emitted no object: {printable}")
    try:
        payload, payload_end = json.JSONDecoder().raw_decode(
            completed.stdout[payload_start:]
        )
    except json.JSONDecodeError as error:
        raise RuntimeError(f"JSON-producing command emitted invalid JSON: {printable}") from error
    if completed.stdout[payload_start + payload_end :].strip():
        raise RuntimeError(f"JSON-producing command emitted trailing text: {printable}")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def git_output(*arguments: str) -> str:
    return subprocess.check_output(
        ["git", *arguments], cwd=ROOT, text=True, env=COMMAND_ENV
    ).strip()


def cargo_version() -> str:
    metadata = json.loads(
        subprocess.check_output(
            [CARGO, "metadata", "--no-deps", "--format-version", "1"],
            cwd=ROOT,
            text=True,
            env=COMMAND_ENV,
        )
    )
    for package in metadata.get("packages", []):
        if package.get("name") == "rwml":
            version = package.get("version")
            if isinstance(version, str) and version:
                return version
    raise RuntimeError("cargo metadata did not contain the rwml package")


def require_clean_worktree() -> None:
    status = git_output("status", "--porcelain")
    if status:
        raise RuntimeError(
            "release preflight requires a clean worktree; commit or remove local changes first"
        )


def expected_release_assets(version: str) -> tuple[str, ...]:
    return tuple(
        template.format(
            version=version,
        )
        for template in ASSET_NAME_TEMPLATES
    )


def relative(path: pathlib.Path) -> str:
    try:
        return path.relative_to(ROOT).as_posix()
    except ValueError:
        return path.as_posix()


def cargo_target_directory() -> pathlib.Path:
    target_dir = pathlib.Path(COMMAND_ENV["CARGO_TARGET_DIR"])
    return target_dir if target_dir.is_absolute() else ROOT / target_dir


def cargo_example_binary(
    target_dir: pathlib.Path, name: str, *, platform_name: str | None = None
) -> pathlib.Path:
    platform_name = os.name if platform_name is None else platform_name
    filename = f"{name}.exe" if platform_name == "nt" else name
    return target_dir / "release" / "examples" / filename


def run_python(script: str, *arguments: str) -> list[str]:
    return [PYTHON, f"scripts/{script}", *arguments]


def run_python_with(interpreter: str, script: str, *arguments: str) -> list[str]:
    return [interpreter, f"scripts/{script}", *arguments]


def venv_interpreter(
    venv_dir: pathlib.Path, *, platform_name: str | None = None
) -> pathlib.Path:
    platform_name = os.name if platform_name is None else platform_name
    if platform_name == "nt":
        return venv_dir / "Scripts" / "python.exe"
    return venv_dir / "bin" / "python"


def ensure_validation_tools(output_dir: pathlib.Path) -> str:
    venv_dir = output_dir / "python-tools"
    interpreter = venv_interpreter(venv_dir)
    if not interpreter.is_file():
        run([PYTHON, "-m", "venv", relative(venv_dir)])
    try:
        run([str(interpreter), "-c", PYTHON_TOOL_CHECK])
    except subprocess.CalledProcessError:
        run(
            [
                str(interpreter),
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "--no-cache-dir",
                PYMUPDF_REQUIREMENT,
                PILLOW_REQUIREMENT,
                PYTHON_DOCX_REQUIREMENT,
            ]
        )
        run([str(interpreter), "-c", PYTHON_TOOL_CHECK])
    return str(interpreter)


def build_preflight(output_dir: pathlib.Path) -> dict[str, object]:
    require_clean_worktree()
    version = cargo_version()
    git_rev = git_output("rev-parse", "HEAD")
    output_dir.mkdir(parents=True, exist_ok=True)

    run(run_python("public_hygiene_audit.py"))
    run(run_python("gen_public_corpus.py", "--check"))
    run([CARGO, "audit"])
    run(
        [
            CARGO,
            "check",
            "--manifest-path",
            "fuzz/Cargo.toml",
            "--all-targets",
            "--locked",
        ]
    )
    run(
        [
            CARGO,
            "package",
            "--manifest-path",
            "rwml-fonts/Cargo.toml",
        ]
    )
    run(
        [
            CARGO,
            "package",
            "--locked",
            "--config",
            'patch.crates-io.rwml-fonts.path="rwml-fonts"',
        ]
    )

    hygiene_report = output_dir / "public-hygiene.json"
    render_report = output_dir / "render-validation.json"
    benchmark_report = output_dir / "extract-benchmark.json"
    manifest = output_dir / "rwml-release-manifest.json"
    validation_python = ensure_validation_tools(output_dir)
    run(
        [
            validation_python,
            "-m",
            "unittest",
            "discover",
            "-s",
            "tests",
            "-p",
            "test_*.py",
        ]
    )
    edit_output = output_dir / "edit-validation"
    run(
        [
            CARGO,
            "run",
            "--locked",
            "--example",
            "validate_edit",
            "--features",
            "docx",
            "--",
            "corpus/public",
            relative(edit_output),
        ]
    )
    run(
        run_python_with(
            validation_python,
            "validate_edit_check.py",
            "corpus/public",
            relative(edit_output),
        )
    )
    run_to_file(run_python("public_hygiene_audit.py", "--json"), hygiene_report)
    run_json_to_file(
        run_python_with(
            validation_python,
            "render_validate.py",
            "--json",
            "--soffice",
            "local",
            "--manifest",
            "corpus/public/RENDER_MANIFEST.tsv",
            "--recall-min",
            "0.97",
            "--min-mean-recall",
            "0.90",
            "--max-skipped",
            "0",
            "--verify-oracle",
        ),
        render_report,
    )
    target_dir = cargo_target_directory()
    extract_binary = cargo_example_binary(target_dir, "extract")
    run(
        [CARGO, "build", "--release", "--example", "extract", "--locked"]
    )
    run(
        run_python_with(
            validation_python,
            "bench_vs_mature.py",
            "--corpus",
            "corpus/public/benchmark",
            "--extract-bin",
            relative(extract_binary),
            "--json",
            "--version",
            version,
            "--git-rev",
            git_rev,
            "--min-poi-recall-mean",
            "0.95",
            "--min-poi-f1-mean",
            "0.95",
            "--min-lo-recall-mean",
            "0.95",
            "--max-errors",
            "0",
            "--min-scored",
            "3",
            "--max-scored",
            "3",
            "--min-lo-scored",
            "3",
            "--max-lo-scored",
            "3",
            "--output",
            relative(benchmark_report),
        )
    )

    font_artifact = target_dir / "package" / f"rwml-fonts-{version}.crate"
    main_artifact = target_dir / "package" / f"rwml-{version}.crate"
    run(
        run_python(
            "release_manifest.py",
            "--version",
            version,
            "--git-rev",
            git_rev,
            "--release-policy",
            "public-release",
            "--enforce-policy-inputs",
            "--hygiene-report",
            relative(hygiene_report),
            "--validation-report",
            relative(render_report),
            "--benchmark-report",
            relative(benchmark_report),
            "--corpus-manifest",
            "corpus/public/MANIFEST.tsv",
            "--corpus-manifest",
            "corpus/public/RENDER_MANIFEST.tsv",
            "--output",
            relative(manifest),
            relative(main_artifact),
            relative(font_artifact),
        )
    )

    expected_paths = {
        expected_release_assets(version)[0]: main_artifact,
        expected_release_assets(version)[1]: font_artifact,
        expected_release_assets(version)[2]: hygiene_report,
        expected_release_assets(version)[3]: render_report,
        expected_release_assets(version)[4]: benchmark_report,
        expected_release_assets(version)[5]: manifest,
    }
    missing = [name for name, path in expected_paths.items() if not path.is_file()]
    if missing:
        raise RuntimeError("preflight did not produce expected assets: " + ", ".join(missing))

    return {
        "version": version,
        "git_rev": git_rev,
        "output_dir": relative(output_dir),
        "assets": expected_release_assets(version),
    }


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--output-dir",
        type=pathlib.Path,
        default=pathlib.Path("target/release-preflight"),
        help="ignored local directory for evidence and the release manifest",
    )
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    output_dir = args.output_dir
    if not output_dir.is_absolute():
        output_dir = ROOT / output_dir
    COMMAND_ENV.setdefault("CARGO_TARGET_DIR", str(output_dir / "cargo-target"))
    try:
        result = build_preflight(output_dir)
    except (OSError, RuntimeError, subprocess.CalledProcessError, json.JSONDecodeError) as error:
        print(f"release_preflight: {error}", file=sys.stderr)
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
