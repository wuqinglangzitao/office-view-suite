import contextlib
import importlib.util
import io
import pathlib
import sys
import tempfile
import unittest
import zipfile


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "scripts" / "validate_edit_check.py"
SPEC = importlib.util.spec_from_file_location("validate_edit_check", SCRIPT)
validate_edit_check = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = validate_edit_check
SPEC.loader.exec_module(validate_edit_check)


class LoadedDocument:
    def __init__(self, inline_shapes):
        self.inline_shapes = inline_shapes


class ValidateEditCheckTests(unittest.TestCase):
    @staticmethod
    def write_package(path, payload=b"document"):
        path.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(path, "w") as archive:
            archive.writestr("[Content_Types].xml", b"types")
            archive.writestr("word/document.xml", payload)

    def make_case(self, root):
        input_dir = root / "input"
        output_dir = root / "output"
        relative = pathlib.Path("synthetic") / "alpha.docx"
        self.write_package(input_dir / relative)
        (input_dir / "MANIFEST.tsv").write_text(
            "# path\twarnings\nsynthetic/alpha.docx\t-\n", encoding="utf-8"
        )
        self.write_package(output_dir / "pass" / relative)
        self.write_package(output_dir / "bimg" / relative, payload=b"edited")
        return input_dir, output_dir, relative

    @staticmethod
    def loader(path):
        images = [object()] if "bimg" in path.parts else []
        return LoadedDocument(images)

    @staticmethod
    def run_main(input_dir, output_dir, loader):
        stdout = io.StringIO()
        stderr = io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            status = validate_edit_check.main(
                [str(input_dir), str(output_dir)], document_loader=loader
            )
        return status, stdout.getvalue(), stderr.getvalue()

    def test_public_manifest_contains_exactly_twenty_one_documents(self):
        public = pathlib.Path(__file__).resolve().parents[1] / "corpus" / "public"

        self.assertEqual(len(validate_edit_check.expected_docx(public)), 21)

    def test_complete_outputs_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            input_dir, output_dir, _ = self.make_case(pathlib.Path(tmp))

            status, stdout, stderr = self.run_main(
                input_dir, output_dir, self.loader
            )

            self.assertEqual(status, 0)
            self.assertIn("expected=1", stdout)
            self.assertEqual(stderr, "")

    def test_zero_input_is_an_error(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            input_dir = root / "input"
            input_dir.mkdir()

            status, _, stderr = self.run_main(
                input_dir, root / "output", self.loader
            )

            self.assertEqual(status, 2)
            self.assertIn("no DOCX inputs", stderr)

    def test_missing_output_is_fail_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            input_dir, output_dir, relative = self.make_case(pathlib.Path(tmp))
            (output_dir / "pass" / relative).unlink()

            status, _, stderr = self.run_main(
                input_dir, output_dir, self.loader
            )

            self.assertEqual(status, 1)
            self.assertIn("PASS-MISSING", stderr)

    def test_part_drift_is_fail_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            input_dir, output_dir, relative = self.make_case(pathlib.Path(tmp))
            self.write_package(output_dir / "pass" / relative, payload=b"drift")

            status, _, stderr = self.run_main(
                input_dir, output_dir, self.loader
            )

            self.assertEqual(status, 1)
            self.assertIn("PASS-DRIFT", stderr)

    def test_python_docx_open_failure_is_fail_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            input_dir, output_dir, _ = self.make_case(pathlib.Path(tmp))

            def loader(path):
                if "pass" in path.parts:
                    raise ValueError("cannot open")
                return self.loader(path)

            status, _, stderr = self.run_main(input_dir, output_dir, loader)

            self.assertEqual(status, 1)
            self.assertIn("PASS-OPEN-FAIL", stderr)

    def test_missing_inline_image_is_fail_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            input_dir, output_dir, _ = self.make_case(pathlib.Path(tmp))

            status, _, stderr = self.run_main(
                input_dir,
                output_dir,
                lambda _path: LoadedDocument([]),
            )

            self.assertEqual(status, 1)
            self.assertIn("BIMG-NO-IMAGE", stderr)

    def test_unexpected_output_is_fail_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            input_dir, output_dir, _ = self.make_case(pathlib.Path(tmp))
            self.write_package(output_dir / "pass" / "synthetic" / "extra.docx")

            status, _, stderr = self.run_main(
                input_dir, output_dir, self.loader
            )

            self.assertEqual(status, 1)
            self.assertIn("PASS-UNEXPECTED", stderr)


if __name__ == "__main__":
    unittest.main()
