#!/usr/bin/env bash
# Builds the mapcheck WASM the website map builder ships (app/src/chain/wasm/mapcheck.wasm).
#
# The build runs in the official Rust image, so the bytes do not depend on the machine: the
# repository, CARGO_HOME and the toolchain sit at the same paths everywhere, so the paths the
# module embeds (panic locations) are the same too. CI runs it and fails when the committed file
# differs.
#
#   bash crates/mapcheck/wasm/build.sh          # rebuild app/src/chain/wasm/mapcheck.wasm
#   bash crates/mapcheck/wasm/build.sh --check  # rebuild, then fail if the committed copy differs
set -euo pipefail
cd "$(dirname "$0")/../../.."

# Same version as rust-toolchain.toml.
IMAGE=rust:1.92.0-slim-bookworm
OUT=app/src/chain/wasm/mapcheck.wasm
TARGET=target/mapcheck-wasm-docker

# Docker Desktop on Windows needs a Windows path for the bind mount.
SRC=$(pwd -W 2>/dev/null || pwd)

MSYS_NO_PATHCONV=1 docker run --rm \
  -v "$SRC:/deadroll" \
  -w /deadroll \
  -e CARGO_TARGET_DIR="/deadroll/$TARGET" \
  "$IMAGE" \
  bash -c "rustup target add wasm32-unknown-unknown >/dev/null && cargo build --locked -p deadroll-mapcheck-wasm --target wasm32-unknown-unknown --profile wasm"

mkdir -p "$(dirname "$OUT")"
BUILT="$TARGET/wasm32-unknown-unknown/wasm/deadroll_mapcheck_wasm.wasm"
if [ "${1:-}" = "--check" ]; then
  if ! cmp -s "$BUILT" "$OUT"; then
    echo "$OUT is stale: run bash crates/mapcheck/wasm/build.sh and commit it" >&2
    exit 1
  fi
  echo "$OUT matches the build"
else
  cp "$BUILT" "$OUT"
  SIZE=$(wc -c < "$OUT")
  SHA=$(sha256sum "$OUT" | cut -d' ' -f1)
  echo "mapcheck.wasm: $SIZE bytes, sha256 $SHA"
fi
