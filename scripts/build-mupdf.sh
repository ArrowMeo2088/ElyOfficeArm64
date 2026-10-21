#!/usr/bin/env bash
# Builds the pinned MuPDF, with patches/mupdf, into engines/mupdf-build.
# On Windows the arch is the one CARGO_BUILD_TARGET names: x64 or arm64.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
src=$root/engines/mupdf-src
out=$root/engines/mupdf-build
jobs=$(sysctl -n hw.ncpu 2>/dev/null || nproc)

# reference/ stays read-only; the copy takes the patches.
mkdir -p "$src"
case $(uname -s) in
  MINGW* | MSYS*)
    rm -rf "$src" && mkdir -p "$src"
    # A checkout here writes real symlinks, and zxing's libzint links are broken,
    # so tar could not recreate them. A Windows checkout of theirs keeps their
    # target text instead, and MSVC compiles none of them: leave them out.
    links=$root/engines/mupdf-links.txt
    (cd "$root/reference/mupdf" && find . -type l >"$links")
    tar -C "$root/reference/mupdf" --exclude .git --exclude-from="$links" -cf - . |
      tar -C "$src" -xf -
    ;;
  *) rsync -a --delete --exclude .git "$root/reference/mupdf/" "$src/" ;;
esac
for patch in "$root"/patches/mupdf/*.patch; do
  patch -d "$src" -p1 --forward --batch --quiet <"$patch"
done
case $(uname -s) in
  MINGW* | MSYS*)
    # The MSVC build follows the Rust target cargo builds for; x64 unless told.
    # The solution names its platform ARM64, the triplet and Rust say arm64.
    case ${CARGO_BUILD_TARGET:-x86_64-pc-windows-msvc} in
      x86_64-pc-windows-msvc)
        arch=x64
        platform=x64
        ;;
      aarch64-pc-windows-msvc)
        arch=arm64
        platform=ARM64
        ;;
      *)
        echo "build-mupdf.sh: no MSVC build for $CARGO_BUILD_TARGET" >&2
        exit 1
        ;;
    esac
    triplet=$arch-windows-static
    # MSVC: the static runtime Rust links, OpenSSL from vcpkg.
    ssl=$(cygpath -w "$root/engines/vcpkg/installed/$triplet")
    [ -f "$ssl/lib/libcrypto.lib" ] || {
      echo "build-mupdf.sh: no static OpenSSL; run engines/vcpkg/vcpkg install openssl:$triplet" >&2
      exit 1
    }
    cat >"$src/ely.props" <<PROPS
<Project xmlns="http://schemas.microsoft.com/developer/msbuild/2003">
  <ItemDefinitionGroup>
    <ClCompile>
      <RuntimeLibrary>MultiThreaded</RuntimeLibrary>
      <PreprocessorDefinitions>HAVE_LIBCRYPTO;%(PreprocessorDefinitions)</PreprocessorDefinitions>
      <AdditionalIncludeDirectories>$ssl\include;%(AdditionalIncludeDirectories)</AdditionalIncludeDirectories>
    </ClCompile>
  </ItemDefinitionGroup>
</Project>
PROPS
    msbuild=$("/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe" -latest -find 'MSBuild\**\Bin\MSBuild.exe' | tr -d '\r')
    "$msbuild" "$(cygpath -w "$src/platform/win32/mupdf.sln")" -m -nologo -v:minimal \
      -t:libmupdf -p:Configuration=Release -p:Platform=$platform -p:PlatformToolset=v143 \
      -p:ForceImportAfterCppTargets="$(cygpath -w "$src/ely.props")"
    rm -rf "$out" && mkdir -p "$out"
    cp "$src"/platform/win32/$platform/Release/*.lib "$out/"
    ls "$out"
    exit 0
    ;;
esac
flags="build=release HAVE_X11=no HAVE_GLUT=no HAVE_CURL=no HAVE_LIBCRYPTO=yes tesseract=yes"
# make does not track flags; a change rebuilds all.
if [ "$(cat "$out/.flags" 2>/dev/null)" != "$flags" ]; then
  rm -rf "$out"
fi
# shellcheck disable=SC2086
make -C "$src" OUT="$out" $flags -j"$jobs" libs "$out/libmupdf-pkcs7.a"
echo "$flags" >"$out/.flags"
