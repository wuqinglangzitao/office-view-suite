import importlib.util
import pathlib
import sys
import unittest


SCRIPT = pathlib.Path(__file__).resolve().parents[1] / "scripts" / "release_preflight.py"
SPEC = importlib.util.spec_from_file_location("release_preflight", SCRIPT)
release_preflight = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = release_preflight
SPEC.loader.exec_module(release_preflight)


class ReleasePreflightTests(unittest.TestCase):
    def test_venv_interpreter_is_platform_safe(self):
        venv = pathlib.Path("target") / "release-preflight" / "python-tools"

        self.assertEqual(
            release_preflight.venv_interpreter(venv, platform_name="nt"),
            venv / "Scripts" / "python.exe",
        )
        self.assertEqual(
            release_preflight.venv_interpreter(venv, platform_name="posix"),
            venv / "bin" / "python",
        )

    def test_release_extract_binary_uses_preflight_target_and_platform_suffix(self):
        target = pathlib.Path("target") / "release-preflight" / "cargo-target"

        self.assertEqual(
            release_preflight.cargo_example_binary(
                target, "extract", platform_name="nt"
            ),
            target / "release" / "examples" / "extract.exe",
        )
        self.assertEqual(
            release_preflight.cargo_example_binary(
                target, "extract", platform_name="posix"
            ),
            target / "release" / "examples" / "extract",
        )

    def test_preflight_uses_shared_pinned_environment_for_tests_and_validation(self):
        text = SCRIPT.read_text(encoding="utf-8")

        self.assertIn("validation_python = ensure_validation_tools(output_dir)", text)
        self.assertIn('"unittest",', text)
        self.assertIn('"validate_edit_check.py",', text)
        self.assertIn('"render_validate.py",', text)
        self.assertIn('"bench_vs_mature.py",', text)
        self.assertIn('"--extract-bin",', text)


if __name__ == "__main__":
    unittest.main()
