#!/usr/bin/env bash
# MARK-06, MARK-07 live: find, replace, Tab, lists, tables, images.
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
for tool in ax-values ax-press click drop; do
  swiftc -O -o "$work/$tool" "$root/tests/scripts/$tool.swift"
done
mkdir "$work/notes"
printf 'alpha beta alpha\n\ngamma alpha\n' >"$work/notes/t.md"
# A two-pixel PNG.
python3 -c 'import struct,zlib,sys
c=lambda t,d: struct.pack(">I",len(d))+t+d+struct.pack(">I",zlib.crc32(t+d))
open(sys.argv[1],"wb").write(b"\x89PNG\r\n\x1a\n"+c(b"IHDR",struct.pack(">IIBBBBB",2,2,8,2,0,0,0))+c(b"IDAT",zlib.compress(b"\0"+b"\xff\0\0"*2+b"\0"+b"\0\xff\0"*2))+c(b"IEND",b""))' "$work/red pic.png"
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
key() { osascript -e "tell application \"System Events\" to $1" -e 'delay 0.8' >/dev/null; }
press() {
  for _ in $(seq 20); do
    [ "$("$work/ax-press" "$app" "$1")" = pressed ] && { sleep 0.8; return 0; }
    sleep 0.25
  done
  fail "could not press $1"
}
# Clicks the find box's `$1`th text field, top first.
field() {
  local found
  found=$("$work/ax-values" "$app" | awk -F'\t' '$1 == "AXTextField" { sub("at=", "", $5); print $5 }' | sort -t, -k2 -n | sed -n "$1p")
  [ -n "$found" ] || fail "no find field $1"
  IFS=, read -r x y <<<"$found"
  "$work/click" "$x" "$y"
  sleep 0.5
}
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
is() { [ "$(cat "$work/notes/t.md")" = "$1" ] || fail "$2: $(cat "$work/notes/t.md")"; }

launch "$work/notes/t.md"
waits 'mark: shown as "visual"'
[ "$("$work/ax-press" $$ none)" != untrusted ] || fail "needs Accessibility permission"
osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app) to true" -e 'delay 2' >/dev/null
# Replace all in the visual editor.
key 'keystroke "f" using command down'
key 'keystroke "alpha"'
press "Toggle replace"
field 2
key 'keystroke "omega"'
press "Replace all"
saves
is $'omega beta omega\n\ngamma omega' "replace all"
# A regular expression in the source editor.
press Source
waits 'mark: shown as "source"'
field 1
key 'keystroke "a" using command down'
key 'keystroke "g.mma"'
press "Regular expression"
field 2
key 'keystroke "a" using command down'
key 'keystroke "delta"'
press Replace
saves
is $'omega beta omega\n\ndelta omega' "regex replace"
# Tab indents the selected line; Esc closes the box first.
field 1
key 'key code 53'
key 'key code 48'
saves
grep -qx '[[:space:]]\{1,\}delta omega' "$work/notes/t.md" || fail "tab: $(cat "$work/notes/t.md")"
key 'key code 48 using shift down'
saves
is $'omega beta omega\n\ndelta omega' "shift-tab"
# A list item continues on Return.
key 'key code 125 using command down'
key 'key code 36'
key 'keystroke "- one"'
key 'key code 36'
key 'keystroke "two"'
saves
grep -qx -- '- two' "$work/notes/t.md" || fail "list: $(cat "$work/notes/t.md")"
# Tables: insert, then a column and a row.
press Visual
waits 'mark: shown as "visual"'
# Closing the find box hands the keys to the page.
key 'keystroke "f" using command down'
key 'key code 53'
# Command arrows move though AppKit sends no edit command.
key 'key code 125 using command down'
press "Insert Table"
press "Add Column Right"
press "Add Row Below"
saves
header=$(grep -m1 '^ *|' "$work/notes/t.md" || true)
[ "$(tr -cd '|' <<<"$header" | wc -c | tr -d ' ')" = 5 ] || fail "table columns: $(cat "$work/notes/t.md")"
[ "$(grep -c '^ *|' "$work/notes/t.md")" = 5 ] || fail "table rows: $(cat "$work/notes/t.md")"
[ "$(head -n 1 "$work/notes/t.md")" = "omega beta omega" ] || fail "table not at the end: $(cat "$work/notes/t.md")"
press "Delete Column"
saves
header=$(grep -m1 '^ *|' "$work/notes/t.md" || true)
[ "$(tr -cd '|' <<<"$header" | wc -c | tr -d ' ')" = 4 ] || fail "delete column: $(cat "$work/notes/t.md")"
# A dropped image is copied beside the file, name escaped.
window=$("$work/ax-values" "$app" | awk -F'\t' '$1 == "AXWindow" { sub("at=", "", $5); print $5; exit }')
IFS=, read -r x y <<<"$window"
[ "$("$work/drop" "$work/red pic.png" "$x" "$y")" = dropped ] || fail "the drop was refused"
waits "mark: attached 1 images"
saves
cmp -s "$work/red pic.png" "$work/notes/assets/red pic.png" || fail "dropped copy: $(ls -R "$work/notes")"
grep -qF '](assets/red%20pic.png)' "$work/notes/t.md" || fail "drop link: $(cat "$work/notes/t.md")"
# A pasted image lands in assets beside the file.
if [ "${ELY_CLIPBOARD_TEST:-}" = 1 ]; then
  osascript -e "set the clipboard to (read (POSIX file \"$work/red pic.png\") as «class PNGf»)"
  key 'key code 125 using command down'
  key 'key code 36'
  key 'keystroke "v" using command down'
  saves
  png=$(ls "$work/notes/assets"/pasted-*.png 2>/dev/null | head -n 1)
  [ -n "$png" ] || fail "no pasted image: $(ls -R "$work/notes")"
  grep -qF "](assets/$(basename "$png"))" "$work/notes/t.md" || fail "image link: $(cat "$work/notes/t.md")"
