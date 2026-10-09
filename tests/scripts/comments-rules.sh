#!/usr/bin/env bash
# comments.py fails on long and multi-line comments.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
fail() {
  echo "FAIL: $*" >&2
  exit 1
}
check() { python3 "$root/tests/scripts/comments.py" "$work" >/dev/null 2>&1; }
mkdir -p "$work/apps/a/web/src" "$work/tests/scripts"
printf '// One short line.\nconst a = 1\n' >"$work/apps/a/web/src/a.js"
check || fail "a short comment failed"
printf '/* A comment\n   spanning lines. */\n' >"$work/apps/a/web/src/a.js"
! check || fail "a multi-line JS block comment passed"
printf 'const a = 1 /* A comment\n   spanning lines. */\n' >"$work/apps/a/web/src/a.js"
! check || fail "an inline multi-line JS block comment passed"
printf '// One two three four five six seven eight nine ten eleven.\n' >"$work/apps/a/web/src/a.js"
! check || fail "an eleven-word JS comment passed"
rm "$work/apps/a/web/src/a.js"
printf '// One two three four five six seven eight nine ten eleven.\n' >"$work/tests/scripts/a.swift"
! check || fail "an eleven-word Swift comment passed"
echo "ok: comment rules"
