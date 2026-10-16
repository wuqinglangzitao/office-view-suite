#!/usr/bin/env python3
"""Idempotently publish a crate while verifying immutable registry identity."""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import pathlib
import subprocess
import sys
import tarfile
import tempfile
import time
from typing import Optional
from urllib import error, parse, request


API_ROOT = "https://crates.io/api/v1/crates"
DOWNLOAD_ROOT = "https://static.crates.io/crates"
USER_AGENT = "rwml-release-ci (github actions)"


class PublishError(RuntimeError):
    pass


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as artifact:
        for chunk in iter(lambda: artifact.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def registry_checksum(name: str, version: str) -> Optional[str]:
    url = f"{API_ROOT}/{parse.quote(name, safe='')}/{parse.quote(version, safe='')}"
    registry_request = request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with request.urlopen(registry_request, timeout=15) as response:
            payload = json.loads(response.read())
    except error.HTTPError as exc:
        if exc.fp is not None:
            exc.close()
        if exc.code == 404:
            return None
        raise PublishError(f"crates.io returned HTTP {exc.code} for {name} {version}") from exc
    except (error.URLError, TimeoutError, json.JSONDecodeError) as exc:
        raise PublishError(f"failed to query crates.io for {name} {version}: {exc}") from exc

    checksum = payload.get("version", {}).get("checksum")
    if not isinstance(checksum, str) or len(checksum) != 64:
        raise PublishError(f"crates.io returned no valid checksum for {name} {version}")
    try:
        bytes.fromhex(checksum)
    except ValueError as exc:
        raise PublishError(f"crates.io returned an invalid checksum for {name} {version}") from exc
    return checksum.lower()


def require_matching_checksum(name: str, version: str, local: str, remote: str) -> None:
    if local != remote:
        raise PublishError(
            f"published {name} {version} checksum {remote} does not match local artifact {local}"
        )


def registry_artifact(name: str, version: str, expected_checksum: str) -> bytes:
    encoded_name = parse.quote(name, safe="")
    encoded_file = parse.quote(f"{name}-{version}.crate", safe="")
    url = f"{DOWNLOAD_ROOT}/{encoded_name}/{encoded_file}"
    artifact_request = request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with request.urlopen(artifact_request, timeout=30) as response:
            artifact = response.read()
    except error.HTTPError as exc:
        if exc.fp is not None:
            exc.close()
        raise PublishError(
            f"crates.io returned HTTP {exc.code} for {name} {version} artifact"
        ) from exc
    except (error.URLError, TimeoutError) as exc:
        raise PublishError(
            f"failed to download crates.io artifact for {name} {version}: {exc}"
        ) from exc

    actual_checksum = hashlib.sha256(artifact).hexdigest()
    require_matching_checksum(name, version, actual_checksum, expected_checksum)
    return artifact


def normalized_vcs_info(data: bytes, name: str, version: str) -> bytes:
    try:
        payload = json.loads(data)
        git = payload["git"]
        sha1 = git["sha1"]
    except (UnicodeDecodeError, json.JSONDecodeError, KeyError, TypeError) as exc:
        raise PublishError(
            f"invalid .cargo_vcs_info.json in {name} {version} artifact"
        ) from exc
    if not isinstance(sha1, str) or len(sha1) != 40:
        raise PublishError(f"invalid VCS revision in {name} {version} artifact")
    try:
        bytes.fromhex(sha1)
    except ValueError as exc:
        raise PublishError(f"invalid VCS revision in {name} {version} artifact") from exc
    git["sha1"] = "<normalized>"
    return json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")


def crate_payload(
    archive_source: pathlib.Path | bytes, name: str, version: str
) -> dict[str, tuple[int, bytes]]:
    expected_root = f"{name}-{version}"
    fileobj = io.BytesIO(archive_source) if isinstance(archive_source, bytes) else None
    try:
        with tarfile.open(
            archive_source if isinstance(archive_source, pathlib.Path) else None,
            mode="r:gz",
            fileobj=fileobj,
        ) as archive:
            files: dict[str, tuple[int, bytes]] = {}
            for member in archive.getmembers():
                path = pathlib.PurePosixPath(member.name)
                if path.is_absolute() or ".." in path.parts:
                    raise PublishError(f"unsafe path in {name} {version} artifact")
                if not path.parts or path.parts[0] != expected_root:
                    raise PublishError(f"unexpected package root in {name} {version} artifact")
                if member.isdir():
                    continue
                if not member.isfile() or len(path.parts) < 2:
                    raise PublishError(f"unsupported entry in {name} {version} artifact")
                relative = pathlib.PurePosixPath(*path.parts[1:]).as_posix()
                if relative in files:
                    raise PublishError(f"duplicate path in {name} {version} artifact")
                extracted = archive.extractfile(member)
                if extracted is None:
                    raise PublishError(f"unreadable entry in {name} {version} artifact")
                data = extracted.read()
                if relative == ".cargo_vcs_info.json":
                    data = normalized_vcs_info(data, name, version)
                files[relative] = (member.mode & 0o777, data)
    except (OSError, tarfile.TarError) as exc:
        raise PublishError(f"invalid crate archive for {name} {version}: {exc}") from exc
    return files


def synchronize_published_artifact(
    name: str,
    version: str,
    artifact: pathlib.Path,
    local_checksum: str,
    published_checksum: str,
) -> None:
    published_artifact = registry_artifact(name, version, published_checksum)
    local_payload = crate_payload(artifact, name, version)
    published_payload = crate_payload(published_artifact, name, version)
    if local_payload != published_payload:
        raise PublishError(
            f"published {name} {version} package payload differs from local artifact"
        )

    staged_path: Optional[pathlib.Path] = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb",
            dir=artifact.parent,
            prefix=f".{artifact.name}.",
            delete=False,
        ) as staged:
            staged.write(published_artifact)
            staged_path = pathlib.Path(staged.name)
        staged_path.replace(artifact)
        staged_path = None
    finally:
        if staged_path is not None:
            staged_path.unlink(missing_ok=True)

    print(
        f"{name} {version} already published as {published_checksum}; "
        f"synchronized equivalent local repackage {local_checksum} to registry artifact"
    )


