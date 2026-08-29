#!/usr/bin/env bash
# Create a stable self-signed certificate for signing SHIPPED macOS builds, and
# print what CI needs to use it.
#
# Why this exists: macOS keys a privacy grant (Screen Recording, Accessibility,
# Microphone) and a Keychain ACL to the *code identity* of the program asking.
# An ad-hoc signed build has no certificate, so that identity falls back to the
# binary's hash -- which changes with every release. Each update therefore looks
# like a different program: users are asked to grant all three permissions
# again, and are asked for their login password again, with switches in System
# Settings that still read as "on" for a version that no longer exists.
#
# Signing every release with ONE certificate replaces that hash with the
# certificate, which does not change. Measured on macOS 26: two builds with
# different cdhashes signed by the same certificate produce an identical
# designated requirement, which is what the privacy database stores.
#
#   designated => identifier "com.cogniclone.recorder" and certificate leaf = H"..."
#
# What this does NOT do: Gatekeeper still blocks the first launch, because that
# needs notarization, which needs a real Apple Developer ID. This is a stopgap
# until the Developer Program enrollment completes -- at which point only the
# certificate changes and the CI plumbing below stays as it is.
#
# Swapping to the Developer ID will invalidate existing grants once, because the
# certificate is the identity. That is a one-time cost, not a recurring one.
set -euo pipefail

CERT_NAME="${CERT_NAME:-CogniClone Release Signing}"
OUT_DIR="${1:-$HOME/Desktop}"
P12="$OUT_DIR/cogniclone-release-signing.p12"
P12_PASSWORD="$(openssl rand -base64 24)"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "Generating '$CERT_NAME' (valid 10 years)..."
openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
  -keyout "$TMP/key.pem" -out "$TMP/cert.pem" \
  -subj "/CN=$CERT_NAME" \
  -addext "basicConstraints=critical,CA:false" \
  -addext "keyUsage=critical,digitalSignature" \
  -addext "extendedKeyUsage=critical,codeSigning" 2>/dev/null

# OpenSSL 3 defaults to AES-256/SHA-256 for PKCS#12, which the macOS Security
# framework cannot read ("MAC verification failed"). Pin the older algorithms
# it does understand rather than depending on the legacy provider being built.
openssl pkcs12 -export -out "$P12" \
  -inkey "$TMP/key.pem" -in "$TMP/cert.pem" \
  -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 \
  -passout "pass:$P12_PASSWORD"

echo
echo "Wrote $P12"
echo
echo "Add these three repository secrets (Settings -> Secrets and variables -> Actions):"
echo
echo "  APPLE_SIGNING_IDENTITY"
echo "    $CERT_NAME"
echo
echo "  APPLE_CERTIFICATE_PASSWORD"
echo "    $P12_PASSWORD"
echo
echo "  APPLE_CERTIFICATE"
echo "    (the base64 below, as one line)"
echo
base64 -i "$P12"
echo
echo "Keep $P12 and the password somewhere safe: losing them means the next"
echo "release is signed by a different identity, and every user grants all"
echo "three permissions again. Then delete the copy on your Desktop."
