#!/usr/bin/env bash
# Builds the pinned LibreOffice with the macOS LOK patches.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
src=$root/engines/lo-src
pin=$(awk '$1 == "fetch" && $2 == "libreoffice-core" { print $4 }' "$root/scripts/fetch-references.sh")
options=(
  --enable-bogus-pkg-config
  --enable-mergelibs
  --enable-release-build
  --without-java
  --without-junit
  --without-help
  --disable-odk
  --disable-online-update
  --disable-extension-integration
  --disable-scripting-beanshell
  --disable-scripting-javascript
  --disable-firebird-sdbc
  --disable-postgresql-sdbc
  --disable-ext-nlpsolver
  --disable-ext-wiki-publisher
  --disable-report-builder
  --disable-sdremote
  --disable-sdremote-bluetooth
  --disable-poppler
  --disable-lotuswordpro
  --disable-gpgmepp
  --disable-cups
  --disable-evolution2
  --with-galleries=no
  --with-lang=
  --with-theme=colibre
  --with-fonts
  --disable-python
  --disable-coinmp
  --disable-lpsolve
  --without-doxygen
  --with-myspell-dicts
)

[ "$(uname)" = Darwin ] || {
  echo "build-libreoffice.sh: macOS only; Linux and Windows use the TDF builds" >&2
  exit 1
}
if [ ! -d "$src/.git" ]; then
  git init -q "$src"
  git -C "$src" remote add origin https://github.com/LibreOffice/core.git
  git -C "$src" fetch -q --depth 1 origin "$pin"
  git -C "$src" checkout -q FETCH_HEAD
fi
# The bundled spelling dictionaries (DOC-12).
git -C "$src" config submodule.dictionaries.url https://github.com/LibreOffice/dictionaries.git
git -C "$src" submodule update -q --init --depth 1 dictionaries
[ "$(git -C "$src" rev-parse HEAD)" = "$pin" ] || {
  echo "build-libreoffice.sh: $src is not at $pin" >&2
  exit 1
}
"$root/scripts/apply-lo-patches.sh" "$src"

for tool in make gnu-sed bison flex gettext; do
  prefix=$(brew --prefix "$tool")
  PATH=$prefix/libexec/gnubin:$prefix/bin:$PATH
done
export PATH PKG_CONFIG_LIBDIR=/var/empty PKG_CONFIG_PATH=

cd "$src"
if [ ! -f config_host.mk ] || ! printf '%s\n' "${options[@]}" | cmp -s - autogen.lastrun; then
  ./autogen.sh "${options[@]}"
fi
make -j"$(sysctl -n hw.ncpu)"

# Bundled dictionaries: English only; each one slows LOK start (PERF-01).
keep=(dict-en)
extensions=$src/instdir/LibreOffice.app/Contents/Resources/extensions
for dict in "$extensions"/dict-*; do
  name=$(basename "$dict")
  if [[ " ${keep[*]} " != *" $name "* ]]; then
    rm -rf "$dict"
  fi
done
echo "build-libreoffice.sh: dictionaries kept: $(ls "$extensions" | tr '\n' ' ')"
# Ely's kernel defaults: Excel A1 formulas.
cp "$root/packaging/libreoffice/ely.xcd" "$src/instdir/LibreOffice.app/Contents/Resources/registry/"

"$root/scripts/inert-lo.sh" "$src/instdir/LibreOffice.app"
