"""Fail when code or docs use a term that GLOSSARY.md lists under `_Avoid_:`.

Usage:
  python scripts/check_glossary_terms.py             check this repo against the baseline
  python scripts/check_glossary_terms.py --update    rewrite the baseline from today's hits
  python scripts/check_glossary_terms.py --root DIR  check another tree (the bad fixture)

An `_Avoid_:` line is a comma-separated list. An entry with a qualifier in
parentheses ("click (for the step itself)") depends on context, so it is
skipped and counted as skipped.

A term matches as a whole phrase in any case, plural included, with its words
joined by a space, `_`, `-` or nothing: "prompt chain" finds prompt_chain and
promptChains. An all-capitals term ("SOP") matches case-sensitively, so
SOPDocument does not count. Phrases listed one per line in
`scripts/glossary_terms_allowed.txt` never count (names such as "SOP Sorcery").

Hits that already exist are recorded per file and term in
`scripts/glossary_terms_baseline.json`. The check fails when a file has more
hits of a term than its baseline allows, so new uses are blocked while old ones
wait for their rename. After a rename, `--update` shrinks the baseline.
"""

import json
import re
import subprocess
import sys
from pathlib import Path

BASELINE = "scripts/glossary_terms_baseline.json"
ALLOWED = "scripts/glossary_terms_allowed.txt"
# Not checked: the glossary itself, this check's own files, history that is
# never rewritten, vendored upstream skills, lockfiles.
EXCLUDE_PREFIXES = (
    "GLOSSARY.md",
    "CHANGELOG.md",
    "docs/changelog/",
    "scripts/check_glossary_terms.py",
    "scripts/test_check_glossary_terms.py",
    "scripts/glossary_terms_",
    "scripts/glossary_fixture/",
    "db/migrations/",
    ".claude/skills/",
    "node_modules/",
)
EXCLUDE_NAMES = ("package-lock.json", "Cargo.lock", "uv.lock", "pnpm-lock.yaml", "yarn.lock")
MAX_BYTES = 1_000_000


def parse_glossary(text):
    """Return (terms, skipped): greppable avoid terms and qualified entries."""
    terms, skipped = [], []
    for line in text.splitlines():
        stripped = line.strip()
        if not stripped.startswith("_Avoid_:"):
            continue
        for entry in stripped[len("_Avoid_:"):].split(","):
            entry = entry.strip().strip("`\"'").strip()
            if not entry:
                continue
            if "(" in entry:
                skipped.append(entry)
            elif entry not in terms:
                terms.append(entry)
    return terms, skipped


def term_pattern(term):
    words = term.split()
    if term.isupper():
        body = r"(?:\s+|[_-])?".join(re.escape(w) for w in words) + r"s?"
        return re.compile(r"(?<![A-Za-z0-9])" + body + r"(?![A-Za-z0-9])")
    body = r"(?:\s+|[_-])?".join(re.escape(w) for w in words) + r"(?:e?s)?"
    start = r"(?:(?<![A-Za-z0-9])|(?<=[a-z])(?=[A-Z]))"
    return re.compile(start + r"(?i:" + body + r")(?![a-z0-9])")


def blank_allowed(text, allowed):
    for phrase in allowed:
        text = re.sub(re.escape(phrase), lambda m: " " * len(m.group(0)), text, flags=re.I)
    return text


def list_files(root):
    out = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=root, capture_output=True, encoding="utf-8", check=True,
    ).stdout
    files = []
    for rel in out.splitlines():
        if rel.startswith(EXCLUDE_PREFIXES) or Path(rel).name in EXCLUDE_NAMES:
            continue
        path = root / rel
        if path.is_file() and path.stat().st_size <= MAX_BYTES:
            files.append(rel)
    return files


def read_text(path):
    try:
        return path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        return None


def scan(root):
    """Return (hits, checked, terms, skipped); hits maps path -> term -> [line numbers]."""
    glossary = read_text(root / "GLOSSARY.md")
    if glossary is None:
        raise SystemExit(f"no GLOSSARY.md in {root}")
    terms, skipped = parse_glossary(glossary)
    allowed_file = read_text(root / ALLOWED) or ""
    allowed = [p.strip() for p in allowed_file.splitlines() if p.strip() and not p.startswith("#")]
    patterns = [(t, term_pattern(t)) for t in terms]
    hits, checked = {}, 0
    for rel in list_files(root):
        text = read_text(root / rel)
        if text is None:
            continue
        checked += 1
        text = blank_allowed(text, allowed)
        for term, pattern in patterns:
            lines = [text.count("\n", 0, m.start()) + 1 for m in pattern.finditer(text)]
            if lines:
                hits.setdefault(rel, {})[term] = lines
    return hits, checked, terms, skipped


def main(argv):
    if argv and argv[0] in ("-h", "--help"):
        print(__doc__.strip())
        return 0
    update = "--update" in argv
    root = Path(argv[argv.index("--root") + 1]) if "--root" in argv else Path(__file__).resolve().parent.parent
    root = root.resolve()
    hits, checked, terms, skipped = scan(root)
    print(f"glossary terms: checked {checked} files for {len(terms)} avoid terms "
          f"({len(skipped)} qualified entries skipped)")
    if checked == 0 or not terms:
        print("FAIL: nothing was checked")
        return 1
    counts = {rel: {t: len(lines) for t, lines in by_term.items()} for rel, by_term in hits.items()}
    baseline_path = root / BASELINE
    if update:
        baseline_path.write_text(json.dumps(counts, indent=2, sort_keys=True) + "\n",
                                 encoding="utf-8", newline="\n")
        print(f"baseline written: {sum(sum(c.values()) for c in counts.values())} hits in {len(counts)} files")
        return 0
    baseline = json.loads(read_text(baseline_path) or "{}")
    new, shrunk = [], 0
    for rel, by_term in sorted(hits.items()):
        for term, lines in sorted(by_term.items()):
            allowed_count = baseline.get(rel, {}).get(term, 0)
            if len(lines) > allowed_count:
                new.append(f"{rel}:{','.join(map(str, lines))}: \"{term}\" "
                           f"({len(lines)} hits, baseline {allowed_count})")
    for rel, by_term in baseline.items():
        for term, count in by_term.items():
            if len(hits.get(rel, {}).get(term, [])) < count:
                shrunk += 1
    if shrunk:
        print(f"note: {shrunk} baseline entries are above today's count; run --update to shrink them")
    if new:
        print("FAIL: use the GLOSSARY.md term instead of these avoided ones:")
        print("\n".join("  " + n for n in new))
        return 1
    print("OK: no new avoided terms")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
