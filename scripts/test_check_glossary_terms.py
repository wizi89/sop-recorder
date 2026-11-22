"""Proves the glossary avoid-term check (DL-01b) still goes red on a bad tree.

Run: python -m unittest discover -s scripts -p "test_check_glossary_terms.py"
"""

import contextlib
import io
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_glossary_terms as check  # noqa: E402

FIXTURE = Path(__file__).resolve().parent / "glossary_fixture"


def run(*argv):
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        code = check.main(list(argv))
    return code, out.getvalue()


class BadFixture(unittest.TestCase):
    def test_fixture_goes_red_and_names_the_hit(self):
        code, out = run("--root", str(FIXTURE))
        self.assertEqual(code, 1, out)
        self.assertIn('notes.md:4: "prompt chain"', out)
        self.assertIn('"tenant"', out)

    def test_fixture_checks_files(self):
        _, out = run("--root", str(FIXTURE))
        self.assertRegex(out, r"checked [1-9]\d* files")


class Matching(unittest.TestCase):
    def find(self, term, text):
        return bool(check.term_pattern(term).search(text))

    def test_joined_words_and_plural(self):
        for text in ("a prompt chain", "prompt_chain = 1", "promptChains", "prompt-chain",
                     "the prompt\n   chain"):
            self.assertTrue(self.find("prompt chain", text), text)

    def test_whole_words_only(self):
        self.assertFalse(self.find("tenant", "subtenantx"))
        self.assertFalse(self.find("tenant", "tenantship"))

    def test_capitals_are_case_sensitive(self):
        self.assertTrue(self.find("SOP", "one SOP per recording"))
        self.assertTrue(self.find("SOP", "all SOPs"))
        self.assertFalse(self.find("SOP", "SOPDocument"))
        self.assertFalse(self.find("SOP", "generate_sop_steps"))

    def test_qualified_entries_are_skipped(self):
        terms, skipped = check.parse_glossary(
            "_Avoid_: event, click (for the step itself)\n_Avoid_: `tenant`, workspace\n")
        self.assertEqual(terms, ["event", "tenant", "workspace"])
        self.assertEqual(skipped, ["click (for the step itself)"])

    def test_allowed_phrases_do_not_count(self):
        text = check.blank_allowed("SOP Sorcery makes guides", ["SOP Sorcery"])
        self.assertFalse(self.find("SOP", text))


class ThisRepo(unittest.TestCase):
    def test_repo_has_no_new_avoided_terms(self):
        code, out = run()
        self.assertEqual(code, 0, out)


if __name__ == "__main__":
    unittest.main()
