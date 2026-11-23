"""Proves the glossary avoid-term check (DL-01b) still goes red on a bad tree.

Run: python -m unittest discover -s scripts -p "test_check_glossary_terms.py"
"""

import contextlib
import io
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_glossary_terms as check

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
        for text in (
            "a prompt chain",
            "prompt_chain = 1",
            "promptChains",
            "prompt-chain",
            "the prompt\n   chain",
        ):
            self.assertTrue(self.find("prompt chain", text), text)

    def test_whole_words_only(self):
        self.assertFalse(self.find("tenant", "subtenantx"))
        self.assertFalse(self.find("tenant", "tenantship"))

    def test_capitals_are_case_sensitive(self):
        self.assertTrue(self.find("SOP", "one SOP per recording"))
        self.assertTrue(self.find("SOP", "all SOPs"))
        self.assertFalse(self.find("SOP", "SOPDocument"))
        self.assertFalse(self.find("SOP", "generate_sop_steps"))

    def test_a_defined_terms_code_name_is_not_its_avoided_synonym(self):
        pattern = check.term_pattern("SOP document", spaced_only=True)
        self.assertTrue(pattern.search("the SOP document"))
        self.assertFalse(pattern.search("sop_document = build()"))
        self.assertFalse(pattern.search("SOPDocument"))
        self.assertEqual(check.defined_terms("**SOPDocument**:\nx\n"), {"sopdocument"})

    def test_qualified_entries_are_skipped(self):
        terms, skipped = check.parse_glossary(
            "_Avoid_: event, click (for the step itself)\n_Avoid_: `tenant`, workspace\n"
        )
        self.assertEqual(terms, ["event", "tenant", "workspace"])
        self.assertEqual(skipped, ["click (for the step itself)"])

    def test_allowed_phrases_do_not_count(self):
        text = check.blank_allowed("SOP Sorcery makes guides", ["SOP Sorcery"])
        self.assertFalse(self.find("SOP", text))


class CodeNames(unittest.TestCase):
    def test_a_defined_name_in_capitals_is_its_code_name(self):
        pattern = check.code_name_pattern(["SOPDocument", "Guide", "Legacy bridge"])
        text = check.blank_allowed(
            "ENABLE_SOP_DOCUMENT_OUTPUTS and sop_document_id", [], pattern
        )
        self.assertFalse(check.term_pattern("SOP").search(text))

    def test_a_bare_avoided_term_still_counts(self):
        pattern = check.code_name_pattern(["SOPDocument"])
        text = check.blank_allowed("PARENT_SOP_ID and one SOP", [], pattern)
        self.assertEqual(len(check.term_pattern("SOP").findall(text)), 2)


class IssueText(unittest.TestCase):
    """`--text`: the Issue glossary workflow's check (DL-04a)."""

    def check_text(self, text):
        path = Path(self.tmp.name) / "issue.md"
        path.write_text(text, encoding="utf-8")
        return run("--root", str(FIXTURE), "--text", str(path))

    def setUp(self):
        import tempfile

        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)

    def test_an_avoided_term_in_prose_goes_red_with_its_line(self):
        code, out = self.check_text("Spec: tenant keys\n\nEach tenant gets a key.\n")
        self.assertEqual(code, 1, out)
        self.assertIn('line 1,3: "tenant"', out)

    def test_quoted_code_does_not_count(self):
        text = "Rename `prompt_chain` to the new name.\n\n```\nprompt_chain = 1\n```\n"
        code, out = self.check_text(text)
        self.assertEqual(code, 0, out)

    def test_an_unclosed_fence_blanks_to_the_end(self):
        code, out = self.check_text("Log:\n```\ntenant=1\n")
        self.assertEqual(code, 0, out)

    def test_blanking_keeps_line_numbers(self):
        text = "```\na\nb\n```\ntenant\n"
        self.assertEqual(check.blank_code(text).count("\n"), text.count("\n"))


class ThisRepo(unittest.TestCase):
    def test_repo_has_no_new_avoided_terms(self):
        code, out = run()
        self.assertEqual(code, 0, out)


if __name__ == "__main__":
    unittest.main()
