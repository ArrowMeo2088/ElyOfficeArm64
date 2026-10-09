#!/usr/bin/env bash
# Builds the pinned Git, relocatable, into engines/git.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
src=$root/engines/git-src
out=$root/engines/git
jobs=$(sysctl -n hw.ncpu 2>/dev/null || nproc)

# reference/ stays read-only; the build runs in a copy.
mkdir -p "$src"
rsync -a --delete --exclude .git "$root/reference/git/" "$src/"
# Relocatable; built-ins are symlinks, so bundles copy one binary.
flags="prefix=$out RUNTIME_PREFIX=YesPlease INSTALL_SYMLINKS=YesPlease NO_GETTEXT=YesPlease NO_TCLTK=YesPlease NO_PERL=YesPlease NO_PYTHON=YesPlease"
# The bundles say macOS 14; Git must not need newer.
[ "$(uname)" = Darwin ] && export MACOSX_DEPLOYMENT_TARGET=14.0 && flags="$flags MACOSX_DEPLOYMENT_TARGET=14.0"
if [ "$(cat "$out/.flags" 2>/dev/null)" != "$flags" ]; then
  rm -rf "$out"
fi
# shellcheck disable=SC2086
make -C "$src" $flags -j"$jobs" install
echo "$flags" >"$out/.flags"
"$out/bin/git" --version
