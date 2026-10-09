#!/usr/bin/env bash
# Builds the pinned MuPDF, with patches/mupdf, into engines/mupdf-build.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
src=$root/engines/mupdf-src
out=$root/engines/mupdf-build
jobs=$(sysctl -n hw.ncpu 2>/dev/null || nproc)

# reference/ stays read-only; the copy takes the patches.
mkdir -p "$src"
rsync -a --delete --exclude .git "$root/reference/mupdf/" "$src/"
for patch in "$root"/patches/mupdf/*.patch; do
  patch -d "$src" -p1 --forward --batch --quiet <"$patch"
done
flags="build=release HAVE_X11=no HAVE_GLUT=no HAVE_CURL=no HAVE_LIBCRYPTO=yes tesseract=yes"
# make does not track flags; a change rebuilds all.
if [ "$(cat "$out/.flags" 2>/dev/null)" != "$flags" ]; then
  rm -rf "$out"
fi
# shellcheck disable=SC2086
make -C "$src" OUT="$out" $flags -j"$jobs" libs "$out/libmupdf-pkcs7.a"
echo "$flags" >"$out/.flags"
