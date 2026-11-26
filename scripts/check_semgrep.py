"""Run the repo's custom Semgrep rules (.semgrep/) and prove they still work.

Three assertions, each a failure on its own:
  1. The bad fixture is red: every line marked `// <n>` is flagged, and
     nothing else.
  2. The good fixture is green: no finding.
  3. src/ and src-tauri/src/ are green. The Rust scan is the module
     boundary check (CODING_STANDARDS.md R6) and prints its own count line.
Every scan must also have scanned at least one file: a check that scanned
nothing proves nothing.

Usage:
  python scripts/check_semgrep.py            run the check, from any directory
  python scripts/check_semgrep.py --version  print the pinned Semgrep version (CI installs it)

It uses an installed `semgrep` only at the pinned version, else `uvx` fetches it.
"""

import json
import os
import re
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CONFIG = ".semgrep"
FIXTURES = os.path.join(".semgrep", "fixtures")
SEMGREP_VERSION = "1.186.0"


def installed_version():
    try:
        out = subprocess.run(["semgrep", "--version"], capture_output=True, text=True)
    except OSError:
        return None
    return out.stdout.strip()


def semgrep_cmd():
    if shutil.which("semgrep") and installed_version() == SEMGREP_VERSION:
        return ["semgrep"]
    if shutil.which("uvx"):
        return ["uvx", "--quiet", f"semgrep=={SEMGREP_VERSION}"]
    sys.exit(f"check_semgrep: needs semgrep {SEMGREP_VERSION} or uvx")


def scan(target):
    """Return (findings as (path, line), number of files scanned)."""
    cmd = semgrep_cmd() + [
        "scan", "--config", CONFIG, "--json", "--metrics=off",
        "--disable-version-check", "--no-git-ignore", target,
    ]
    env = dict(os.environ, PYTHONUTF8="1")
    out = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", env=env)
    try:
        data = json.loads(out.stdout)
    except json.JSONDecodeError:
        sys.exit(f"check_semgrep: semgrep gave no JSON for {target}:\n{out.stderr}")
    if data.get("errors"):
        sys.exit(f"check_semgrep: semgrep errors on {target}: {data['errors']}")
    if not data["paths"]["scanned"]:
        sys.exit(f"check_semgrep: semgrep scanned 0 files in {target}")
    found = [(os.path.normpath(r["path"]), r["start"]["line"]) for r in data["results"]]
    return found, len(data["paths"]["scanned"])


def marked_lines(path):
    with open(path, encoding="utf-8") as f:
        return [i for i, line in enumerate(f, 1) if re.search(r"// \d+$", line.rstrip())]


def main():
    if sys.argv[1:] == ["--version"]:
        print(SEMGREP_VERSION)
        return
    os.chdir(ROOT)
    failed = False
    for name in sorted(os.listdir(FIXTURES)):
        path = os.path.normpath(os.path.join(FIXTURES, name))
        found, _ = scan(path)
        got = sorted(line for _, line in found)
        if "-bad." in name:
            want = marked_lines(path)
            ok = got == want and want
            print(f"{path}: {len(got)} finding(s), {len(want)} marked {'OK' if ok else 'MISMATCH'}")
            if not ok:
                print(f"  flagged lines {got}, marked lines {want}")
                failed = True
        elif "-ok." in name:
            print(f"{path}: {len(got)} finding(s) {'OK' if not got else 'FAIL'}")
            failed |= bool(got)

    found, scanned = scan("src")
    print(f"src: scanned {scanned} file(s), {len(found)} finding(s)")
    for path, line in found:
        print(f"  {path}:{line}")
    if found:
        failed = True

    found, scanned = scan(os.path.join("src-tauri", "src"))
    print(f"boundaries: checked {scanned} files, {len(found)} finding(s)")
    for path, line in found:
        print(f"  {path}:{line}")
    if found:
        failed = True
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
