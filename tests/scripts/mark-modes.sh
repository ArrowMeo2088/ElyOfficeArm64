#!/usr/bin/env bash
# MARK-01 to 04 live: modes, images, new and rename.
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
swiftc -O -o "$work/share" "$root/tests/scripts/window-share.swift"
# A magenta picture beside the document.
python3 - "$work/pic.png" <<'PY'
import struct, sys, zlib
w, h = 240, 120
rows = b"".join(b"\0" + b"\xff\x00\xaa" * w for _ in range(h))
def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
png = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
png += chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b"")
open(sys.argv[1], "wb").write(png)
PY
text=$'---\ntitle: Modes\n---\n# Modes\n\nFirst line.\n\n![pic](pic.png)\n\n- [ ] task\n\nLast line.\n'
printf '%s' "$text" >"$work/doc.md"
cp "$work/doc.md" "$work/orig.md"
ELY_LOG=info ELY_LOCALE=en ELY_NO_DIALOG=1 HOME="$work" XDG_DATA_HOME="$work/data" \
  "$root/target/debug/ely-mark" "$work/doc.md" >"$work/out" 2>"$work/err" &
app=$!
shows() { grep -c "mark: shown as" "$work/err" || true; }
# Waits for show number `n`, as `mode`.
wait_shown() {
  for _ in $(seq 120); do
    [ "$(shows)" -ge "$1" ] && break
    kill -0 "$app" 2>/dev/null || fail "ElyMark quit: $(tail -n 5 "$work/err")"
    sleep 0.25
  done
  [ "$(grep "mark: shown as" "$work/err" | sed -n "$1p" | grep -c "\"$2\"")" = 1 ] ||
    fail "show $1 is not $2: $(grep 'mark: shown' "$work/err")"
}
key() { osascript -e "tell application \"System Events\" to $1" -e 'delay 0.8' >/dev/null; }
saved() {
  local before
  before=$(grep -c "office: saved" "$work/err" || true)
  key 'keystroke "s" using command down'
  for _ in $(seq 40); do
    [ "$(grep -c "office: saved" "$work/err" || true)" -gt "$before" ] && return 0
    sleep 0.25
  done
  fail "no save"
}
same() { cmp -s "$work/orig.md" "$1" || fail "$2: $(cat "$1")"; }
wait_shown 1 visual
osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $app) to true" -e 'delay 1' >/dev/null
share=$("$work/share" "$app" 255,0,170)
case $share in denied) fail "needs Screen Recording permission" ;; none) fail "no window" ;; esac
awk -v s="$share" 'BEGIN { exit !(s > 0.003) }' || fail "the picture beside the file did not show ($share)"
# Switching alone changes nothing.
key 'keystroke "2" using command down'
wait_shown 2 source
key 'keystroke "3" using command down'
wait_shown 3 preview
key 'keystroke "1" using command down'
wait_shown 4 visual
saved
same "$work/doc.md" "switching modes changed the file"
# Preview takes no typing.
key 'keystroke "3" using command down'
wait_shown 5 preview
key 'keystroke "z"'
key 'keystroke "1" using command down'
wait_shown 6 visual
saved
same "$work/doc.md" "preview took typing"
# Visual edits undo after a trip through preview.
key 'keystroke "y"'
key 'keystroke "3" using command down'
wait_shown 7 preview
key 'keystroke "1" using command down'
wait_shown 8 visual
key 'keystroke "z" using command down'
wait_shown 9 visual
saved
same "$work/doc.md" "undo did not survive the trip"
# A source edit shows in visual and saves.
key 'keystroke "2" using command down'
wait_shown 10 source
key 'keystroke "x"'
key 'keystroke "1" using command down'
wait_shown 11 visual
saved
[ "$(cat "$work/doc.md"; echo .)" = "x$text." ] || fail "source edit: $(cat "$work/doc.md")"
# One timeline: undo walks back through both modes, redo returns.
key 'keystroke "v"'
key 'keystroke "2" using command down'
wait_shown 12 source
key 'keystroke "s"'
key 'keystroke "1" using command down'
wait_shown 13 visual
key 'keystroke "z" using command down'
wait_shown 14 visual
key 'keystroke "z" using command down'
wait_shown 15 visual
saved
[ "$(cat "$work/doc.md"; echo .)" = "x$text." ] || fail "undo across modes: $(cat "$work/doc.md")"
key 'keystroke "z" using {command down, shift down}'
wait_shown 16 visual
saved
[ "$(cat "$work/doc.md"; echo .)" = "vx$text." ] || fail "redo across modes: $(cat "$work/doc.md")"
key 'keystroke "z" using command down'
wait_shown 17 visual
saved
# Rename refuses a taken name, moves to a free one.
printf 'other\n' >"$work/taken.md"
key 'key code 120'
key 'keystroke "a" using command down'
key 'keystroke "taken.md"'
key 'key code 36'
for _ in $(seq 20); do grep -q "did not rename" "$work/err" && break; sleep 0.25; done
grep -q "did not rename" "$work/err" || fail "a rename onto a file went through"
[ "$(cat "$work/taken.md")" = other ] || fail "the taken file changed"
key 'key code 120'
key 'keystroke "a" using command down'
key 'keystroke "moved.md"'
key 'key code 36'
for _ in $(seq 20); do grep -q "renamed .*moved.md" "$work/err" && break; sleep 0.25; done
[ -f "$work/moved.md" ] && [ ! -e "$work/doc.md" ] || fail "rename: $(ls "$work")"
recent=$(find "$work/Library" -name recent.json)
grep -q 'moved.md' "$recent" && ! grep -q 'doc.md' "$recent" || fail "recent after rename: $(cat "$recent")"
key 'keystroke "w"'
saved
[ "$(cat "$work/moved.md"; echo .)" = "wx$text." ] || fail "save after rename: $(cat "$work/moved.md")"
# Undone steps still redo after a mode trip.
key 'keystroke "2" using command down'
wait_shown 18 source
key 'keystroke "q"'
key 'keystroke "z" using command down'
key 'keystroke "3" using command down'
wait_shown 19 preview
key 'keystroke "2" using command down'
wait_shown 20 source
key 'keystroke "z" using {command down, shift down}'
wait_shown 21 source
saved
[ "$(cat "$work/moved.md"; echo .)" = "qwx$text." ] || fail "source redo after a trip: $(cat "$work/moved.md")"
key 'keystroke "1" using command down'
wait_shown 22 visual
key 'keystroke "p"'
key 'keystroke "z" using command down'
key 'keystroke "3" using command down'
wait_shown 23 preview
key 'keystroke "1" using command down'
wait_shown 24 visual
key 'keystroke "z" using {command down, shift down}'
wait_shown 25 visual
saved
[ "$(cat "$work/moved.md"; echo .)" = "pqwx$text." ] || fail "visual redo after a trip: $(cat "$work/moved.md")"
# New opens an empty visual document.
key 'keystroke "n" using command down'
wait_shown 26 visual
fresh=$(find "$work/Library" -name untitled.md)
[ -n "$fresh" ] && [ ! -s "$fresh" ] || fail "the new document is not empty: $fresh"

key 'keystroke "q" using command down'
key 'key code 36'
for _ in $(seq 40); do kill -0 "$app" 2>/dev/null || break; sleep 0.25; done
app=
! grep -q "panicked" "$work/err" || fail "panics: $(grep -m3 panicked "$work/err")"
echo "ok: mark modes (picture share $share)"
