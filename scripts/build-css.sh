#!/usr/bin/env sh
# Regenerates crates/web/tailwind.css from crates/web/tailwind/input.css.
# Needs Node.js and npm. The versions are pinned in tailwind/package.json.
set -eu
cd "$(dirname "$0")/../crates/web/tailwind"
npm install --silent --no-audit --no-fund
npx tailwindcss -i input.css -o ../tailwind.css --minify
