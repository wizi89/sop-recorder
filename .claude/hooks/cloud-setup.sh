#!/usr/bin/env bash
# SessionStart hook: prepares a cloud session so the agent's loop runs the
# same checks as CI: the Tauri system libraries for `cargo clippy` and
# `cargo test` on Linux, and `npm ci` for tsc, vitest and the hook's Biome
# check. Versioned here rather than in the environment's setup script, which
# lives outside the repo and is skipped when the environment cache exists.
#
# Runs only where CLAUDE_CODE_REMOTE=true (cloud sessions; CI's Linux test
# leg sets it to prove this script). Idempotent: installed packages and an
# existing node_modules are skipped. Never fails the session: it reports what
# failed and exits 0, and CI is still the hard gate.

[ "${CLAUDE_CODE_REMOTE:-}" = "true" ] || exit 0
cd "${CLAUDE_PROJECT_DIR:-.}" || exit 0

# Tauri v2 on Linux, plus PipeWire and libclang for xcap's screen capture.
PACKAGES="libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libxdo-dev
libssl-dev libasound2-dev libpipewire-0.3-dev libclang-dev pkg-config build-essential"

sudo_cmd=""
[ "$(id -u)" -eq 0 ] || sudo_cmd="sudo -n"

failed=""
missing=""
for p in $PACKAGES; do
  dpkg -s "$p" >/dev/null 2>&1 || missing="$missing $p"
done
if [ -n "$missing" ]; then
  # shellcheck disable=SC2086
  if ! { $sudo_cmd apt-get update -qq && $sudo_cmd env DEBIAN_FRONTEND=noninteractive \
      apt-get install -y -qq $missing; } >/tmp/cloud-setup-apt.log 2>&1; then
    failed="$failed apt(see /tmp/cloud-setup-apt.log)"
  fi
fi

# The platform lists the registries in NO_PROXY, which sends npm around the
# agent proxy into a container without DNS (cloud-no-proxy.sh fixes that for
# later commands; hooks run before it takes effect).
NO_PROXY="$(printf '%s' "${NO_PROXY:-${no_proxy:-}}" | tr ',' '\n' \
  | grep -vE 'pypi\.org|pythonhosted\.org|npmjs\.org' | grep -v '^$' | paste -sd, -)"
export NO_PROXY no_proxy="$NO_PROXY"

if [ ! -d node_modules ]; then
  npm ci --no-audit --no-fund >/tmp/cloud-setup-npm.log 2>&1 \
    || failed="$failed npm-ci(see /tmp/cloud-setup-npm.log)"
fi

if [ -n "$failed" ]; then
  echo "cloud-setup: FAILED:$failed. cargo and npm checks may not run here; CI still does."
else
  echo "cloud-setup: Tauri system libraries and node_modules ready; run cargo clippy and cargo test in src-tauri."
fi
exit 0
