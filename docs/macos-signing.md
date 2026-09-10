# Signing and notarizing the macOS build

Shipped macOS builds are signed with a **Developer ID Application** certificate
issued to cogniclone UG (Team ID `GU8UU55YUW`) and notarized by Apple. Both
happen inside `.github/workflows/release.yml`; nothing is signed by hand.

Until September 2026 this was a self-signed stopgap that gave releases a stable
code identity but could not pass Gatekeeper. That script is gone; the CI
plumbing around it survived unchanged, which was the point of building it that
way.

## Why the identity has to stay stable

macOS keys a privacy grant (Screen Recording, Accessibility, Microphone) and a
Keychain ACL to the *code identity* of the program asking. With a real
certificate that identity is the certificate, not the binary hash, so it
survives every release. Sign a release with a different certificate and every
user is asked to grant all three permissions again, with switches in System
Settings that still read as "on" for a version that no longer exists.

So: **do not re-issue the certificate unless it expires.** It is valid until
**11 September 2031**. What protects you is `APPLE_SIGNING_PRIVATE_KEY` in
Bitwarden Secrets Manager: Apple will re-issue a certificate against the same
key, and the resulting code identity is identical, so nothing re-prompts.

## Repository secrets

| Secret | What it is |
| --- | --- |
| `APPLE_CERTIFICATE` | base64 of `developerid.p12` (leaf + G2 intermediate) |
| `APPLE_CERTIFICATE_PASSWORD` | export password for that `.p12` |
| `APPLE_SIGNING_IDENTITY` | `Developer ID Application: cogniclone UG (haftungsbeschrankt) (GU8UU55YUW)` |
| `APPLE_API_ISSUER` | App Store Connect API issuer UUID |
| `APPLE_API_KEY` | its 10-character key ID |
| `APPLE_API_KEY_P8` | contents of `AuthKey_<key id>.p8` |

`APPLE_SIGNING_IDENTITY` is passed straight to `codesign -s`. Copy it verbatim
from `security find-identity -v -p codesigning` -- a re-typed name fails with
"failed to resolve signing identity", which looks exactly like a missing
certificate.

The notarization credentials are an App Store Connect **Team** key, so they are
not tied to anyone's Apple ID. Only the Account Holder can create one
(App Store Connect -> Users and Access -> Integrations -> Team Keys), and the
`.p8` is downloadable exactly once.

## Where the values live

GitHub secrets are write-only -- nothing can be read back out of them. The
source of truth is the Bitwarden Secrets Manager project `Infrastruktur`,
reachable with the `bws` wrappers set up in `docs/DEPLOYMENT.md` of the
sop-sorcery repo:

```bash
bws-do secret list                       # keys, ids and values as JSON
```

| Secrets Manager key | Recreatable? |
| --- | --- |
| `APPLE_SIGNING_PRIVATE_KEY` | **No.** RSA key behind the certificate. |
| `APPLE_API_KEY_P8` | **No.** Apple allows one download. |
| `APPLE_CERTIFICATE` | Yes -- rebuild from the private key, below. |
| `APPLE_CERTIFICATE_PASSWORD` | Yes -- chosen when the `.p12` is rebuilt. |
| `APPLE_SIGNING_IDENTITY`, `APPLE_TEAM_ID`, `APPLE_API_KEY`, `APPLE_ISSUER_ID` | Yes -- readable in the Apple portals. |
| `APPLE_ID`, `APPLE_PASSWORD` | Yes -- the app-specific-password fallback. |

Note the key is `APPLE_ISSUER_ID`, but the variable Tauri reads is
`APPLE_API_ISSUER`. `bws run` maps a key to a variable of the *same* name, so
that one has to be renamed by hand for a locally notarized build.

## Recovering everything from Bitwarden

Verified end to end on 10 September 2026: the identity rebuilt this way had the
same SHA-1 as the original (`77DE5BA9...`), which is what makes it the same code
identity and costs nobody their permissions.

You need the private key from Bitwarden and a certificate re-downloaded from
developer.apple.com. **Both.** The Apple account going away is the one scenario
this does not cover.

```bash
bws-do secret list | python3 -c "
import json,sys
d={s['key']:s['value'] for s in json.load(sys.stdin)}
v=d['APPLE_SIGNING_PRIVATE_KEY']
open('developerid.key','w').write(v if v.endswith(chr(10)) else v+chr(10))"
```

Download the certificate from developer.apple.com/account -> Certificates (an
issued certificate stays downloadable for its whole life), then follow the
`.p12` rebuild in the next section from step 2 onward.

### Fallback: app-specific password

