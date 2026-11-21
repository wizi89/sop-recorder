#!/usr/bin/env bash
# SessionStart hook: turns on the repo's committed git hooks (.githooks/), so
# pre-commit feedback runs in every session, cloud runs included.
# It never overwrites a core.hooksPath that points elsewhere: a clone may run
# its own hooks (sop-recorder's private denylist hooks chain to .githooks/
# themselves). Then it only says so. Never fails the session.
repo="${CLAUDE_PROJECT_DIR:-.}"
current=$(git -C "$repo" config core.hooksPath 2>/dev/null)
if [ -z "$current" ]; then
  git -C "$repo" config core.hooksPath .githooks 2>/dev/null || true
elif [ "$current" != ".githooks" ]; then
  echo "enable-githooks: core.hooksPath is '$current', left unchanged; that hooks dir must call .githooks/pre-commit" >&2
fi
exit 0
