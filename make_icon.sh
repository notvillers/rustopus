#!/usr/bin/env bash
# Regenerate the macOS-style squircle app tile from a plain (transparent) logo.
#
# Usage:
#   ./make_icon.sh                       # octo.png -> octo_icon.png (defaults)
#   ./make_icon.sh path/to/logo.png      # custom source
#   ./make_icon.sh logo.png out.png      # custom source + output
#
# Env overrides:
#   BG_TOP / BG_BOTTOM  gradient colors (default white -> light gray)
#   LOGO_SIZE           logo box inside the tile, px  (default 620)
#   Y_NUDGE             vertical optical nudge, px    (default -15, up)
#
# Requires ImageMagick 7 (`magick`). After running, rebuild the client so the
# new PNG is re-embedded (main.rs / menubar.rs include_bytes!, and the mac
# .icns via build_client_mac.sh).
set -euo pipefail

DIR="$(cd "$(dirname "$0")" && pwd)/client/src/assets/images"
SRC="${1:-$DIR/octo.png}"
OUT="${2:-$DIR/octo_icon.png}"

BG_TOP="${BG_TOP:-#ffffff}"
BG_BOTTOM="${BG_BOTTOM:-#e6ebf2}"
LOGO_SIZE="${LOGO_SIZE:-620}"
Y_NUDGE="${Y_NUDGE:--15}"

# macOS "Big Sur" geometry: 1024 canvas, 824 rounded tile (~100px margin),
# corner radius ~22.5% of the tile.
CANVAS=1024
TILE=824
RADIUS=185

command -v magick >/dev/null || { echo "error: ImageMagick 'magick' not found" >&2; exit 1; }
[ -f "$SRC" ] || { echo "error: source not found: $SRC" >&2; exit 1; }

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# 1. gradient, clipped to a rounded-rect (squircle-ish) tile.
magick -size ${TILE}x${TILE} gradient:"$BG_TOP"-"$BG_BOTTOM" "$TMP/grad.png"
magick -size ${TILE}x${TILE} xc:none -fill white \
  -draw "roundrectangle 0,0,$((TILE-1)),$((TILE-1)),$RADIUS,$RADIUS" "$TMP/mask.png"
magick "$TMP/grad.png" "$TMP/mask.png" -alpha off \
  -compose CopyOpacity -composite "$TMP/tile.png"

# 2. scale the logo to fit the tile, keeping aspect ratio.
magick "$SRC" -resize ${LOGO_SIZE}x${LOGO_SIZE} "$TMP/logo.png"

# 3. tile centered on the canvas, logo centered on the tile (optical nudge).
magick -size ${CANVAS}x${CANVAS} xc:none \
  "$TMP/tile.png" -gravity center -geometry +0+0 -composite \
  "$TMP/logo.png" -gravity center -geometry +0${Y_NUDGE} -composite \
  "$OUT"

echo "wrote $OUT ($(magick identify -format '%wx%h' "$OUT"))"
