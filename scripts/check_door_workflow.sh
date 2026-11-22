#!/usr/bin/env bash
# Proves the gate in .github/workflows/one-way-door.yml both ways, on stubbed
# PRs: the workflow's own run script is extracted and run against a fake `gh`
# that serves a PR's files, commits and body. A door PR with an unticked box
# must go red, a PR touching no door and a fully ticked one must go green, and
# boxes must be appended once, also on a later push.
# Usage: bash scripts/check_door_workflow.sh <a non-guardrail door path>
# Needs bash and jq (both on GitHub's runners).
set -u
door=${1:?usage: check_door_workflow.sh <door path>}
wf=.github/workflows/one-way-door.yml
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
awk '/run: \|/ { on = 1; next } on { sub(/^          /, ""); print }' "$wf" > "$tmp/run.sh"
[ -s "$tmp/run.sh" ] || { echo "door: no run script found in $wf"; exit 1; }

gh() {
  case "$*" in
    *"/files"*) cat "$T/files" ;;
    *"/commits"*) tr ' ' '\n' < "$T/commits" ;;
    *"-X POST"*) : ;;
    *"-X PATCH"*) jq -r .body > "$T/body" ;;
    *"pulls/"*) cat "$T/body" ;;
  esac
}
export -f gh
export REPO=o/r PR=1 T="$tmp"

checked=0; wrong=0
# case <name> <expected green|red> <author> <commit authors> <files...>; the body persists between cases.
run_case() {
  name=$1 want=$2; export AUTHOR=$3; printf '%s' "$4" > "$tmp/commits"; shift 4
  printf '%s\n' "$@" > "$tmp/files"
  bash "$tmp/run.sh" > "$tmp/out" 2>&1 && got=green || got=red
  checked=$((checked + 1))
  if [ "$got" = "$want" ]; then echo "door: $name: $got, as expected"
  else wrong=$((wrong + 1)); echo "door: $name: $got, expected $want"; sed 's/^/  /' "$tmp/out"; fi
}
# sed -i differs between GNU and BSD sed; this works on both.
edit_body() { sed "$1" "$tmp/body" > "$tmp/body.new" && mv "$tmp/body.new" "$tmp/body"; }
boxes() { grep -c "^- \[.\] $1" "$tmp/body"; }
expect() { if [ "$1" != "$2" ]; then wrong=$((wrong + 1)); echo "door: $3: got $1, expected $2"; fi; }

printf 'Body\n' > "$tmp/body"
run_case "no door path" green me me README.md
expect "$(grep -c 'one-way-door-checklist' "$tmp/body")" 0 "no door path leaves the body alone"

run_case "guardrail path with an unticked box" red me me .githooks/pre-commit
expect "$(boxes Guardrail)" 1 "guardrail box appended once"
run_case "rerun appends no duplicate" red me me .githooks/pre-commit
expect "$(boxes Guardrail)" 1 "guardrail box still once after a rerun"

edit_body 's/^- \[ \] Guardrail/- [x] Guardrail/'
run_case "ticked guardrail box" green me me .githooks/pre-commit
run_case "a later push touches another door" red me me .githooks/pre-commit "$door"
expect "$(grep -c '^- \[x\] Guardrail' "$tmp/body")" 1 "the ticked box stays ticked"
edit_body 's/^- \[ \]/- [x]/'
run_case "every box ticked" green me me .githooks/pre-commit "$door"

printf 'Bump\n' > "$tmp/body"
run_case "Dependabot alone bumps an action" green 'dependabot[bot]' 'dependabot[bot]' .github/workflows/one-way-door.yml
run_case "someone else pushed to a Dependabot PR" red 'dependabot[bot]' 'dependabot[bot] me' .github/workflows/one-way-door.yml

if [ "$wrong" -gt 0 ]; then echo "door: checked $checked cases, $wrong wrong"; exit 1; fi
echo "door: checked $checked cases, all as expected"
