#!/usr/bin/env bash
# Builds the rasterizer WASM exactly as uploaded on-chain and prints its size and SHA-256.
# Output: target/render-wasm/deadroll_render.wasm
set -euo pipefail
cd "$(dirname "$0")/../../.."
BINARYEN=132.0.0
OUT=target/render-wasm
cargo build -p deadroll-render-wasm --target wasm32-unknown-unknown --profile wasm
mkdir -p "$OUT"
# Rust 1.92 emits these features by default; the stripped module has no target_features section.
npx -y -p "binaryen@$BINARYEN" wasm-opt -Oz \
  --enable-bulk-memory --enable-bulk-memory-opt --enable-nontrapping-float-to-int \
  --enable-sign-ext --enable-mutable-globals --enable-reference-types --enable-multivalue \
  target/wasm32-unknown-unknown/wasm/deadroll_render_wasm.wasm -o "$OUT/deadroll_render.wasm"
SIZE=$(wc -c < "$OUT/deadroll_render.wasm")
SHA=$(node -e "console.log(require('crypto').createHash('sha256').update(require('fs').readFileSync('$OUT/deadroll_render.wasm')).digest('hex'))")
echo "deadroll_render.wasm: $SIZE bytes, sha256 $SHA"
if [ "$SIZE" -gt 61440 ]; then echo "over the 60 KB budget" >&2; exit 1; fi