def wait_for_matching_version(
    name: str,
    version: str,
    local_checksum: str,
    attempts: int,
    interval: float,
) -> None:
    last_error: Optional[PublishError] = None
    for attempt in range(attempts):
        try:
            checksum = registry_checksum(name, version)
            last_error = None
        except PublishError as exc:
            checksum = None
            last_error = exc
        if checksum is not None:
            require_matching_checksum(name, version, local_checksum, checksum)
            return
        if attempt + 1 < attempts:
            time.sleep(interval)

    if last_error is not None:
        raise PublishError(
            f"{name} {version} did not become verifiable after publication: {last_error}"
        ) from last_error
    raise PublishError(f"{name} {version} did not become visible after publication")


def ensure_published(
    name: str,
    version: str,
    artifact: pathlib.Path,
    manifest_path: Optional[pathlib.Path],
    *,
    poll_attempts: int = 12,
    poll_interval: float = 10,
    check_only: bool = False,
) -> str:
    if poll_attempts < 1 or poll_interval < 0:
        raise PublishError("poll attempts must be positive and interval must be nonnegative")
    if not artifact.is_file():
        raise PublishError(f"crate artifact does not exist: {artifact}")

    local_checksum = sha256_file(artifact)
    published = registry_checksum(name, version)
    if published is not None:
        if published == local_checksum:
            print(f"{name} {version} already published with matching checksum")
        else:
            synchronize_published_artifact(
                name,
                version,
                artifact,
                local_checksum,
                published,
            )
        return "already-published"
    if check_only:
        print(f"{name} {version} is not published; local artifact is ready")
        return "not-published"

    command = ["cargo", "publish"]
    if manifest_path is not None:
        command.extend(["--manifest-path", str(manifest_path)])
    result = subprocess.run(command, check=False)

    try:
        wait_for_matching_version(
            name,
            version,
            local_checksum,
            attempts=poll_attempts,
            interval=poll_interval,
        )
    except PublishError as exc:
        if result.returncode != 0:
            raise PublishError(
                f"cargo publish exited with {result.returncode} and registry recovery failed: {exc}"
            ) from exc
        raise

    if result.returncode == 0:
        print(f"published {name} {version} with verified checksum")
        return "published"
    print(f"recovered {name} {version} after cargo publish exited with {result.returncode}")
    return "recovered"


def parse_args(argv: Optional[list[str]] = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--artifact", required=True, type=pathlib.Path)
    parser.add_argument("--manifest-path", type=pathlib.Path)
    parser.add_argument("--poll-attempts", type=int, default=12)
    parser.add_argument("--poll-interval", type=float, default=10)
    parser.add_argument("--check-only", action="store_true")
    return parser.parse_args(argv)


def main(argv: Optional[list[str]] = None) -> int:
    args = parse_args(argv)
    try:
        ensure_published(
            args.name,
            args.version,
            args.artifact,
            args.manifest_path,
            poll_attempts=args.poll_attempts,
            poll_interval=args.poll_interval,
            check_only=args.check_only,
        )
    except PublishError as exc:
        print(f"publish_crate: {exc}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
