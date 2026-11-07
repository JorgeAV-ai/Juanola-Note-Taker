#!/usr/bin/env bash
# Descarga el código de llama.cpp que usa third_party/llama-cpp-sys-2 (tag fijado,
# con soporte LFM3) y verifica su SHA-256.
set -euo pipefail
TAG="b11960"
SHA="2b37ffa871595f90b94164f6c510ad9a34bd7f94da21557cc9878587e163af37"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/third_party/llama-cpp-sys-2/llama.cpp"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
curl -sfL -o "$TMP/l.tgz" "https://github.com/ggml-org/llama.cpp/archive/refs/tags/${TAG}.tar.gz"
echo "$SHA  $TMP/l.tgz" | shasum -a 256 -c -
rm -rf "$DEST" && mkdir -p "$DEST"
tar -xzf "$TMP/l.tgz" -C "$DEST" --strip-components=1
echo "OK: llama.cpp ${TAG} en $DEST"
