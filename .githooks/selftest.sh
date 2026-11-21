#!/bin/sh
# Proves .githooks/pre-commit works: plants violations in a throwaway repo and
# fails unless the hook reports each one, counts the files it checked (N > 0),
# skips the exempt file, follows a rename, handles a path with a space, and
# still exits 0 (report-only). Run: sh .githooks/selftest.sh
hook="$(cd "$(dirname "$0")" && pwd)/pre-commit"
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
cd "$tmp" && git init -q && git config user.email t@t && git config user.name t
mkdir -p server src db/migrations "with space"
big() { head -c 24000 /dev/zero | tr '\0' "$2" > "$1"; }
big moved.txt c; git add -A && git commit -qm base --no-verify
git mv moved.txt renamed.txt
big big.txt a
big "with space/big file.txt" d
big Cargo.lock b
big xlock e
printf 'fn main(){let x=1;}\n' > src/bad.rs
git add -A
out=$(sh "$hook" 2>&1); rc=$?
fail() { echo "selftest FAILED: $1"; echo "$out"; exit 1; }
[ "$rc" -eq 0 ] || fail "hook exited $rc; report-only must exit 0"
for f in big.txt "with space/big file.txt" renamed.txt xlock; do
  echo "$out" | grep -qF "$f is ~6000 tokens, over the 5000 cap" || fail "$f not reported over the cap"
done
echo "$out" | grep -qF "Cargo.lock is" && fail "exempt file Cargo.lock was capped"
echo "$out" | grep -qi "warning" && fail "hook printed a warning"
echo "$out" | grep -qF "token cap checked 5 staged file(s), 4 over the cap" || fail "wrong file count"
if echo "$out" | grep -q "skipped"; then echo "selftest: linter not installed here, linter check skipped"
else echo "$out" | grep -qF "Diff in" || fail "linter finding 'Diff in' missing"; fi
echo "selftest passed"
