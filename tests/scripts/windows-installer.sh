#!/usr/bin/env bash
# Installer upgrades remove only owned files; busy files change nothing.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
makensis=${MAKENSIS:-"/c/Program Files (x86)/NSIS/makensis.exe"}
case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) ;; *) echo "SKIP: not Windows"; exit 0 ;; esac
[ -x "$makensis" ] || { echo "SKIP: no makensis"; exit 0; }
export MSYS2_ARG_CONV_EXCL='*'

work=$root/target/installer-test
key='HKCU\Software\Ely Office\ElyProbe'
uninstall_key='HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\Ely Office ElyProbe'
clean() {
  reg delete "$key" /f >/dev/null 2>&1 || true
  reg delete "$uninstall_key" /f >/dev/null 2>&1 || true
  rm -f "$APPDATA/Microsoft/Windows/Start Menu/Programs/ElyProbe.lnk"
}
trap 'kill $(jobs -p) 2>/dev/null || true; clean' EXIT
clean
rm -rf "$work"
mkdir -p "$work"
fail() { echo "FAIL: $*" >&2; exit 1; }

# Two releases of a fake app; v2 adds extra.txt.
build() {
  local version=$1 stage=$work/stage-$1
  mkdir -p "$stage/licenses" "$stage/data"
  cp /c/Windows/System32/PING.EXE "$stage/ely-probe.exe"
  echo "$version" >"$stage/ely-probe-worker.exe"
  echo "$version" >"$stage/licenses/LICENSE"
  echo "$version" >"$stage/data/info.txt"
  [ "$version" = 1 ] || echo "$version" >"$stage/extra.txt"
  printf '!macro ElyTypesInstall\n!macroend\n!macro ElyTypesUninstall\n!macroend\n' >"$work/types.nsh"
  "$root/packaging/windows/files.sh" "$stage" ely-probe >"$work/files-$version.nsh"
  "$makensis" -V1 -INPUTCHARSET UTF8 -DAPP_NAME=ElyProbe -DAPP_SLUG=ely-probe -DVERSION="$version" \
    -DSTAGE="$(cygpath -w "$stage")" -DICON="$(cygpath -w "$root/assets/icons/ely-mark.ico")" \
    -DTYPES="$(cygpath -w "$work/types.nsh")" -DFILES="$(cygpath -w "$work/files-$version.nsh")" \
    -DOUTFILE="$(cygpath -w "$work/setup-$version.exe")" "$(cygpath -w "$root/packaging/windows/installer.nsi")"
}
build 1
build 2

install() { "$work/setup-$1.exe" /S "/D=$(cygpath -w "$2")" && echo 0 || echo $?; }
snapshot() { (cd "$1" && find . -type f -exec md5sum {} + | sort); }
put() { mkdir -p "$(dirname "$1")" && echo user >"$1"; }
must() { [ -e "$1" ] || fail "$2: $1 is gone"; }
mustnt() { [ ! -e "$1" ] || fail "$2: $1 is still there"; }
# Holds a file open without delete or write sharing.
hold() {
  rm -f "$work/held"
  powershell -NoProfile -Command "\$f=[IO.File]::Open('$(cygpath -w "$1")','Open','Read','Read'); \
    New-Item '$(cygpath -w "$work/held")' | Out-Null; Start-Sleep 60" &
  until [ -e "$work/held" ]; do sleep 0.2; done
}

# A chosen folder's own files survive install and reinstall.
dir=$work/stranger
for f in keep.exe keep.dll keep.json git/keep.txt notes/note.md; do put "$dir/$f"; done
[ "$(install 1 "$dir")" = 0 ] || fail "stranger install"
[ "$(install 2 "$dir")" = 0 ] || fail "stranger reinstall"
for f in keep.exe keep.dll keep.json git/keep.txt notes/note.md; do must "$dir/$f" stranger; done
must "$dir/extra.txt" "stranger reinstall"
grep -q '^2$' "$dir/licenses/LICENSE" || fail "stranger reinstall kept the old licence"

# Older releases are cleaned only where our key points.
legacy() {
  for f in ely-probe.exe libcef.dll chrome_100.pak old.json git/cmd/git.exe locales/en.pak \
    notes/note.md keep.md; do put "$1/$f"; done
}
dir=$work/unkeyed
legacy "$dir"
[ "$(install 2 "$dir")" = 0 ] || fail "unkeyed install"
must "$dir/git/cmd/git.exe" "unkeyed folder"
dir=$work/legacy
legacy "$dir"
reg add "$key" /v InstallDir /d "$(cygpath -w "$dir")" /f >/dev/null
[ "$(install 2 "$dir")" = 0 ] || fail "legacy upgrade"
for f in libcef.dll chrome_100.pak old.json git locales; do mustnt "$dir/$f" "legacy upgrade"; done
for f in notes/note.md keep.md extra.txt; do must "$dir/$f" "legacy upgrade"; done

# Busy files: the installer refuses; the old install stays whole.
busy() {
  local name=$1 dir=$2 file=$3 before
  before=$(snapshot "$dir")
  hold "$file"
  [ "$(install 2 "$dir")" = 2 ] || fail "$name: installer did not refuse"
  kill %% 2>/dev/null; wait 2>/dev/null || true
  [ "$(snapshot "$dir")" = "$before" ] || fail "$name: the old install changed"
  mustnt "$dir/.ely-old" "$name"
}
dir=$work/busy
[ "$(install 1 "$dir")" = 0 ] || fail "busy setup"
busy "held program" "$dir" "$dir/ely-probe-worker.exe"
busy "held file in a folder" "$dir" "$dir/licenses/LICENSE"
dir=$work/busy-legacy
legacy "$dir"
reg add "$key" /v InstallDir /d "$(cygpath -w "$dir")" /f >/dev/null
busy "held legacy git" "$dir" "$dir/git/cmd/git.exe"

dir=$work/running
[ "$(install 1 "$dir")" = 0 ] || fail "running setup"
before=$(snapshot "$dir")
"$dir/ely-probe.exe" -n 30 127.0.0.1 >/dev/null &
sleep 1
[ "$(install 2 "$dir")" = 2 ] || fail "running: installer did not refuse"
kill %% 2>/dev/null; wait 2>/dev/null || true
[ "$(snapshot "$dir")" = "$before" ] || fail "running: the old install changed"

# A failed copy puts the old release back.
dir=$work/rollback
[ "$(install 1 "$dir")" = 0 ] || fail "rollback setup"
put "$dir/extra.txt/blocker"
before=$(snapshot "$dir")
[ "$(install 2 "$dir")" != 0 ] || fail "rollback: blocked copy succeeded"
[ "$(snapshot "$dir")" = "$before" ] || fail "rollback: the old install changed"
mustnt "$dir/.ely-old" rollback

# Uninstalling removes only what was installed.
dir=$work/stranger
cp "$dir/uninstall.exe" "$work/uninstall.exe"
"$work/uninstall.exe" /S "_?=$(cygpath -w "$dir")" || fail "uninstall"
for f in keep.exe keep.dll keep.json git/keep.txt notes/note.md; do must "$dir/$f" uninstall; done
for f in ely-probe.exe ely-probe-worker.exe extra.txt licenses data uninstall.exe .ely-files; do
  mustnt "$dir/$f" uninstall
done
rm -rf "$work"
echo "PASS: windows installer"
