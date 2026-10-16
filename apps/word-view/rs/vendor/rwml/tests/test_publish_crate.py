import hashlib
import importlib.util
import io
import json
import pathlib
import subprocess
import sys
import tarfile
import tempfile
import unittest
import urllib.error
from contextlib import redirect_stdout
from unittest import mock


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "scripts" / "publish_crate.py"
SPEC = importlib.util.spec_from_file_location("publish_crate", SCRIPT)
publish_crate = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = publish_crate
SPEC.loader.exec_module(publish_crate)


class FakeResponse:
    def __init__(self, payload):
        self.payload = (
            payload if isinstance(payload, bytes) else json.dumps(payload).encode("utf-8")
        )

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, traceback):
        return False

    def read(self):
        return self.payload


def missing_version():
    return urllib.error.HTTPError("https://example.invalid", 404, "missing", {}, None)


class PublishCrateTests(unittest.TestCase):
    def ensure_published(self, *args, **kwargs):
        with redirect_stdout(io.StringIO()):
            return publish_crate.ensure_published(*args, **kwargs)

    def artifact(self, root):
        path = pathlib.Path(root) / "crate-0.1.1.crate"
        path.write_bytes(b"deterministic crate bytes")
        checksum = hashlib.sha256(path.read_bytes()).hexdigest()
        return path, checksum

    def response(self, checksum):
        return FakeResponse({"version": {"checksum": checksum}})

    def packaged_artifact(self, root, filename, git_sha, source=b"pub fn value() {}\n"):
        path = pathlib.Path(root) / filename
        package_root = "rwml-0.1.1"
        files = {
            ".cargo_vcs_info.json": json.dumps(
                {"git": {"sha1": git_sha}, "path_in_vcs": ""},
                indent=2,
            ).encode("utf-8"),
            "Cargo.toml": b'[package]\nname = "rwml"\nversion = "0.1.1"\n',
            "src/lib.rs": source,
        }
        with tarfile.open(path, "w:gz") as archive:
            for relative, data in sorted(files.items()):
                info = tarfile.TarInfo(f"{package_root}/{relative}")
                info.mode = 0o644
                info.mtime = 0
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        checksum = hashlib.sha256(path.read_bytes()).hexdigest()
        return path, checksum

    def test_matching_published_version_skips_upload(self):
        with tempfile.TemporaryDirectory() as root:
            artifact, checksum = self.artifact(root)
            with mock.patch.object(
                publish_crate.request, "urlopen", return_value=self.response(checksum)
            ), mock.patch.object(publish_crate.subprocess, "run") as run:
                status = self.ensure_published(
                    "rwml", "0.1.1", artifact, None, poll_attempts=2, poll_interval=0
                )

        self.assertEqual(status, "already-published")
        run.assert_not_called()

    def test_preexisting_publication_reuses_equivalent_registry_artifact(self):
        with tempfile.TemporaryDirectory() as root:
            artifact, local_checksum = self.packaged_artifact(
                root, "local.crate", "1" * 40
            )
            remote, remote_checksum = self.packaged_artifact(
                root, "remote.crate", "2" * 40
            )
            remote_bytes = remote.read_bytes()
            self.assertNotEqual(local_checksum, remote_checksum)
            output = io.StringIO()
            with mock.patch.object(
                publish_crate.request,
                "urlopen",
                side_effect=[self.response(remote_checksum), FakeResponse(remote_bytes)],
            ), mock.patch.object(publish_crate.subprocess, "run") as run:
                with redirect_stdout(output):
                    status = publish_crate.ensure_published(
                        "rwml",
                        "0.1.1",
                        artifact,
                        None,
                        poll_attempts=2,
                        poll_interval=0,
                    )
            synchronized_bytes = artifact.read_bytes()

        self.assertEqual(status, "already-published")
        self.assertEqual(synchronized_bytes, remote_bytes)
        self.assertIn("synchronized", output.getvalue())
        run.assert_not_called()

    def test_preexisting_publication_rejects_different_package_payload(self):
        with tempfile.TemporaryDirectory() as root:
            artifact, _ = self.packaged_artifact(
                root, "local.crate", "1" * 40, source=b"pub fn value() { new() }\n"
            )
            remote, remote_checksum = self.packaged_artifact(
                root, "remote.crate", "2" * 40, source=b"pub fn value() { old() }\n"
            )
            with mock.patch.object(
                publish_crate.request,
                "urlopen",
                side_effect=[
                    self.response(remote_checksum),
                    FakeResponse(remote.read_bytes()),
                ],
            ), mock.patch.object(publish_crate.subprocess, "run"):
                with self.assertRaisesRegex(publish_crate.PublishError, "payload"):
                    self.ensure_published(
                        "rwml", "0.1.1", artifact, None, poll_attempts=2, poll_interval=0
                    )

    def test_preexisting_publication_rejects_tampered_registry_download(self):
        with tempfile.TemporaryDirectory() as root:
            artifact, _ = self.packaged_artifact(
                root, "local.crate", "1" * 40
            )
            remote, remote_checksum = self.packaged_artifact(
                root, "remote.crate", "2" * 40
            )
            tampered = remote.read_bytes() + b"tampered"
            with mock.patch.object(
                publish_crate.request,
                "urlopen",
                side_effect=[self.response(remote_checksum), FakeResponse(tampered)],
            ), mock.patch.object(publish_crate.subprocess, "run"):
                with self.assertRaisesRegex(publish_crate.PublishError, "checksum"):
                    self.ensure_published(
                        "rwml", "0.1.1", artifact, None, poll_attempts=2, poll_interval=0
                    )

    def test_failed_publish_recovers_when_matching_version_appears(self):
        with tempfile.TemporaryDirectory() as root:
            artifact, checksum = self.artifact(root)
            responses = [missing_version(), missing_version(), self.response(checksum)]
            with mock.patch.object(
                publish_crate.request, "urlopen", side_effect=responses
            ), mock.patch.object(
                publish_crate.subprocess,
                "run",
                return_value=subprocess.CompletedProcess([], 101),
            ) as run, mock.patch.object(publish_crate.time, "sleep"):
                status = self.ensure_published(
                    "rwml-fonts",
                    "0.1.1",
                    artifact,
                    pathlib.Path("rwml-fonts/Cargo.toml"),
                    poll_attempts=2,
                    poll_interval=0,
                )

        self.assertEqual(status, "recovered")
        command = run.call_args.args[0]
        self.assertEqual(command[:2], ["cargo", "publish"])
        self.assertIn("--manifest-path", command)
        self.assertNotIn("--token", command)

    def test_registry_server_error_does_not_trigger_upload(self):
        error = urllib.error.HTTPError(
            "https://example.invalid", 503, "unavailable", {}, None
        )
        with tempfile.TemporaryDirectory() as root:
            artifact, _ = self.artifact(root)
            with mock.patch.object(
                publish_crate.request, "urlopen", side_effect=error
            ), mock.patch.object(publish_crate.subprocess, "run") as run:
                with self.assertRaisesRegex(publish_crate.PublishError, "503"):
                    self.ensure_published(
                        "rwml", "0.1.1", artifact, None, poll_attempts=2, poll_interval=0
                    )

        run.assert_not_called()

    def test_check_only_absent_version_does_not_upload(self):
        with tempfile.TemporaryDirectory() as root:
            artifact, _ = self.artifact(root)
            with mock.patch.object(
                publish_crate.request, "urlopen", side_effect=missing_version()
            ), mock.patch.object(publish_crate.subprocess, "run") as run:
                status = self.ensure_published(
                    "rwml",
                    "0.1.1",
                    artifact,
                    None,
                    poll_attempts=2,
                    poll_interval=0,
                    check_only=True,
                )

        self.assertEqual(status, "not-published")
        run.assert_not_called()

    def test_different_checksum_after_upload_is_fatal(self):
        with tempfile.TemporaryDirectory() as root:
            artifact, _ = self.artifact(root)
            responses = [missing_version(), self.response("0" * 64)]
            with mock.patch.object(
                publish_crate.request, "urlopen", side_effect=responses
            ), mock.patch.object(
                publish_crate.subprocess,
                "run",
                return_value=subprocess.CompletedProcess([], 0),
            ):
                with self.assertRaisesRegex(publish_crate.PublishError, "checksum"):
                    self.ensure_published(
                        "rwml", "0.1.1", artifact, None, poll_attempts=1, poll_interval=0
                    )

    def test_failed_publish_remaining_absent_is_fatal(self):
        with tempfile.TemporaryDirectory() as root:
            artifact, _ = self.artifact(root)
            responses = [missing_version(), missing_version(), missing_version()]
            with mock.patch.object(
                publish_crate.request, "urlopen", side_effect=responses
            ), mock.patch.object(
                publish_crate.subprocess,
                "run",
                return_value=subprocess.CompletedProcess([], 101),
            ), mock.patch.object(publish_crate.time, "sleep"):
                with self.assertRaisesRegex(
                    publish_crate.PublishError, "cargo publish exited with 101"
                ):
                    self.ensure_published(
                        "rwml", "0.1.1", artifact, None, poll_attempts=2, poll_interval=0
                    )

    def test_successful_publish_remaining_absent_is_fatal(self):
        with tempfile.TemporaryDirectory() as root:
            artifact, _ = self.artifact(root)
            responses = [missing_version(), missing_version(), missing_version()]
            with mock.patch.object(
                publish_crate.request, "urlopen", side_effect=responses
            ), mock.patch.object(
                publish_crate.subprocess,
                "run",
                return_value=subprocess.CompletedProcess([], 0),
            ), mock.patch.object(publish_crate.time, "sleep"):
                with self.assertRaisesRegex(publish_crate.PublishError, "did not become visible"):
                    self.ensure_published(
                        "rwml", "0.1.1", artifact, None, poll_attempts=2, poll_interval=0
                    )


if __name__ == "__main__":
    unittest.main()
