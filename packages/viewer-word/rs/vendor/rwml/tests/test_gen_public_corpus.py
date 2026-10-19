import importlib.util
import io
import pathlib
import sys
import tempfile
import unittest
import zipfile
from contextlib import redirect_stderr, redirect_stdout
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "gen_public_corpus.py"
SPEC = importlib.util.spec_from_file_location("gen_public_corpus", SCRIPT)
gen_public_corpus = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(gen_public_corpus)


def manifest_paths(path: pathlib.Path) -> set[str]:
    return {
        line.split("\t", 1)[0]
        for line in path.read_text(encoding="utf-8").splitlines()
        if line and not line.startswith("#") and not line.startswith("path\t")
    }


class PublicCorpusGeneratorTests(unittest.TestCase):
    def run_generator(self, out_dir: pathlib.Path, *args: str) -> int:
        with mock.patch.object(gen_public_corpus, "OUT_DIR", str(out_dir)):
            with mock.patch.object(sys, "argv", [str(SCRIPT), *args]):
                with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                    return gen_public_corpus.main()

    def test_check_is_no_write_and_detects_missing_or_stale_outputs(self):
        with tempfile.TemporaryDirectory() as tmp:
            out_dir = pathlib.Path(tmp) / "synthetic"

            self.assertEqual(self.run_generator(out_dir, "--check"), 1)
            self.assertFalse(out_dir.exists())

            self.assertEqual(self.run_generator(out_dir), 0)
            stale = out_dir / "comments.docx"
            stale.write_bytes(b"stale")
            self.assertEqual(self.run_generator(out_dir, "--check"), 1)
            self.assertEqual(stale.read_bytes(), b"stale")

    def test_every_public_docx_is_manifested_or_attributed(self):
        vendored = {
            "vendored/python-docx/blk-inner-content.docx",
            "vendored/python-docx/having-images.docx",
            "vendored/python-docx/sct-inner-content.docx",
            "vendored/python-docx/test.docx",
        }
        expected = {f"synthetic/{name}" for name in gen_public_corpus.CORPUS} | vendored
        self.assertEqual(manifest_paths(ROOT / "corpus/public/MANIFEST.tsv"), expected)
        self.assertEqual(
            manifest_paths(ROOT / "corpus/public/RENDER_MANIFEST.tsv"), expected
        )

        corpus_root = ROOT / "corpus/public"
        observed = {
            path.relative_to(corpus_root).as_posix()
            for path in corpus_root.rglob("*.docx")
        }
        attribution = (corpus_root / "ATTRIBUTION.md").read_text(encoding="utf-8")
        provenance = (corpus_root / "PROVENANCE.md").read_text(encoding="utf-8")
        for name in gen_public_corpus.CORPUS:
            self.assertIn(f"`synthetic/{name}`", provenance)
        for relative in vendored:
            self.assertIn(f"`{pathlib.PurePosixPath(relative).name}`", attribution)

    def test_render_activation_fixtures_are_deterministic_and_cover_target_markup(self):
        expected_markup = {
            "style-hidden-tabs-table.docx": (
                "<w:highlight",
                "<w:vertAlign",
                "<w:caps",
                "<w:smallCaps",
                "<w:strike",
                "<w:vanish",
                'xml:space="preserve"',
                "<w:tabs",
                "<w:bidiVisual",
                "<w:tblCellMar",
                "<w:vAlign",
                "w:hanging=",
                "<w:shd",
            ),
            "pagination-keep.docx": (
                "<w:keepNext",
                "<w:keepLines",
                "<w:widowControl",
                '<w:pStyle w:val="KeepGroup"',
                '<w:pgSz w:w="7200" w:h="5000"',
            ),
            "two-columns.docx": ('<w:cols w:num="2"',),
            "rtl-table.docx": (
                "<w:bidi",
                "<w:rtl",
                "<w:bidiVisual",
                "<w:tblCellMar",
            ),
            "table-cell-lists.docx": (
                "<w:numPr",
                "<w:bidi",
                "<w:rtl",
                "<w:bidiVisual",
                "Nested bullet level",
                "<w:tc><w:tbl>",
            ),
            "wrap-top-bottom.docx": (
                "<wp:anchor",
                "<wp:positionH",
                "<wp:positionV",
                "<wp:wrapTopAndBottom",
            ),
        }

        self.assertLessEqual(expected_markup.keys(), gen_public_corpus.CORPUS.keys())
        for name, markers in expected_markup.items():
            with self.subTest(name=name):
                first = gen_public_corpus.CORPUS[name]()
                self.assertEqual(first, gen_public_corpus.CORPUS[name]())
                with zipfile.ZipFile(io.BytesIO(first)) as archive:
                    self.assertTrue(archive.infolist())
                    self.assertTrue(
                        all(info.create_system == 3 for info in archive.infolist())
                    )
                    document_xml = archive.read("word/document.xml").decode("utf-8")
                    if name == "pagination-keep.docx":
                        styles_xml = archive.read("word/styles.xml").decode("utf-8")
                        self.assertIn('<w:style w:type="paragraph"', styles_xml)
                        self.assertIn("<w:keepNext", styles_xml)
                        self.assertIn("<w:keepLines", styles_xml)
                    if name == "table-cell-lists.docx":
                        numbering_xml = archive.read("word/numbering.xml").decode(
                            "utf-8"
                        )
                        self.assertIn('<w:start w:val="4"', numbering_xml)
                        self.assertIn('<w:numFmt w:val="decimal"', numbering_xml)
                        self.assertIn('<w:numFmt w:val="bullet"', numbering_xml)
                for marker in markers:
                    self.assertIn(marker, document_xml)


if __name__ == "__main__":
    unittest.main()
