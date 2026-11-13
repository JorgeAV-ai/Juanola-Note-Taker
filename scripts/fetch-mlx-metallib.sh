#!/usr/bin/env bash
# Descarga el mlx.metallib precompilado de la wheel oficial `mlx-metal` de PyPI,
# en la misma versión de MLX que trae third_party/mlxrs-sys, y verifica su SHA-256
# (fijado aquí: el de la API de PyPI vendría del mismo sitio que la URL).
# La wheel es la de macOS 26: de ahí el `minimumSystemVersion` de tauri.conf.json.
set -euo pipefail
VERSION="0.32.2"
WHEEL="mlx_metal-${VERSION}-py3-none-macosx_26_0_arm64.whl"
SHA="e6abeac9ac5265830c9c1541b6f96e9be37a85c2446763a46ad466c63a3837ab"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/third_party/mlx-metallib"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

META="$(curl -sf "https://pypi.org/pypi/mlx-metal/${VERSION}/json")"
URL="$(python3 -c "
import json,sys; d=json.loads(sys.argv[1]); print([u for u in d['urls'] if u['filename']==sys.argv[2]][0]['url'])" "$META" "$WHEEL")"
curl -sfL -o "$TMP/w.whl" "$URL"
echo "$SHA  $TMP/w.whl" | shasum -a 256 -c -
mkdir -p "$DEST"
unzip -q -o -j "$TMP/w.whl" mlx/lib/mlx.metallib -d "$DEST"
echo "OK: $DEST/mlx.metallib"
