#!/usr/bin/env bash
# Create a stable self-signed code-signing identity for local development.
#
# Why this exists: the macOS Keychain binds an item's ACL to the code identity
# of the program asking for it. A dev build is ad-hoc signed, so that identity
# is nothing but the binary's cdhash -- which changes on every `cargo build`.
# Each rebuild therefore looks like a brand-new program that has never been
# granted access, and macOS re-prompts for the login password. "Always Allow"
# only ever whitelists the one build it was clicked for.
#
# Signing every dev build with ONE stable certificate fixes that: the ACL then
# keys off the certificate, which does not change, so a single "Always Allow"
# holds across rebuilds.
#
# This is for local development only. It does nothing for shipped builds, which
# need a real Developer ID certificate and notarization.
#
# Idempotent: re-running when the identity already exists is a no-op.
set -euo pipefail

CERT_NAME="CogniClone Dev"
KEYCHAIN="$HOME/Library/Keychains/login.keychain-db"

if security find-identity -v -p codesigning | grep -qF "$CERT_NAME"; then
  echo "Identity '$CERT_NAME' already present -- nothing to do."
  security find-identity -v -p codesigning | grep -F "$CERT_NAME"
  exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Generating a self-signed code-signing certificate ('$CERT_NAME')..."
# codeSigning EKU is what makes `security find-identity -p codesigning` list it.
openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
  -keyout "$TMP/key.pem" -out "$TMP/cert.pem" \
  -subj "/CN=$CERT_NAME" \
  -addext "basicConstraints=critical,CA:false" \
  -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=critical,codeSigning" 2>/dev/null

# OpenSSL 3 defaults to AES-256/SHA-256 for PKCS#12, which the macOS Security
# framework cannot read ("MAC verification failed"). Pin the older algorithms
# it does understand rather than depending on the legacy provider being built.
openssl pkcs12 -export -out "$TMP/id.p12" \
  -inkey "$TMP/key.pem" -in "$TMP/cert.pem" \
  -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 \
  -passout pass:devcert

echo "Importing into the login keychain..."
# -A lets codesign use the private key without a per-use prompt.
security import "$TMP/id.p12" -k "$KEYCHAIN" -P devcert -A -T /usr/bin/codesign

echo "Trusting it for code signing (macOS may ask for your password once)..."
# User trust domain, so this needs no sudo. A self-signed certificate that is
# not trusted makes codesign fail with CSSMERR_TP_NOT_TRUSTED.
security add-trusted-cert -r trustRoot -p codeSign -k "$KEYCHAIN" "$TMP/cert.pem"

echo
if security find-identity -v -p codesigning | grep -qF "$CERT_NAME"; then
  echo "Done. '$CERT_NAME' is now a usable code-signing identity:"
  security find-identity -v -p codesigning | grep -F "$CERT_NAME"
  echo
  echo "Next: run the app as usual (npm run tauri dev). The first keychain"
  echo "prompt will still appear -- click 'Always Allow' once, and it will"
  echo "stop asking on subsequent rebuilds."
else
  echo "Certificate was created but is not listed as a signing identity." >&2
  exit 1
fi
