#!/usr/bin/env bash
# Builds dist/<Name>.app. Usage: bundle.sh <slug> [release|debug]
set -euo pipefail

root=$(cd "$(dirname "$0")/../.." && pwd)
slug=${1:?app slug, such as ely-doc}
profile=${2:-release}
dist=${ELY_DIST:-$root/dist}
identity=${ELY_SIGN_IDENTITY:--}
prefix=ely.zacharyzhang.office

name=$(awk -F'\t' -v slug="$slug" '$1 == slug { print $2 }' "$root/packaging/apps.tsv")
[ -n "$name" ] || {
  echo "bundle.sh: unknown app $slug" >&2
  exit 1
}
version=$(awk -F'"' '/^version = / { print $2; exit }' "$root/Cargo.toml")
case $profile in
  release | debug) ;;
  *)
    echo "bundle.sh: profile is release or debug" >&2
    exit 1
    ;;
esac
build() { # cargo build arguments
  case $profile in
    release) cargo build --manifest-path "$root/Cargo.toml" --release "$@" ;;
    debug) cargo build --manifest-path "$root/Cargo.toml" "$@" ;;
  esac
}
uses() { grep -q "^$1" "$root/apps/$slug/Cargo.toml"; }
need() { # path, what to run
  [ -e "$1" ] || {
    echo "bundle.sh: no $1; run $2" >&2
    exit 1
  }
}
build -p "$slug"
icon=$root/assets/icons/$slug.icns
need "$icon" packaging/icons/build.sh

app=$dist/$name.app
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$root/target/$profile/$slug" "$app/Contents/MacOS/$slug"
cp "$icon" "$app/Contents/Resources/$slug.icns"
# Code inside the app, signed before it.
inner=()
if uses ely-lok; then
  lo=${ELY_LO_APP:-$root/engines/lo-src/instdir/LibreOffice.app}
  need "$lo/Contents/Frameworks" scripts/build-libreoffice.sh
  build -p ely-lok --bin ely-lok-worker
  cp "$root/target/$profile/ely-lok-worker" "$app/Contents/MacOS/"
  # The kernel alone, in LibreOffice's layout: nothing to register.
  mkdir -p "$app/Contents/Frameworks"
  ditto "$lo/Contents/Frameworks" "$app/Contents/Frameworks"
  ditto "$lo/Contents/Resources" "$app/Contents/Resources"
  inner+=("$app/Contents/MacOS/ely-lok-worker")
  while IFS= read -r binary; do
    if file -b "$binary" | grep -q '^Mach-O'; then
      inner+=("$binary")
    elif [ "$(cat "$binary")" = "invalid - merged lib" ]; then
      # Merged-library builds leave these placeholders.
      rm "$binary"
    else
      echo "bundle.sh: $binary is not code; Frameworks holds only code" >&2
      exit 1
    fi
  done < <(find "$app/Contents/Frameworks" -type f -not -path "*/Chromium Embedded Framework.framework/*")
fi
if uses ely-pdf-engine; then
  tessdata=${ELY_TESSDATA:-$root/engines/tessdata}
  need "$tessdata/eng.traineddata" scripts/fetch-tessdata.sh
  build -p ely-pdf-engine --bin ely-pdf-worker
  cp "$root/target/$profile/ely-pdf-worker" "$app/Contents/MacOS/"
  mkdir "$app/Contents/Resources/tessdata"
  cp "$tessdata"/{eng,chi_sim,chi_tra}.traineddata "$app/Contents/Resources/tessdata/"
  inner+=("$app/Contents/MacOS/ely-pdf-worker")
fi
if [ "$slug" = ely-tree ]; then
  need "$root/engines/git/bin/git" "scripts/build-git.sh and scripts/fetch-git-lfs.sh"
  ditto "$root/engines/git" "$app/Contents/Resources/git"
  while IFS= read -r binary; do
    file -b "$binary" | grep -q '^Mach-O' && inner+=("$binary")
  done < <(find "$app/Contents/Resources/git" -type f -perm -111)
fi
# Web content apps carry CEF and their bundled pages.
framework=
helpers=()
if grep -q '^ely-web-content' "$root/apps/$slug/Cargo.toml"; then
  cef=${ELY_CEF:-$root/engines/cef}
  [ -d "$cef/Chromium Embedded Framework.framework" ] || {
    echo "bundle.sh: no CEF at $cef; run scripts/fetch-cef.sh" >&2
    exit 1
  }
  mkdir -p "$app/Contents/Frameworks"
  framework="$app/Contents/Frameworks/Chromium Embedded Framework.framework"
  cp -R "$cef/Chromium Embedded Framework.framework" "$framework"
  case $profile in
    release) cargo build --manifest-path "$root/Cargo.toml" --release -p ely-web-content --bin ely-web-helper ;;
    debug) cargo build --manifest-path "$root/Cargo.toml" -p ely-web-content --bin ely-web-helper ;;
  esac
  # Chromium looks for these five, named after the app's binary.
  for kind in "" " (GPU)" " (Renderer)" " (Plugin)" " (Alerts)"; do
    helper="$slug Helper$kind"
    contents="$app/Contents/Frameworks/$helper.app/Contents"
    mkdir -p "$contents/MacOS"
    cp "$root/target/$profile/ely-web-helper" "$contents/MacOS/$helper"
    id=$(echo "${slug#ely-}.helper$kind" | tr -d '()' | tr ' ' '.' | tr 'A-Z' 'a-z')
    cat >"$contents/Info.plist" <<HELPER
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>$helper</string>
  <key>CFBundleIdentifier</key><string>$prefix.$id</string>
  <key>CFBundleName</key><string>$helper</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>LSUIElement</key><string>1</string>
</dict>
</plist>
HELPER
    helpers+=("$app/Contents/Frameworks/$helper.app")
  done
  content=$root/engines/content/$slug
  [ -f "$content/index.html" ] || {
    echo "bundle.sh: no page at $content; run its scripts/build-*-content.sh" >&2
    exit 1
  }
  cp -R "$content" "$app/Contents/Resources/content"
fi
"$root/packaging/licenses.sh" "$slug" "$app/Contents/Resources/Licenses"
cat >"$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>$slug</string>
  <key>CFBundleIconFile</key><string>$slug</string>
  <key>CFBundleIdentifier</key><string>$prefix.${slug#ely-}</string>
  <key>CFBundleName</key><string>$name</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>ZacharyZhang, zacharyzhang.com</string>
$("$root/packaging/macos/documents.sh" "$slug")
</dict>
</plist>
PLIST
# Nested code is signed before the app that holds it.
for target in ${inner[@]+"${inner[@]}"} ${framework:+"$framework"} ${helpers[@]+"${helpers[@]}"} "$app"; do
  if [ "$identity" = - ]; then
    codesign --force --sign - "$target"
  else
    # Hardened runtime: V8 in CEF's helpers needs its entitlements.
    entitled=()
    case $target in
      *" Helper (Renderer).app") entitled=(--entitlements "$root/packaging/macos/entitlements/renderer.plist") ;;
      *" Helper (GPU).app") entitled=(--entitlements "$root/packaging/macos/entitlements/gpu.plist") ;;
      *" Helper (Plugin).app") entitled=(--entitlements "$root/packaging/macos/entitlements/plugin.plist") ;;
    esac
    codesign --force --options runtime --timestamp ${entitled[@]+"${entitled[@]}"} --sign "$identity" "$target"
  fi
done
echo "$app"
