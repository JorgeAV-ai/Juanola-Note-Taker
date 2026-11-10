#!/bin/zsh
# Renderiza el fondo del .dmg desde apps/desktop/src-tauri/dmg/background.html
# con Chrome headless a 1x y 2x, y los junta en un TIFF multirresolución para
# que Finder lo pinte nítido en pantallas Retina. Solo hace falta al cambiar
# el diseño: el TIFF va en git.
set -euo pipefail
cd "$(dirname "$0")/.."
DIR=apps/desktop/src-tauri/dmg
CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

for scale in 1 2; do
  "$CHROME" --headless=new --disable-gpu --hide-scrollbars --virtual-time-budget=4000 \
    --window-size=660,400 --force-device-scale-factor=$scale \
    --screenshot="$TMP/bg@${scale}x.png" "file://$PWD/$DIR/background.html" 2>/dev/null
done

tiffutil -cathidpicheck "$TMP/bg@1x.png" "$TMP/bg@2x.png" -out "$DIR/background.tiff"
cp "$TMP/bg@2x.png" "$DIR/background-preview.png"
echo "listo: $DIR/background.tiff"
