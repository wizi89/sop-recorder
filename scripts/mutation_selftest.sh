#!/usr/bin/env bash
# Proves scripts/mutation_diff.sh catches a weak test (coverage rows FL-06a,
# CD-05c), in a throwaway repo with this repo's layout. A branch adds an age
# gate in Rust and in TypeScript. Each test checks the label of one adult and
# one child, so the mutants of the label must die. Each also leaves a gap a
# mutant must get through:
#   - TypeScript: the boundary is never asked, so `>=` -> `>` survives;
#   - Rust: no test calls `is_senior` (cargo-mutants makes no `>` mutant of
#     `>=`), so its mutants are missed.
# Needs cargo-mutants and this repo's node_modules (npm ci). Linux and macOS.
# Run: bash scripts/mutation_selftest.sh
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
cd "$tmp"
git init -q -b main && git config user.email t@t && git config user.name t
mkdir -p scripts src/lib src/test src-tauri/src
cp "$here/scripts/mutation_diff.sh" scripts/
cp "$here/stryker.config.json" .
ln -s "$here/node_modules" node_modules
printf 'node_modules\nreports/\n.stryker-tmp/\nsrc-tauri/target/\n' > .gitignore
printf '{ "name": "mutation-selftest", "private": true, "type": "module" }\n' > package.json
cat > vitest.config.ts <<'TS'
import { defineConfig } from "vitest/config";

export default defineConfig({ test: { environment: "node" } });
TS
cat > src-tauri/Cargo.toml <<'TOML'
[package]
name = "agegate"
version = "0.0.0"
edition = "2021"

[lib]
path = "src/lib.rs"
TOML
printf 'pub fn placeholder() {}\n' > src-tauri/src/lib.rs
printf 'export const placeholder = 0;\n' > src/lib/agegate.ts
git add -A && git commit -qm base

cat > src-tauri/src/lib.rs <<'RS'
pub fn is_adult(age: u32) -> bool {
    age >= 18
}

pub fn label(age: u32) -> &'static str {
    if is_adult(age) { "adult" } else { "child" }
}

pub fn is_senior(age: u32) -> bool {
    age >= 65
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_adult_and_a_child() {
        assert_eq!(super::label(30), "adult");
        assert_eq!(super::label(5), "child");
    }
}
RS
cat > src/lib/agegate.ts <<'TS'
export const isAdult = (age: number): boolean => age >= 18;

export const label = (age: number): string => (isAdult(age) ? "adult" : "child");
TS
cat > src/test/agegate.test.ts <<'TS'
import { expect, it } from "vitest";
import { label } from "../lib/agegate";

it("labels an adult and a child", () => {
  expect(label(30)).toBe("adult");
  expect(label(5)).toBe("child");
});
TS
git add -A && git commit -qm change

out="$(bash scripts/mutation_diff.sh main~1 report.md 2>&1)" || {
  echo "mutation selftest FAILED: mutation_diff.sh exited non-zero"; echo "$out"; exit 1; }
fail() { echo "mutation selftest FAILED: $1"; echo "$out"; exit 1; }
rust="$(echo "$out" | grep -E '^Caught [0-9]+, missed [0-9]+' || true)"
ts="$(echo "$out" | grep -E '^Killed [0-9]+, survived [0-9]+' || true)"
[ -n "$rust" ] || fail "no cargo-mutants counts"
[ -n "$ts" ] || fail "no Stryker counts"
caught="$(echo "$rust" | sed -E 's/^Caught ([0-9]+).*/\1/')"
missed="$(echo "$rust" | sed -E 's/^Caught [0-9]+, missed ([0-9]+).*/\1/')"
killed="$(echo "$ts" | sed -E 's/^Killed ([0-9]+).*/\1/')"
survived="$(echo "$ts" | sed -E 's/^Killed [0-9]+, survived ([0-9]+).*/\1/')"
[ "$caught" -gt 0 ] || fail "cargo-mutants caught no mutant"
[ "$missed" -gt 0 ] || fail "the weak Rust test let no mutant through"
[ "$killed" -gt 0 ] || fail "Stryker killed no mutant"
[ "$survived" -gt 0 ] || fail "the weak TypeScript test let no mutant survive"
echo "$out" | grep -qF 'is_senior' || fail "the Rust missed list does not name is_senior"
echo "$out" | grep -qF 'src/lib/agegate.ts:1' || fail "the TypeScript survivor list does not name isAdult"
# The readiness gate reads these two lines.
echo "mutation selftest: the weak tests let a mutant through in Rust and TypeScript, as expected"
echo "mutation selftest: checked $((caught + missed)) Rust and $((killed + survived)) TypeScript mutants"
