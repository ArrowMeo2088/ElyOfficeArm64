#!/usr/bin/env bash
# Prints the engines' key: a hash of every pin, patch and recipe.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
find scripts/fetch-references.sh scripts/apply-lo-patches.sh scripts/build-libreoffice.sh scripts/build-mupdf.sh \
  scripts/build-git.sh scripts/fetch-git-lfs.sh scripts/fetch-tessdata.sh scripts/fetch-cef.sh \
  packaging/libreoffice patches -type f | LC_ALL=C sort | xargs shasum -a 256 | shasum -a 256 | cut -c1-12
