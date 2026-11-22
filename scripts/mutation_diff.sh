#!/usr/bin/env bash
# Mutation testing on the code a branch changed (docs/agents/mutation.md).
#
#   bash scripts/mutation_diff.sh [base] [report]
#
# base:   what the branch is compared with (default origin/main).
# report: where the markdown report goes (default reports/mutation/report.md).
#
# Rust: cargo-mutants mutates only the lines the diff touches in src-tauri/src.
# TypeScript: Stryker mutates the src/ files the branch added or changed.
#
# Information, not a gate: a surviving mutant never fails this script. It
# fails only when a tool cannot run. cargo-mutants runs in place, so commit
# first: it restores each file after its mutant, but an interrupted run can
# leave one behind (`git diff` shows it).
set -euo pipefail

base="${1:-origin/main}"
report="${2:-reports/mutation/report.md}"
root="$(git rev-parse --show-toplevel)"
cd "$root"
mkdir -p "$(dirname "$report")"
report="$(cd "$(dirname "$report")" && pwd)/$(basename "$report")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failed=0

rust_diff="$work/rust.diff"
git diff --relative=src-tauri "$base"...HEAD -- src-tauri/src >"$rust_diff"
ts_files="$(git diff --no-renames --diff-filter=AM --name-only "$base"...HEAD -- 'src/*.ts' 'src/*.tsx' \
  | grep -v -E '(^src/test/|\.test\.tsx?$|\.d\.ts$)' || true)"

{
  echo "## Mutation testing"
  echo
  echo "Against \`$base\`. A survivor is a change no test noticed. Kill it with a test, or say in the PR why it does not matter."
  echo
} >"$report"

# --- Rust ---------------------------------------------------------------
echo "### Rust (cargo-mutants, changed lines)" >>"$report"
echo >>"$report"
if [ ! -s "$rust_diff" ]; then
  echo "No change in \`src-tauri/src\`." >>"$report"
elif ! cargo mutants --version >/dev/null 2>&1; then
  echo "cargo-mutants is not installed: \`cargo install --locked cargo-mutants\`." >>"$report"
  failed=1
else
  status=0
  (cd src-tauri && cargo mutants --in-place --in-diff "$rust_diff" --no-shuffle \
    --output "$work" -- --lib) >"$work/cargo-mutants.log" 2>&1 || status=$?
  out="$work/mutants.out"
  # 0: all caught, 2: some missed, 3: some timed out. Anything else: it did not run.
  if [ -f "$out/outcomes.json" ] && { [ "$status" -eq 0 ] || [ "$status" -eq 2 ] || [ "$status" -eq 3 ]; }; then
    # grep -c prints 0 and exits 1 on an empty list; a missing list is 0 too.
    n() { c=0; [ -f "$out/$1.txt" ] && c=$(grep -c . "$out/$1.txt" || true); echo "${c:-0}"; }
    echo "Caught $(n caught), missed $(n missed), timeout $(n timeout), unviable $(n unviable)." >>"$report"
    if [ "$(n missed)" -gt 0 ]; then
      echo >>"$report"
      head -n 30 "$out/missed.txt" | sed -E 's/^/- `/; s/$/`/' >>"$report"
      [ "$(n missed)" -gt 30 ] && echo "- the first 30 of $(n missed)" >>"$report"
    fi
  elif grep -q "No mutants to filter\|No mutants found" "$work/cargo-mutants.log"; then
    echo "The changed lines hold no mutable code." >>"$report"
  else
    failed=1
    {
      echo "cargo-mutants did not run (exit $status). The end of its log:"
      echo
      echo '```'
      tail -n 30 "$work/cargo-mutants.log"
      echo '```'
    } >>"$report"
  fi
fi
echo >>"$report"

# --- TypeScript ---------------------------------------------------------
echo "### TypeScript (Stryker, changed files)" >>"$report"
echo >>"$report"
if [ -z "$ts_files" ]; then
  echo "No change in a \`src/\` module." >>"$report"
else
  json="reports/mutation/mutation.json"
  rm -f "$json"
  status=0
  npx stryker run --mutate "$(printf '%s' "$ts_files" | paste -sd, -)" >"$work/stryker.log" 2>&1 || status=$?
  if [ "$status" -eq 0 ] && [ -f "$json" ]; then
    node - "$json" >>"$report" <<'EOF'
const report = JSON.parse(require("node:fs").readFileSync(process.argv[2], "utf8"));
const counts = {};
const left = [];
for (const [file, { mutants }] of Object.entries(report.files)) {
  for (const m of mutants) {
    counts[m.status] = (counts[m.status] ?? 0) + 1;
    if (m.status === "Survived" || m.status === "NoCoverage") {
      left.push(`- \`${file}:${m.location.start.line}\` ${m.mutatorName}${m.status === "NoCoverage" ? " (no test runs it)" : ""}: \`${(m.replacement ?? "").replace(/\s+/g, " ").slice(0, 80)}\``);
    }
  }
}
const c = (k) => counts[k] ?? 0;
console.log(`Killed ${c("Killed")}, survived ${c("Survived")}, no coverage ${c("NoCoverage")}, timeout ${c("Timeout")}.`);
if (left.length) {
  console.log();
  console.log(left.slice(0, 30).join("\n"));
  if (left.length > 30) console.log(`- the first 30 of ${left.length}`);
}
EOF
  else
    failed=1
    {
      echo "Stryker did not run (exit $status). The end of its log:"
      echo
      echo '```'
      tail -n 30 "$work/stryker.log"
      echo '```'
    } >>"$report"
  fi
fi

cat "$report"
exit "$failed"
