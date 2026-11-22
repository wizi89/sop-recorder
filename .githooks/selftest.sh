#!/bin/sh
# Proves .githooks/pre-commit works, in a throwaway repo:
#   - planted violations: the hook blocks (exit 1) and reports each one, counts
#     the code files it checked (N > 0), follows a rename and handles a path
#     with a space;
#   - exempt and uncapped files (generated Tauri code, a large doc) are not
#     capped;
#   - clean staged files: the hook lets the commit through (exit 0).
# Biome runs from this repo's node_modules when installed (npm ci).
# Run: sh .githooks/selftest.sh
here="$(cd "$(dirname "$0")" && pwd)"
hook="$here/pre-commit"
real_biome="$(cd "$here/.." && pwd)/node_modules/.bin/biome"
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
cd "$tmp" && git init -q && git config user.email t@t && git config user.name t && git config core.autocrlf false
mkdir -p src src-tauri/gen docs "with space"
if [ -x "$real_biome" ]; then
  mkdir -p node_modules/.bin
  printf '#!/bin/sh\nexec "%s" "$@"\n' "$real_biome" > node_modules/.bin/biome
  chmod +x node_modules/.bin/biome
  printf 'node_modules/\n' > .gitignore
fi
big() { head -c 24000 /dev/zero | tr '\0' "$2" > "$1"; }
fail() { echo "selftest FAILED: $1"; echo "$out"; exit 1; }

# Clean: small formatted Rust and TypeScript, plus a large doc and generated file.
printf 'fn main() {}\n' > src/good.rs
printf 'export const x = 1;\n' > src/good.ts
big docs/long.md f
big src-tauri/gen/schema.rs b
git add -A
out=$(sh "$hook" 2>&1); rc=$?
[ "$rc" -eq 0 ] || fail "clean stage: hook exited $rc, must let the commit through"
echo "$out" | grep -qF "token cap checked 2 staged code file(s), 0 over the cap" || fail "clean stage: wrong file count"
echo "$out" | grep -qF "is ~6000 tokens" && fail "clean stage: an exempt or uncapped file was capped"
git commit -qm base --no-verify

# Violations.
big moved.css c; git add -A && git commit -qm moved --no-verify
git mv moved.css renamed.css
big big.mjs a
big "with space/big file.sh" d
printf 'fn main(){let x=1;}\n' > src/bad.rs
printf 'export const y=2\n' > src/ugly.ts
git add -A
out=$(sh "$hook" 2>&1); rc=$?
[ "$rc" -eq 1 ] || fail "violations: hook exited $rc, must block with 1"
for f in big.mjs "with space/big file.sh" renamed.css; do
  echo "$out" | grep -qF "$f is ~6000 tokens, over the 5000 cap" || fail "$f not reported over the cap"
done
echo "$out" | grep -qi "warning" && fail "hook printed a warning"
echo "$out" | grep -qF "token cap checked 5 staged code file(s), 3 over the cap" || fail "wrong file count"
echo "$out" | grep -qF "commit blocked by: token-cap" || fail "block message missing"
if echo "$out" | grep -q "rustfmt not found"; then echo "selftest: rustfmt not installed here, Rust check skipped"
else
  echo "$out" | grep -qF "Diff in" || fail "rustfmt finding 'Diff in' missing"
  echo "$out" | grep -qF " rustfmt" || fail "rustfmt missing from the block message"
fi
if echo "$out" | grep -q "Biome not installed"; then echo "selftest: Biome not installed here, web check skipped"
else
  echo "$out" | grep -qF "ugly.ts" || fail "Biome finding for src/ugly.ts missing"
  echo "$out" | grep -qF " biome." || fail "biome missing from the block message"
fi
echo "selftest passed"
