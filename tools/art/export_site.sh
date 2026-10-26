#!/usr/bin/env bash
# Renders the site-only art (assets/site_sprites) to PNG in app/assets, one pixel per pixel (the site
# scales by integers). Run from the repo root after changing a site sprite: bash tools/art/export_site.sh
set -euo pipefail
cd "$(dirname "$0")/../.."
art() { cargo run -q -p art -- "$@" >/dev/null; }
out=app/assets
mkdir -p "$out/city" "$out/interiors" "$out/ui" "$out/craft" "$out/screens"
# Pictures made of pieces (buildings, the parchment frame, loading screens).
for f in assets/site_sprites/buildings/*.layout; do art compose "$f" --out "$out/city/$(basename "${f%.layout}").png"; done
for f in assets/site_sprites/screens/*.layout; do art compose "$f" --out "$out/screens/$(basename "${f%.layout}").png"; done
# One PNG per sprite.
art assets/site_sprites/buildings/city_tiles.dsl --scale 1 --out "$out/city"
art assets/site_sprites/interiors/interiors.dsl --scale 1 --out "$out/interiors"
art assets/site_sprites/ui/ui.dsl --scale 1 --out "$out/ui"
art assets/site_sprites/benches/benches.dsl --scale 1 --out "$out/craft"
find "$out" -name '*.png' | wc -l
