#!/usr/bin/env bash
# Usage: release.sh [batch]; a dmg per app of the batch.
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
batch=${1:-1}
dist=${ELY_DIST:-$root/dist}
version=$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")
arch=$(uname -m)
[ -z "${ELY_NOTARY_PROFILE:-}" ] || [ -n "${ELY_SIGN_IDENTITY:-}" ] || {
  echo "release.sh: notarizing needs ELY_SIGN_IDENTITY, a Developer ID" >&2
  exit 1
}
# Our source as published: the committed tree.
source=$dist/ElyOffice-$version-source.tar.gz
mkdir -p "$dist"
git -C "$root" archive --format=tar.gz --prefix="ElyOffice-$version/" -o "$source" HEAD
echo "$source"
# Copyleft upstream as shipped: patched MuPDF (AGPL), Git (GPLv2).
upstream=$dist/ElyOffice-$version-upstream-source.tar.gz
[ -f "$root/engines/mupdf-src/COPYING" ] || { echo "release.sh: no engines/mupdf-src; run the MuPDF build" >&2; exit 1; }
tar -czf "$upstream" --exclude .git --exclude .DS_Store \
  -C "$root/engines" mupdf-src -C "$root/reference" git
echo "$upstream"
awk -F'\t' -v batch="$batch" '$4 == batch { print $1 "\t" $2 }' "$root/packaging/apps.tsv" |
  while IFS=$'\t' read -r slug name; do
    "$root/packaging/macos/bundle.sh" "$slug" release
    codesign --verify --deep --strict "$dist/$name.app"
    dmg=$dist/$name-$version-$arch.dmg
    rm -f "$dmg"
    hdiutil create -quiet -volname "$name" -srcfolder "$dist/$name.app" -format UDZO "$dmg"
    if [ -n "${ELY_NOTARY_PROFILE:-}" ]; then
      codesign --force --timestamp --sign "$ELY_SIGN_IDENTITY" "$dmg"
      xcrun notarytool submit "$dmg" --keychain-profile "$ELY_NOTARY_PROFILE" --wait
      xcrun stapler staple "$dmg"
    else
      echo "release.sh: $dmg is not notarized (no ELY_NOTARY_PROFILE)" >&2
    fi
    echo "$dmg"
  done
rm -rf "$root/target/release/incremental"
