#!/usr/bin/env bash
# MARK-05, MARK-10 live: folder tree, outline, search, recent.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
[ "$(uname)" = Darwin ] || { echo "SKIP: macOS only"; exit 0; }
[ -d "${ELY_CEF:-$root/engines/cef}/Chromium Embedded Framework.framework" ] || {
  echo "SKIP: no CEF; run scripts/fetch-cef.sh"
  exit 0
}
command -v npm >/dev/null || { echo "SKIP: no npm"; exit 0; }
work=$(mktemp -d)
app=
trap 'if [ -n "$app" ]; then kill "$app" 2>/dev/null || true; wait "$app" 2>/dev/null || true; fi; rm -rf "$work"' EXIT
fail() {
  echo "FAIL: $*" >&2
  exit 1
}

"$root/scripts/build-mark-content.sh" >"$work/build" 2>&1 || fail "build: $(tail -n 5 "$work/build")"
cargo build -q -p ely-mark --manifest-path "$root/Cargo.toml"
for tool in ax-values ax-press click; do
  swiftc -O -o "$work/$tool" "$root/tests/scripts/$tool.swift"
done
mkdir -p "$work/notes/sub"
# A heading inside a quote Milkdown rewrites: one raw block.
printf '# Alpha\n\nSome text.\n\n> ## Beta ##\n>\n> More words here.\n' >"$work/notes/a.md"
# A hit hidden in a link's address comes first.
printf '# Other\n\nwords [link](words)\nwords too\n' >"$work/notes/sub/b.md"
printf 'banana\nana too\n\n[link](words)\nwords two' >"$work/notes/d.md"
printf -- '---\ntitle: T\n---\n# Heading\n\nneedle here.\n\n# Other\n\nother paragraph.\n' >"$work/notes/c.md"
printf 'words\n' >"$work/notes/skip.txt"
# A file the search cannot read.
printf 'words\n' >"$work/notes/bad.md"
chmod 000 "$work/notes/bad.md"
launch() {
  ELY_LOG=info ELY_LOCALE=en ELY_NO_DIALOG=1 HOME="$work" \
    "$root/target/debug/ely-mark" "$@" >"$work/out" 2>"$work/err" &
  app=$!
}
waits() {
  for _ in $(seq 120); do
    grep -qF "$1" "$work/err" && return 0
    kill -0 "$app" 2>/dev/null || fail "ElyMark quit: $(tail -n 5 "$work/err")"
    sleep 0.25
  done
  fail "never saw '$1': $(grep -E 'mark|office' "$work/err" | tail -n 5)"
}
# Sets x and y to element `$1`'s center.
at() {
  local found
  for _ in $(seq 40); do
    found=$("$work/ax-values" "$app" | awk -F'\t' -v want="title=$1" '$1 != "AXMenuItem" && $3 == want { sub("at=", "", $5); print $5; exit }')
    if [ -n "$found" ]; then
      IFS=, read -r x y <<<"$found"
      return 0
    fi
    sleep 0.25
  done
  fail "no '$1' in the AX tree"
}
key() { osascript -e "tell application \"System Events\" to $1" -e 'delay 0.8' >/dev/null; }
press() { [ "$("$work/ax-press" "$app" "$1")" = pressed ] || fail "could not press $1"; sleep 0.8; }
saves() {
  local before
  before=$(grep -c "office: saved" "$work/err" || true)
  key 'keystroke "s" using command down'
  for _ in $(seq 40); do
    [ "$(grep -c "office: saved" "$work/err" || true)" -gt "$before" ] && return 0
    sleep 0.25
  done
  fail "no save"
}

