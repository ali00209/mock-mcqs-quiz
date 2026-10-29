#!/usr/bin/env bash
# Builds the database worker to WebAssembly and generates the JS glue that the
# app loads, into crates/web/public/worker/ (Trunk copies it into the app).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build -p mocktest-worker --target wasm32-unknown-unknown --release
mkdir -p crates/web/public/worker
wasm-bindgen --target web --out-dir crates/web/public/worker \
  target/wasm32-unknown-unknown/release/mocktest_worker.wasm
echo "Worker built. Next: scripts/serve.sh"
