"""Fails when a job that runs on pull requests can fail without blocking the merge.

CI is the only hard gate, and only the checks that branch protection requires
block a merge. A PR job is covered when:

- its check name (the job's `name:`, else its key) is a required check; or
- it is a direct `needs:` of a required gate job that runs `if: always()` and
  fails on any `needs.*.result` other than success or skipped.

Anything else is a gap. A failed job that another job only `needs:` turns
the dependant into "skipped", and a skipped required check counts as passed.
A job whose name contains `${{ }}` (a matrix) reports one check per leg, so
it is covered only through a gate. A PR trigger filtered by `branches:`
leaves PRs into other bases (integration branches) without CI, which is a
gap too.

A job that checks nothing (a labeller, an analysis whose results go
elsewhere) is exempted by a comment directly above its key:
`# required-checks: exempt -- <reason>`. A PR that adds one, in any
workflow, gets the One-way Door Guardrail box, so the owner sees it.

Run: gh api repos/<owner>/<repo>/branches/<default> \
       --jq '.protection.required_status_checks.contexts[]' > required.txt
     python3 scripts/check_required_checks.py --required-file required.txt

Standard library only: the workflow subset GitHub uses is parsed by indent.
"""

import argparse
import re
import sys
from pathlib import Path

PR_EVENTS = ("pull_request", "pull_request_target")
KEY = re.compile(r"^(\s*)([\"']?)([A-Za-z0-9_.-]+)\2:\s*(.*?)\s*$")
EXEMPT = re.compile(r"#\s*required-checks:\s*exempt\b\s*(?:--\s*)?(.*)")


def indent(line):
    return len(line) - len(line.lstrip(" "))


def strip_comment(value):
    if value.startswith("#"):
        return ""
    return re.sub(r"\s+#.*$", "", value)


def unquote(value):
    value = value.strip()
    if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
        return value[1:-1]
    return value


def inline_list(value):
    value = value.strip()
    if value.startswith("[") and value.endswith("]"):
        return [unquote(v) for v in value[1:-1].split(",") if v.strip()]
    return [unquote(value)] if value else []


def block(lines, start, parent_indent):
    """The lines after `start` that are indented deeper than the parent key."""
    out = []
    for line in lines[start + 1 :]:
        if line.strip() and not line.lstrip().startswith("#") and indent(line) <= parent_indent:
            break
        out.append(line)
    return out


def child_keys(lines, parent_indent):
    """(index, key, value) of the direct children in a block."""
    child = None
    found = []
    for i, line in enumerate(lines):
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        m = KEY.match(line)
        if not m:
            continue
        ind = len(m.group(1))
        if child is None and ind > parent_indent:
            child = ind
        if ind == child:
            found.append((i, m.group(3), strip_comment(m.group(4))))
    return found


def top_level(lines):
    return {key: (i, value) for i, key, value in child_keys(lines, -1)}


def pr_trigger(lines):
    """None if the workflow never runs on a PR, else the PR filter keys found."""
    keys = top_level(lines)
    if "on" in keys:
        i, value = keys["on"]
    elif "true" in keys:  # YAML 1.1 reads a bare `on` as true
        i, value = keys["true"]
    else:
        return None
    if value:
        events = inline_list(value)
        return [] if any(e in PR_EVENTS for e in events) else None
    body = block(lines, i, 0)
    for j, event, _ in child_keys(body, 0):
        if event in PR_EVENTS:
            ev_indent = indent(body[j])
            return [k for _, k, _ in child_keys(block(body, j, ev_indent), ev_indent)]
    return None


def jobs(lines):
    keys = top_level(lines)
    if "jobs" not in keys:
        return []
    i, _ = keys["jobs"]
    body = block(lines, i, 0)
    found = []
    for j, key, _ in child_keys(body, 0):
        job_indent = indent(body[j])
        exempt = None
        k = j - 1
        while k >= 0 and body[k].lstrip().startswith("#"):
            m = EXEMPT.search(body[k])
            if m:
                exempt = m.group(1).strip() or "no reason given"
            k -= 1
        job_lines = block(body, j, job_indent)
        fields = {}
        for f, field, value in child_keys(job_lines, job_indent):
            rest = block(job_lines, f, indent(job_lines[f]))
            fields[field] = (value, rest)
        name = unquote(fields["name"][0]) if "name" in fields else key
        needs = []
        if "needs" in fields:
            value, rest = fields["needs"]
            needs = inline_list(value) or [unquote(r.strip()[1:]) for r in rest if r.strip().startswith("- ")]
        cond = ""
        if "if" in fields:
            value, rest = fields["if"]
            cond = " ".join([value] + [r.strip() for r in rest])
        found.append(
            {
                "key": key,
                "name": name,
                "needs": needs,
                "if": cond,
                "body": "\n".join(job_lines),
                "exempt": exempt,
            }
        )
    return found


def is_gate(job):
    return "always()" in job["if"] and "needs.*.result" in job["body"]


def check(workflow_dir, required):
    problems, exempted = [], []
    checked = 0
    for path in sorted(Path(workflow_dir).glob("*.y*ml")):
        lines = path.read_text(encoding="utf-8").splitlines()
        filters = pr_trigger(lines)
        if filters is None:
            continue
        all_jobs = jobs(lines)
        if "branches" in filters and any(not j["exempt"] for j in all_jobs):
            problems.append(
                f"{path.name}: the PR trigger is filtered by branches:, so PRs into other bases get no check"
            )
        gates = [j for j in all_jobs if j["name"] in required and is_gate(j)]
        gated = {n for g in gates for n in g["needs"]}
        for job in all_jobs:
            checked += 1
            label = f"{path.name}: job {job['key']}"
            if job["exempt"]:
                exempted.append(f"{label}: exempt ({job['exempt']})")
                continue
            if job["key"] in gated:
                continue
            if "${{" in job["name"]:
                problems.append(f"{label}: its check name depends on the matrix, so only a gate job can cover it")
            elif job["name"] not in required:
                problems.append(f'{label}: "{job["name"]}" is not a required check and no required gate job needs it')
    return checked, problems, exempted


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--required-file", required=True, help="one required check name per line")
    ap.add_argument("--workflows", default=".github/workflows")
    args = ap.parse_args(argv)
    required = {
        line.strip() for line in Path(args.required_file).read_text(encoding="utf-8").splitlines() if line.strip()
    }
    if not required:
        print("required checks: none read; branch protection requires nothing, or the read failed")
        return 1
    checked, problems, exempted = check(args.workflows, required)
    for line in exempted:
        print(line)
    for line in problems:
        print(f"GAP {line}")
    print(f"required checks: checked {checked} PR jobs against {len(required)} required, {len(problems)} gap(s)")
    if checked == 0:
        print("required checks: no PR job found, so nothing was checked")
        return 1
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
