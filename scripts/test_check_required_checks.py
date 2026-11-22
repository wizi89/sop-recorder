"""Proves the required-checks check (FL-03e) goes red on each kind of gap.

Run: python -m unittest discover -s scripts -p "test_check_required_checks.py"
"""

import contextlib
import io
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_required_checks as check

GATED = """\
name: CI
on:
  push:
    branches: [main]
  pull_request:
jobs:
  changes:
    runs-on: ubuntu-latest
  test:
    needs: changes
    strategy:
      matrix:
        os: [a, b]
    name: test (${{ matrix.os }})
  ci:
    needs: [changes, test]
    if: always()
    runs-on: ubuntu-latest
    steps:
      - env:
          RESULTS: ${{ join(needs.*.result, ' ') }}
        run: echo "$RESULTS"
"""

DIRECT = """\
name: Tests
on: [push, pull_request]
jobs:
  lint:
    name: Lint
    runs-on: ubuntu-latest
"""

NOT_PR = """\
name: Release
on:
  push:
    tags: ["v*"]
jobs:
  release:
    runs-on: ubuntu-latest
"""


def run(workflows, required):
    with tempfile.TemporaryDirectory() as tmp:
        wf = Path(tmp) / "workflows"
        wf.mkdir()
        for name, text in workflows.items():
            (wf / name).write_text(text, encoding="utf-8")
        req = Path(tmp) / "required.txt"
        req.write_text("\n".join(required) + "\n", encoding="utf-8")
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = check.main(["--required-file", str(req), "--workflows", str(wf)])
        return code, out.getvalue()


class Green(unittest.TestCase):
    def test_gate_and_direct_requirements_cover_every_job(self):
        code, out = run({"ci.yml": GATED, "tests.yml": DIRECT, "release.yml": NOT_PR}, ["ci", "Lint"])
        self.assertEqual(code, 0, out)
        self.assertIn("required checks: checked 4 PR jobs against 2 required, 0 gap(s)", out)

    def test_exempt_job_is_listed_not_flagged(self):
        wf = DIRECT.replace("  lint:\n", "  # required-checks: exempt -- labels PRs\n  lint:\n")
        code, out = run({"tests.yml": wf, "ci.yml": GATED}, ["ci"])
        self.assertEqual(code, 0, out)
        self.assertIn("job lint: exempt (labels PRs)", out)


class Red(unittest.TestCase):
    def test_job_that_is_not_required(self):
        code, out = run({"tests.yml": DIRECT}, ["Something else"])
        self.assertEqual(code, 1, out)
        self.assertIn('GAP tests.yml: job lint: "Lint" is not a required check', out)

    def test_need_of_a_required_job_that_is_not_a_gate(self):
        # A failed `changes` skips `test`, and skipped counts as passed.
        wf = GATED.replace("    if: always()\n", "")
        code, out = run({"ci.yml": wf}, ["ci"])
        self.assertEqual(code, 1, out)
        self.assertIn("GAP ci.yml: job changes:", out)

    def test_matrix_job_required_by_name(self):
        wf = GATED.replace("    needs: [changes, test]\n", "    needs: [changes]\n")
        code, out = run({"ci.yml": wf}, ["ci", "test (a)", "test (b)"])
        self.assertEqual(code, 1, out)
        self.assertIn("GAP ci.yml: job test: its check name depends on the matrix", out)

    def test_pr_trigger_filtered_by_branches(self):
        wf = GATED.replace("  pull_request:\n", "  pull_request:\n    branches: [main]\n")
        code, out = run({"ci.yml": wf}, ["ci"])
        self.assertEqual(code, 1, out)
        self.assertIn("GAP ci.yml: the PR trigger is filtered by branches:", out)

    def test_no_required_checks_read(self):
        code, out = run({"ci.yml": GATED}, [])
        self.assertEqual(code, 1, out)
        self.assertIn("required checks: none read", out)

    def test_no_pr_job_found(self):
        code, out = run({"release.yml": NOT_PR}, ["ci"])
        self.assertEqual(code, 1, out)
        self.assertIn("no PR job found", out)


if __name__ == "__main__":
    unittest.main()