launch "$work/notes" "$work/notes/a.md"
waits 'mark: shown as'
[ "$("$work/ax-press" $$ none)" != untrusted ] || fail "needs Accessibility permission"
osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app) to true" -e 'delay 1' >/dev/null
# Tree rows are tree items; Return opens a picked one.
at b.md
"$work/click" "$x" "$y"
key 'key code 36'
waits "opened b.md"
# An outline heading takes the caret there.
press a.md
press Outline
at Outline
"$work/click" "$((x - 64))" "$((y + 64))"
key 'keystroke "Z"'
saves
grep -qx '> ## ZBeta ##' "$work/notes/a.md" || fail "outline jump: $(cat "$work/notes/a.md")"
# Folder search finds lines; a hit opens there.
press Search
at Folder
"$work/click" "$x" "$y"
press "This file"
"$work/click" "$x" "$((y - 37))"
key 'keystroke "words"'
key 'key code 36'
press Folder
at "> More words here."
at "words too"
at "bad.md was not searched"
! "$work/ax-values" "$app" | grep -q "title=words	" || fail "a .txt file was searched"
chmod 600 "$work/notes/bad.md"
# Inside a paragraph the caret lands on the hit's line.
press "words too"
key 'key code 36'
key 'keystroke "Q"'
saves
[ "$(cat "$work/notes/sub/b.md")" = $'# Other\n\nwords [link](words)\nQwords too' ] || fail "search hit: $(cat "$work/notes/sub/b.md")"
# Lines count from the file's top, past front matter.
press Search
at Folder
"$work/click" "$x" "$((y - 37))"
key 'keystroke "a" using command down'
key 'keystroke "needle"'
key 'key code 36'
at "needle here."
press "needle here."
key 'key code 36'
waits "opened c.md"
sleep 1
key 'keystroke "Q"'
saves
grep -qx 'Qneedle here.' "$work/notes/c.md" || fail "front matter offset: $(cat "$work/notes/c.md")"
# This file's results follow the shown tab.
press Search
press "This file"
at "Qneedle here."
press b.md
sleep 1.5
! "$work/ax-values" "$app" | grep -q "title=Qneedle here." || fail "results stayed with the last tab"
# Overlapping hits; a hit hidden in a link's address.
press Search
at Folder
"$work/click" "$x" "$((y - 37))"
key 'keystroke "a" using command down'
key 'keystroke "ana"'
key 'key code 36'
press Folder
press "ana too"
key 'key code 36'
waits "opened d.md"
sleep 1
key 'keystroke "Q"'
saves
[ "$(cat "$work/notes/d.md")" = $'banana\nQana too\n\n[link](words)\nwords two' ] || fail "overlapping hit: $(cat "$work/notes/d.md")"
press Search
at Folder
"$work/click" "$x" "$((y - 37))"
key 'keystroke "a" using command down'
key 'keystroke "words"'
key 'key code 36'
press Folder
press "[link](words)"
key 'key code 36'
sleep 1
key 'keystroke "R"'
saves
[ "$(cat "$work/notes/d.md")" = $'banana\nQana too\n\n[Rlink](words)\nwords two' ] || fail "hidden hit: $(cat "$work/notes/d.md")"
# A rename shows in the folder's tree.
press Files
key 'key code 120'
key 'keystroke "a" using command down'
key 'keystroke "moved.md"'
key 'key code 36'
waits "renamed"
at moved.md
! "$work/ax-values" "$app" | grep -q "^AXRow.*title=d.md	" || fail "the tree kept the old name"
kill "$app"
wait "$app" 2>/dev/null || true
app=
# The recent list outlives the app.
python3 -c 'import json,sys; l=json.load(open(sys.argv[1])); sys.exit(0 if any(p.endswith("notes") for p in l) and any(p.endswith("a.md") for p in l) else 1)' \
  "$(find "$work/Library" -name recent.json)" || fail "recent: $(cat "$(find "$work/Library" -name recent.json)")"
launch
waits 'ElyMark window open'
at notes
press notes
key 'key code 36'
waits "opens folder"
at Outline
kill "$app"
wait "$app" 2>/dev/null || true
app=
! grep -q "panicked" "$work/err" || fail "panics: $(grep -m3 panicked "$work/err")"
echo "ok: mark places"