notarytool also accepts `APPLE_ID` + `APPLE_PASSWORD` (an app-specific password
from account.apple.com, never the real one) + `APPLE_TEAM_ID`. Those are in
Bitwarden too. Swapping to them means replacing the three `APPLE_API_*` entries
in the build step's `env` and deleting the "Provide the notarization API key"
step. The tradeoff is that the credential dies with the account it belongs to.

## Re-issuing when the certificate expires

Only the Account Holder can create a Developer ID certificate, and the team gets
a limited number of them. Work outside the repo -- `.gitignore` does not cover
`*.key` or `*.p12`.

```bash
mkdir -p ~/Desktop/cogniclone-signing && cd ~/Desktop/cogniclone-signing

# 1. Private key and CSR (Apple requires RSA 2048)
openssl genrsa -out developerid.key 2048
openssl req -new -key developerid.key -out developerid.csr \
  -subj "/emailAddress=wilhelm@cogniclone.ai/CN=cogniclone UG/C=DE"
```

Upload `developerid.csr` at developer.apple.com/account -> Certificates -> **+**
-> **Developer ID Application** -> profile type **G2 Sub-CA (Xcode 11.4.1 or
later)**. The older "Previous Sub-CA" option issues a certificate notarization
no longer accepts. Download `developerID_application.cer` into the same folder.

```bash
# 2. Convert Apple's DER output and fetch the intermediate it chains to
openssl x509 -inform DER -in developerID_application.cer -out leaf.pem
curl -sSLo DeveloperIDG2CA.cer https://www.apple.com/certificateauthority/DeveloperIDG2CA.cer
openssl x509 -inform DER -in DeveloperIDG2CA.cer -out intermediate.pem

# 3. Bundle into the .p12 CI consumes.
#    The legacy algorithms are not optional: OpenSSL 3 defaults to
#    AES-256/SHA-256 for PKCS#12, which the macOS Security framework cannot
#    read ("MAC verification failed").
openssl rand -base64 24 > p12-password.txt
openssl pkcs12 -export -out developerid.p12 \
  -inkey developerid.key -in leaf.pem -certfile intermediate.pem \
  -keypbe PBE-SHA1-3DES -certpbe PBE-SHA1-3DES -macalg sha1 \
  -passout file:p12-password.txt
```

Shipping the intermediate inside the `.p12` is what lets the runner build the
chain without reaching out to Apple mid-build.

Verify before trusting it. The keychain must be in the **search list** or the
chain cannot be evaluated and a perfectly good certificate reports
"0 valid identities found":

```bash
PW="$(head -1 p12-password.txt)"
security create-keychain -p test check.keychain-db
security unlock-keychain -p test check.keychain-db
security import developerid.p12 -k check.keychain-db -P "$PW" -A
ORIG=$(security list-keychains -d user | tr -d ' "')
security list-keychains -d user -s check.keychain-db $ORIG
security find-identity -v -p codesigning check.keychain-db   # expect 1 valid
security list-keychains -d user -s $ORIG
security delete-keychain check.keychain-db
```

Then push the new values to Secrets Manager -- `--` is required or `bws` reads
a leading `-----BEGIN` as a flag, and it prints the key material into the error
when it does:

```bash
bws-do secret edit <id of APPLE_CERTIFICATE> --value "$(base64 -i developerid.p12 | tr -d '\n')"
bws-do secret edit <id of APPLE_SIGNING_PRIVATE_KEY> --value "$(cat developerid.key)"
```

Mirror them to GitHub with `gh secret set`, then `rm -rf ~/Desktop/cogniclone-signing`.

## Verifying a release

On a machine that **downloaded** the artifact from the release page, so it
carries the quarantine flag:

```bash
codesign -dv --verbose=4 /Applications/cogniclone.app   # Authority: Developer ID Application...
codesign --verify --deep --strict --verbose=2 /Applications/cogniclone.app
spctl -a -vvv -t install /Applications/cogniclone.app   # accepted, source=Notarized Developer ID
xcrun stapler validate /Applications/cogniclone.app
xcrun stapler validate cogniclone_*_universal.dmg
```

Also check the updater payload, because that is the path existing users take and
the one nobody tests: unpack the `.app.tar.gz` from the release and run
`stapler validate` on the app inside it.

If the **DMG** specifically fails `stapler validate`, add a post-bundle step
running `xcrun notarytool submit` and `xcrun stapler staple` against it. Tauri
staples the `.app` reliably, the disk image less so, and an unstapled DMG needs
a network round-trip to Apple the first time a user opens it.

## Local development

Unrelated to any of this. `scripts/macos-dev-cert.sh` creates a stable
self-signed identity and `scripts/macos-dev-sign-run.sh` is the cargo `runner`
that applies it, so the login keychain stops re-prompting on every rebuild.
Neither touches the Developer ID.
