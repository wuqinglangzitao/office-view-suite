import pathlib
import unittest


WORKFLOW = (
    pathlib.Path(__file__).resolve().parents[1]
    / ".github"
    / "workflows"
    / "fuzz.yml"
)


class FuzzWorkflowTests(unittest.TestCase):
    def test_fuzz_targets_use_public_synthetic_seed_corpus(self):
        text = WORKFLOW.read_text(encoding="utf-8")

        for target in ("parse", "edit", "render"):
            self.assertIn(
                f"cargo +nightly fuzz run {target} corpus/public/synthetic --",
                text,
            )


if __name__ == "__main__":
    unittest.main()
