#!/usr/bin/env bash
# Proves the clippy lint table in src-tauri/Cargo.toml catches what it is for
# (coverage row FL-02b), in a throwaway crate with the same [lints] table:
#   - an #[allow] without a reason fails `cargo clippy -- -D warnings`;
#   - the same #[allow] with a reason passes.
# Run: bash scripts/clippy_selftest.sh
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
mkdir -p "$tmp/src"
{
  printf '[package]\nname = "lint-fixture"\nversion = "0.0.0"\nedition = "2021"\n\n'
  # Every [lints...] table, up to the next table.
  awk '/^\[/{on = ($0 ~ /^\[lints/)} on' "$here/src-tauri/Cargo.toml"
} > "$tmp/Cargo.toml"
grep -q 'allow_attributes_without_reason' "$tmp/Cargo.toml" || {
  echo "clippy selftest FAILED: no allow_attributes_without_reason in the lint table"; exit 1; }
fail() { echo "clippy selftest FAILED: $1"; echo "$out"; exit 1; }

printf '#[allow(dead_code)]\nfn unused() {}\n' > "$tmp/src/lib.rs"
out="$(cd "$tmp" && cargo clippy --quiet -- -D warnings 2>&1)" && fail "an #[allow] without a reason passed"
echo "$out" | grep -q 'allow_attributes_without_reason' || fail "clippy failed, but not on the missing reason"
echo "clippy selftest: an #[allow] without a reason is red, as expected"

printf '#[allow(dead_code, reason = "a fixture")]\nfn unused() {}\n' > "$tmp/src/lib.rs"
out="$(cd "$tmp" && cargo clippy --quiet -- -D warnings 2>&1)" || fail "an #[allow] with a reason failed"
echo "clippy selftest: an #[allow] with a reason passes"