fi
# Saved elsewhere mid-copy: no link to nowhere.
mkfifo "$work/slow.png"
mkdir "$work/elsewhere"
window=$("$work/ax-values" "$app" | awk -F'\t' '$1 == "AXWindow" { sub("at=", "", $5); print $5; exit }')
IFS=, read -r x y <<<"$window"
[ "$("$work/drop" "$work/slow.png" "$x" "$y")" = dropped ] || fail "the slow drop was refused"
press "Save As…"
key 'keystroke "g" using {command down, shift down}'
key "keystroke \"$work/elsewhere\""
key 'key code 36'
key 'key code 36'
waits "elsewhere/t.md"
(printf 'x' >"$work/slow.png" &)
waits "the file moved while images copied"
! grep -q "slow" "$work/elsewhere/t.md" || fail "a link to the old folder: $(cat "$work/elsewhere/t.md")"
kill "$app"
wait "$app" 2>/dev/null || true
app=
! grep -q "panicked" "$work/err" || fail "panics: $(grep -m3 panicked "$work/err")"
# Two tabs; typing a replacement keeps the hit.
printf 'alpha beta alpha beta alpha\n' >"$work/notes/v.md"
printf 'alpha beta\n' >"$work/notes/w.md"
cp "$work/red pic.png" "$work/a]b.png"
launch "$work/notes/v.md" "$work/notes/w.md"
waits 'mark: shown as "visual"'
osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app) to true" -e 'delay 2' >/dev/null
press v.md
sleep 1
key 'keystroke "f" using command down'
key 'keystroke "alpha"'
press "Toggle replace"
field 2
key 'keystroke "XY"'
press Replace
saves
[ "$(cat "$work/notes/v.md")" = "XY beta alpha beta alpha" ] || fail "the hit moved: $(cat "$work/notes/v.md")"
# Each tab follows the box, not its last query.
press w.md
field 1
key 'keystroke "a" using command down'
key 'keystroke "beta"'
field 2
key 'keystroke "a" using command down'
key 'keystroke "Y"'
press v.md
field 1
key 'keystroke "a" using command down'
key 'keystroke "alpha"'
field 2
key 'keystroke "a" using command down'
key 'keystroke "Z"'
press w.md
press "Replace all"
saves
[ "$(cat "$work/notes/w.md")" = "Z beta" ] || fail "a stale query: $(cat "$work/notes/w.md")"
# Alt text in source closes no brackets.
press Source
waits 'mark: shown as "source"'
window=$("$work/ax-values" "$app" | awk -F'\t' '$1 == "AXWindow" { sub("at=", "", $5); print $5; exit }')
IFS=, read -r x y <<<"$window"
[ "$("$work/drop" "$work/a]b.png" "$x" "$y")" = dropped ] || fail "the bracket drop was refused"
waits "mark: attached 1 images"
saves
grep -qF '![a\]b](assets/a%5Db.png)' "$work/notes/w.md" || fail "alt escape: $(cat "$work/notes/w.md")"
kill "$app"
wait "$app" 2>/dev/null || true
app=
! grep -q "panicked" "$work/err" || fail "panics: $(grep -m3 panicked "$work/err")"
echo "ok: mark tools"
