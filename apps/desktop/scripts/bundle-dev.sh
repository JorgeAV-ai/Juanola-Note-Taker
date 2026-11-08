#!/usr/bin/env bash
# Genera Juanola.app (perfil debug) con el worker de STT como sidecar y la firma
# ad hoc, para probar con permisos reales de micrófono y audio del sistema
# (`pnpm tauri dev` corre el binario suelto y el process tap devuelve silencio).
# Uso: apps/desktop/scripts/bundle-dev.sh [--open]
set -euo pipefail
APP_DIR="$(cd "$(dirname "$0")/.." && pwd)"
ROOT="$(cd "$APP_DIR/../.." && pwd)"

echo "1/3 Worker de STT (sidecar, release)…"
"$APP_DIR/scripts/build-sidecar.sh"

echo "2/3 App (debug)…"
cd "$APP_DIR"
pnpm -s tauri build --debug --bundles app 2>&1 | grep -E "error|Error|Finished|Bundling" || true
APP="$ROOT/target/debug/bundle/macos/Juanola.app"
[ -x "$APP/Contents/MacOS/juanola-stt-worker" ] || { echo "falló el bundle (falta el sidecar)" >&2; exit 1; }

echo "3/3 Firma ad hoc…"
codesign --force --deep --sign - "$APP" >/dev/null 2>&1
echo "Listo: $APP"
if [ "${1:-}" = "--open" ]; then
  open "$APP"
fi
