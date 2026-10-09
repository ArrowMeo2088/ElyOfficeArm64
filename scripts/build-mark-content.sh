#!/usr/bin/env bash
# Builds ElyMark's page, pinned by the lockfile, into engines/content.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
web=$root/apps/ely-mark/web
cd "$web"
# Install scripts never run; esbuild's binary is its own package.
npm ci --ignore-scripts --no-audit --no-fund
node build.mjs "$root/engines/content/ely-mark"
