#!/usr/bin/env bash
# SessionStart hook for Claude Code cloud sessions.
#
# The cloud platform puts the package registries (pypi.org,
# files.pythonhosted.org, registry.npmjs.org) into NO_PROXY, so uv and npm
# try to reach them directly from a container that has no DNS. Removing them
# from NO_PROXY sends those requests through the agent proxy instead.
#
# Does nothing outside a cloud session, and never fails the session: every
# path exits 0.

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ] || [ -z "${CLAUDE_ENV_FILE:-}" ]; then
  exit 0
fi

current="${NO_PROXY:-${no_proxy:-}}"
filtered="$(printf '%s' "$current" | tr ',' '\n' \
  | grep -vE 'pypi\.org|pythonhosted\.org|npmjs\.org' \
  | grep -v '^$' | paste -sd, -)" || true

{
  printf 'export NO_PROXY=%s\n' "$filtered"
  printf 'export no_proxy=%s\n' "$filtered"
} 2>/dev/null >> "$CLAUDE_ENV_FILE" || true

exit 0
