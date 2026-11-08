#!/usr/bin/env bash
# Compila juanola-stt-worker (release, MLX) y lo deja donde Tauri espera los
# sidecar de `bundle.externalBin`: src-tauri/binaries/<nombre>-<triple>.
# Tauri lo copia a Contents/MacOS/juanola-stt-worker al empaquetar.
set -euo pipefail
APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$APP_DIR/../.." && pwd)"
TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
cargo build -q --release --manifest-path "$ROOT/Cargo.toml" -p juanola-stt --features mlx --bin juanola-stt-worker
mkdir -p "$APP_DIR/src-tauri/binaries"
cp "$ROOT/target/release/juanola-stt-worker" "$APP_DIR/src-tauri/binaries/juanola-stt-worker-$TRIPLE"
