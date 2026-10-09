#!/usr/bin/env bash
# ElyMark signed with hardened runtime still draws its page.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
[ "$(uname)" = Darwin ] || {
  echo "SKIP hardened-macos: macOS only"
  exit 0
}
[ -n "${ELY_SIGN_IDENTITY:-}" ] || {
  echo "SKIP hardened-macos: set ELY_SIGN_IDENTITY to a signing identity"
  exit 0
}
work=$(cd "$(mktemp -d)" && pwd -P)
trap 'pkill -f "$work/ElyMark.app/Contents/MacOS/" 2>/dev/null || true; sleep 1; rm -rf "$work"' EXIT
fail() {
  echo "FAIL: $*" >&2
  exit 1
}
ELY_DIST=$work "$root/packaging/macos/bundle.sh" ely-mark debug >"$work/bundle.log" 2>&1 ||
  fail "bundle: $(tail -n 3 "$work/bundle.log")"
app=$work/ElyMark.app
codesign --verify --deep --strict "$app" || fail "the signature does not verify"
codesign -d --entitlements - "$app/Contents/Frameworks/ely-mark Helper (Renderer).app" 2>/dev/null |
  grep -q allow-jit || fail "the renderer helper has no JIT entitlement"
printf '# Hardened\n' >"$work/t.md"
open -n -a "$app" --env "HOME=$work/home" --env ELY_LOG=info --env ELY_NO_DIALOG=1 "$work/t.md"
log="$work/home/Library/Application Support/ElyOffice/ely-mark/ely.log"
for _ in $(seq 240); do
  grep -q 'mark: shown as' "$log" 2>/dev/null && {
    echo "hardened-macos: the signed page draws"
    exit 0
  }
  sleep 0.25
done
fail "the page never showed: $(grep -E 'web:|mark:' "$log" 2>/dev/null | tail -n 5)"
