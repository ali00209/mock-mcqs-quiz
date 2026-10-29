#!/usr/bin/env bash
# Builds the app with Trunk and serves it on http://localhost:8080 (rebuilds on change).
set -euo pipefail
cd "$(dirname "$0")/../crates/web"
exec trunk serve --port 8080 --open
