#!/usr/bin/env bash
# Builds the web version into ./site (needs the wasm32-unknown-unknown target and wasm-bindgen-cli).
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --profile web --lib --target wasm32-unknown-unknown
rm -rf site && mkdir -p site/pkg
wasm-bindgen --target web --no-typescript --out-dir site/pkg target/wasm32-unknown-unknown/web/neobrush.wasm
if command -v wasm-opt >/dev/null; then
  wasm-opt -Oz --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext -o site/pkg/neobrush_bg.wasm site/pkg/neobrush_bg.wasm
fi
cp web/index.html web/icon.svg site/
cp docs/media/screenshot-dark.png site/og.png
echo "neobrush.wbg.gg" > site/CNAME
touch site/.nojekyll
ls -la site site/pkg
