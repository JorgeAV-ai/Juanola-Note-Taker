#!/usr/bin/env bash
# Empaqueta el ejemplo `record_dual` como .app mínima con las claves de permiso
# (micrófono y captura de audio del sistema) y la lanza con `open`, para que macOS
# le pida los permisos a ella y no a la terminal. Las terminales no declaran
# NSAudioCaptureUsageDescription y el process tap puede devolver silencio.
#
# Uso: crates/audio-capture/scripts/record-dual-app.sh [segundos] [carpeta]
set -euo pipefail

SECS="${1:-30}"
mkdir -p "${2:-recordings}"
OUT="$(cd "$(dirname "${2:-recordings}")" && pwd)/$(basename "${2:-recordings}")"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
APP="$ROOT/target/debug/RecordDual.app"

cargo build -q --manifest-path "$ROOT/Cargo.toml" -p juanola-audio-capture --example record_dual

mkdir -p "$APP/Contents/MacOS" "$OUT"
cp "$ROOT/target/debug/examples/record_dual" "$APP/Contents/MacOS/record_dual"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>dev.juanola.record-dual</string>
  <key>CFBundleName</key><string>RecordDual</string>
  <key>CFBundleExecutable</key><string>record_dual</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSUIElement</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>Prueba de captura del micrófono para Juanola.</string>
  <key>NSAudioCaptureUsageDescription</key><string>Prueba de captura del audio del sistema para Juanola.</string>
</dict>
</plist>
PLIST
codesign --force --sign - "$APP" >/dev/null 2>&1

if pgrep -f "RecordDual.app/Contents/MacOS/record_dual" >/dev/null; then
  echo "Ya hay una grabación en marcha (RecordDual.app). Espera a que acabe o ciérrala." >&2
  exit 1
fi

LOG="$OUT/record_dual.log"
: > "$LOG"
echo "Grabando ${SECS}s en $OUT (log en $LOG)…"
# Ctrl+C solo mataría a `open`; reenviarlo a la app para que cierre los WAV.
trap 'pkill -INT -f "RecordDual.app/Contents/MacOS/record_dual" || true' INT TERM
open -W -n --stdout "$LOG" --stderr "$LOG" "$APP" --args "$SECS" "$OUT" &
wait $! || wait $! || true
grep -E "Listo|Aviso" "$LOG" || tail -5 "$LOG"
