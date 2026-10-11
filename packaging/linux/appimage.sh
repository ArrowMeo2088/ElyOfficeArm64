#!/usr/bin/env bash
# Builds dist/<Name>-<version>-linux-<arch>.AppImage. Usage: appimage.sh [--desktop] <slug>
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
desktop_only=
[ "${1:-}" = --desktop ] && {
  desktop_only=1
  shift
}
slug=${1:?app slug, such as ely-doc}
dist=${ELY_DIST:-$root/dist}

row=$(awk -F'\t' -v slug="$slug" '$1 == slug { print $2 "\t" $3 }' "$root/packaging/apps.tsv")
[ -n "$row" ] || {
  echo "appimage.sh: unknown app $slug" >&2
  exit 1
}
name=${row%%$'\t'*}
categories=${row##*$'\t'}
# The types it opens, from packaging/types.tsv; none drops the line.
mimes=$(awk -F'\t' -v slug="$slug" '$1 == slug { printf "%s;", $3 }' "$root/packaging/types.tsv")
desktop_entry() {
  sed -e "s/@NAME@/$name/" -e "s/@SLUG@/$slug/g" -e "s/@CATEGORIES@/$categories/" -e "s|@MIME@|$mimes|" \
    -e '/^MimeType=$/d' "$root/packaging/linux/app.desktop.in"
}

if [ -n "$desktop_only" ]; then
  desktop_entry
  exit 0
fi

[ "$(uname)" = Linux ] || {
  echo "appimage.sh: Linux only" >&2
  exit 1
}
arch=$(uname -m)
case $arch in
  x86_64)
    pack_sha=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
    deploy_sha=c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
    runtime_sha=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d
    ;;
  aarch64)
    pack_sha=f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158
    deploy_sha=620095110d693282b8ebeb244a95b5e911cf8f65f76c88b4b47d16ae6346fcff
    runtime_sha=00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444
    ;;
  *)
    echo "appimage.sh: no AppImage tools for $arch" >&2
    exit 1
    ;;
esac
icon=$root/assets/icons/$slug.png
[ -f "$icon" ] || {
  echo "appimage.sh: $icon is missing; the icons are not designed yet" >&2
  exit 1
}

fetch_tool() { # file url sha256
  if [ ! -f "$1" ]; then
    mkdir -p "$root/engines"
    curl -fsSL -o "$1" "$2"
  fi
  echo "$3  $1" | sha256sum -c - >/dev/null
  chmod +x "$1"
}
pack=$root/engines/appimagetool-1.9.1-$arch.AppImage
deploy=$root/engines/linuxdeploy-1-alpha-20251107-1-$arch.AppImage
runtime=$root/engines/type2-runtime-20251108-$arch
fetch_tool "$pack" "https://github.com/AppImage/appimagetool/releases/download/1.9.1/appimagetool-$arch.AppImage" "$pack_sha"
fetch_tool "$deploy" "https://github.com/linuxdeploy/linuxdeploy/releases/download/1-alpha-20251107-1/linuxdeploy-$arch.AppImage" "$deploy_sha"
fetch_tool "$runtime" "https://github.com/AppImage/type2-runtime/releases/download/20251108/runtime-$arch" "$runtime_sha"

stage=$root/target/stage/$slug
"$root/packaging/stage.sh" "$slug" "$stage"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
desktop_entry >"$work/$slug.desktop"
appdir=$work/$name.AppDir
bin=$appdir/usr/bin
mkdir -p "$bin"
cp -R "$stage"/. "$bin/"
deployed=(--executable "$bin/$slug")
for helper in ely-lok-worker ely-pdf-worker; do
  if [ -f "$bin/$helper" ]; then
    deployed+=(--executable "$bin/$helper")
  fi
done
if [ -f "$bin/libcef.so" ]; then
  # Binaries here load from usr/lib; CEF's data sits with libcef.
  cef=${ELY_CEF:-$root/engines/cef}
  mkdir -p "$appdir/usr/lib"
  for item in "$bin"/*; do
    if [ -e "$cef/${item##*/}" ]; then
      mv "$item" "$appdir/usr/lib/"
    fi
  done
  deployed+=(--library "$appdir/usr/lib/libcef.so")
fi

# linuxdeploy bundles libraries and fails on a missing one.
LD_LIBRARY_PATH=$appdir/usr/lib "$deploy" --appimage-extract-and-run --appdir "$appdir" "${deployed[@]}" \
  --desktop-file "$work/$slug.desktop" --icon-file "$icon"

mkdir -p "$dist"
version=$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")
out=$dist/$name-$version-linux-$arch.AppImage
ARCH=$arch "$pack" --appimage-extract-and-run --runtime-file "$runtime" "$appdir" "$out"
echo "$out"
