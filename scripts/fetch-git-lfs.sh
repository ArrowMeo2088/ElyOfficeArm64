#!/usr/bin/env bash
# git-lfs at one release, checked, beside Git's own commands.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
exec_dir=$root/engines/git/libexec/git-core
version=v3.8.0
case "$(uname -s)-$(uname -m)" in
  Darwin-arm64) asset=git-lfs-darwin-arm64-$version.zip sum=caff76a7d070d8160c89bc39b6e85d98f24135b6fed038a3b4de2590d25102d8 ;;
  Darwin-x86_64) asset=git-lfs-darwin-amd64-$version.zip sum=f1c17aeca0b4eaab9ea606226477dbed3b84b56fe0811a9f967d2ea2b2393c53 ;;
  Linux-x86_64) asset=git-lfs-linux-amd64-$version.tar.gz sum=e455e00f15d9b95661b8d53498ffb0c3367962cf1ec73c31ab7369516cd6ab8d ;;
  Linux-aarch64) asset=git-lfs-linux-arm64-$version.tar.gz sum=ac9c8efac980bb0505ead384d087e2acb6486fd8498691a2165fa174ec6118c2 ;;
  *) echo "fetch-git-lfs: no pinned build for $(uname -s)-$(uname -m)" >&2; exit 1 ;;
esac
[ -d "$exec_dir" ] || { echo "fetch-git-lfs: build Git first (scripts/build-git.sh)" >&2; exit 1; }
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
curl -fsSL -o "$work/$asset" "https://github.com/git-lfs/git-lfs/releases/download/$version/$asset"
got=$(shasum -a 256 "$work/$asset" | cut -d' ' -f1)
if [ "$got" != "$sum" ]; then
  echo "fetch-git-lfs: $asset hash $got, want $sum" >&2
  exit 1
fi
case "$asset" in
  *.zip) unzip -q "$work/$asset" -d "$work/x" ;;
  *) mkdir "$work/x" && tar -xzf "$work/$asset" -C "$work/x" ;;
esac
install -m 755 "$(find "$work/x" -type f -name git-lfs | head -1)" "$exec_dir/git-lfs"
# The archive has no licence; the tag's own, checked.
curl -fsSL -o "$work/LICENSE.md" "https://raw.githubusercontent.com/git-lfs/git-lfs/$version/LICENSE.md"
got=$(shasum -a 256 "$work/LICENSE.md" | cut -d' ' -f1)
[ "$got" = 4fae9062ab5cdd5fb15486b728534d8aded8b4ae9d84b6d66a956f5162c366b6 ] || {
  echo "fetch-git-lfs: LICENSE.md hash $got" >&2
  exit 1
}
mkdir -p "$root/engines/git/share/doc/git-lfs"
install -m 644 "$work/LICENSE.md" "$root/engines/git/share/doc/git-lfs/LICENSE.md"
"$root/engines/git/bin/git" lfs version
