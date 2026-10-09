#!/usr/bin/env bash
# Writes an app's licences and notices. Usage: licenses.sh <slug> <dir>
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
slug=${1:?app slug}
out=${2:?licence folder}
put() { # into, files...
  local into=$out/$1
  shift
  mkdir -p "$into"
  for file in "$@"; do
    [ -s "$file" ] || {
      echo "licenses.sh: no $file; the release cannot ship without it" >&2
      exit 1
    }
    cp "$file" "$into/"
  done
}
put . "$root/LICENSE"
case $slug in
  ely-doc | ely-table | ely-show) crates=(ely-lok) ;;
  ely-pdf) crates=(ely-pdf-engine) ;;
  ely-mark) crates=(ely-web-content) ;;
  *) crates=() ;;
esac
python3 "$root/packaging/crates.py" "$slug" ${crates[@]+"${crates[@]}"} >"$out/rust-crates.txt"
lo=${ELY_LO_APP:-$root/engines/lo-src/instdir/LibreOffice.app}/Contents/Resources
case $slug in
  ely-doc | ely-table | ely-show)
    put LibreOffice "$lo/LICENSE" "$lo/LICENSE.html" "$lo/NOTICE" "$lo/CREDITS.fodt"
    ;;
  ely-pdf)
    put MuPDF "$root/engines/mupdf-src/COPYING"
    # Every third party the MuPDF build compiled in.
    for part in "$root"/engines/mupdf-build/thirdparty/*; do
      name=${part##*/}
      found=$(find "$root/engines/mupdf-src/thirdparty/$name" -maxdepth 1 -type f \
        \( -iname 'licen[cs]e*' -o -iname 'copying*' -o -iname '*-license.txt' \))
      [ "$name" = libjpeg ] && found=$root/engines/mupdf-src/thirdparty/libjpeg/README
      [ -n "$found" ] || { echo "licenses.sh: no licence for MuPDF's $name" >&2; exit 1; }
      # shellcheck disable=SC2086
      put "MuPDF/$name" $found
    done
    # Every font family MuPDF built into the worker.
    for part in "$root"/engines/mupdf-build/generated/resources/fonts/*; do
      name=${part##*/}
      fonts=$root/engines/mupdf-src/resources/fonts/$name
      found=$(find "$fonts" -maxdepth 1 -type f \( -iname 'licen[cs]e*' -o -iname 'copying*' -o -iname 'notice*' -o -iname 'ofl*' \))
      [ -n "$found" ] || { echo "licenses.sh: no licence for MuPDF's $name fonts" >&2; exit 1; }
      # A README tells the fonts' source; never the licence.
      found="$found $(find "$fonts" -maxdepth 1 -type f -iname 'readme*')"
      # shellcheck disable=SC2086
      put "MuPDF/fonts/$name" $found
    done
    put OpenSSL "$(pkg-config --variable=prefix libcrypto)/LICENSE.txt"
    put tessdata "${ELY_TESSDATA:-$root/engines/tessdata}/LICENSE"
    put GreatVibes "$root/crates/ely-pdf-engine/fonts/OFL.txt"
    ;;
  ely-tree)
    put Git "$root/engines/git-src/COPYING" "$root/engines/git-src/LGPL-2.1"
    put git-lfs "$root/engines/git/share/doc/git-lfs/LICENSE.md"
    ;;
  ely-mark)
    put CEF "$root/reference/cef/LICENSE.txt" "${ELY_CEF:-$root/engines/cef}/CREDITS.html"
    put content "$root/engines/content/ely-mark/THIRD-PARTY-NOTICES.txt"
    ;;
esac
