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
mkdir -p "$work/site/src"
for ext in ts css; do
  printf '/* A comment\n   spanning lines. */\n' >"$work/site/src/a.$ext"
  ! check || fail "a multi-line .$ext block comment passed"
  printf '/* One two three four five six seven eight nine ten eleven. */\n' >"$work/site/src/a.$ext"
  ! check || fail "an eleven-word .$ext comment passed"
  printf '/* One short line. */\n' >"$work/site/src/a.$ext"
  check || fail "a short .$ext comment failed"
  rm "$work/site/src/a.$ext"
done
printf '// One two three four five six seven eight nine ten eleven.\n' >"$work/tests/scripts/a.swift"
! check || fail "an eleven-word Swift comment passed"
echo "ok: comment rules"
