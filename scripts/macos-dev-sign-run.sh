#!/usr/bin/env bash
# Cargo `runner` for local macOS development: sign the freshly built binary
# with the stable dev identity, then run it.
#
# This is the only hook between "cargo built a binary" and "the binary runs".
# `tauri dev` shells out to `cargo run`, which honours `runner`, so every dev
# build gets the same code identity and the Keychain stops re-prompting. See
# scripts/macos-dev-cert.sh for why that matters.
#
# Fail-safe by design: if the identity is missing (CI, a fresh clone, another
# developer's machine) this runs the binary unsigned exactly as cargo would
# have. It must never be the reason a build cannot run.
set -uo pipefail

CERT_NAME="CogniClone Dev"
BIN="${1:?no binary passed by cargo}"
shift

if security find-identity -v -p codesigning 2>/dev/null | grep -qF "$CERT_NAME"; then
  # --force replaces the ad-hoc signature cargo's linker already applied.
  if ! codesign --force --sign "$CERT_NAME" "$BIN" 2>/dev/null; then
    echo "warning: could not sign $BIN with '$CERT_NAME'; running unsigned" >&2
  fi
fi

exec "$BIN" "$@"
