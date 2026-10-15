#!/usr/bin/env bash
# Prints NSIS macros naming what a stage installs.
# Usage: files.sh <stage> <slug>
set -euo pipefail

stage=${1:?staged folder from packaging/stage.sh}
slug=${2:?app slug, such as ely-doc}
names=$({ ls -A "$stage"; echo "$slug.ico"; echo uninstall.exe; } | sort -u)
for name in $names; do
  [[ $name =~ ^[A-Za-z0-9._-]+$ ]] || {
    echo "files.sh: unsafe name $name" >&2
    exit 1
  }
done

# The list the next install and the uninstaller remove.
echo '!macro ElyWriteOwned'
echo '  FileOpen $9 "$INSTDIR\.ely-files" w'
for name in $names; do
  echo "  FileWrite \$9 \"$name\$\\r\$\\n\""
done
echo '  FileClose $9'
echo '!macroend'

# Names this install creates, for a rollback to remove.
echo '!macro ElyNoteNew'
for name in $names; do
  echo "  IfFileExists \"\$INSTDIR\\$name\" +2"
  echo "  FileWrite \$R2 \"$name\$\\r\$\\n\""
done
echo '!macroend'
