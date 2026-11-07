#!/usr/bin/env bash
# Empaqueta el ejemplo `live` (captura + STT en vivo) como .app con permisos de
# micrófono y audio del sistema, igual que record-dual-app.sh, y lo lanza.
# Uso: crates/stt/scripts/live-app.sh [segundos] [idioma|auto]
set -euo pipefail
SECS="${1:-60}"
LANG_ARG="${2:-Spanish}"
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
APP="$ROOT/target/release/JuanolaLive.app"
LOG="$ROOT/recordings/live.log"

echo "Compilando (la primera vez tarda unos minutos)…"
cargo build -q --release --manifest-path "$ROOT/Cargo.toml" -p juanola-stt --features mlx --bins --examples

if pgrep -f "JuanolaLive.app/Contents/MacOS/live" >/dev/null; then
  echo "Ya hay una sesión en marcha (JuanolaLive.app)." >&2
  exit 1
fi

mkdir -p "$APP/Contents/MacOS" "$ROOT/recordings"
cp "$ROOT/target/release/examples/live" "$APP/Contents/MacOS/live"
cp "$ROOT/target/release/juanola-stt-worker" "$APP/Contents/MacOS/juanola-stt-worker"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key><string>dev.juanola.live</string>
  <key>CFBundleName</key><string>JuanolaLive</string>
  <key>CFBundleExecutable</key><string>live</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSUIElement</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>Transcripción del micrófono para Juanola.</string>
  <key>NSAudioCaptureUsageDescription</key><string>Transcripción del audio del sistema para Juanola.</string>
</dict>
</plist>
PLIST
codesign --force --sign - "$APP" >/dev/null 2>&1

: > "$LOG"
echo "Transcribiendo ${SECS}s (log en $LOG)…"
# Ctrl+C solo mataría a `open`: reenviarlo a la app para que cierre limpia.
trap 'pkill -INT -f "JuanolaLive.app/Contents/MacOS/live" || true' INT TERM
open -W -n --env JUANOLA_CLOCK_DEBUG=${JUANOLA_CLOCK_DEBUG:-} --env RUST_LOG=${RUST_LOG:-warn} --stdout "$LOG" --stderr "$LOG" "$APP" --args "$SECS" "$LANG_ARG" &
OPEN_PID=$!
tail -n +1 -f "$LOG" &
TAIL_PID=$!
wait "$OPEN_PID" 2>/dev/null || wait "$OPEN_PID" 2>/dev/null || true
sleep 0.3
kill "$TAIL_PID" 2>/dev/null || true
