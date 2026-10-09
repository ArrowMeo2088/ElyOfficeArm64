#!/usr/bin/env bash
# No LibreOffice app may appear in Open With.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
[ "$(uname)" = Darwin ] || { echo "SKIP lo-unregistered: macOS only"; exit 0; }
fail() { echo "FAIL: $*" >&2; exit 1; }
app=$root/engines/lo-src/instdir/LibreOffice.app
if [ -d "$app" ]; then
  for key in CFBundleDocumentTypes CFBundleURLTypes UTExportedTypeDeclarations UTImportedTypeDeclarations; do
    ! /usr/libexec/PlistBuddy -c "Print :$key" "$app/Contents/Info.plist" >/dev/null 2>&1 || fail "$app claims $key"
  done
  [ ! -e "$app/Contents/PlugIns" ] || fail "$app keeps its Quick Look plug-ins"
fi
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
for ext in docx xlsx pptx odt csv; do : >"$work/sample.$ext"; done
cat >"$work/open.swift" <<'SWIFT'
import AppKit
for path in CommandLine.arguments.dropFirst() {
  for app in NSWorkspace.shared.urlsForApplications(toOpen: URL(fileURLWithPath: path)) { print(app.path) }
}
SWIFT
found=$(swift "$work/open.swift" "$work"/sample.* | grep -i libreoffice || true)
[ -z "$found" ] || fail "Open With lists LibreOffice: $found"
echo "ok: no LibreOffice in Open With"
