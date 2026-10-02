#!/bin/bash
# T-A12 (Track A rung A1, QĐ9): bundle the app-root font set into
# engines/fonts/ — the LAST runtime reference to the installed ONLYOFFICE.app
# (protocol/mod.rs font root "app" serves + enumerates from engines/fonts
# since this task; URL namespace /fonts/app/ unchanged). The SCRIPT is
# committed; the bundled CONTENT under engines/ NEVER is (.gitignore engines/*).
#
# What it copies — the app's own font root
# (/Applications/ONLYOFFICE.app/Contents/Resources/login/fonts), layout
# preserved, license texts copied alongside (licensing.md § S6 — per-file font
# licenses, NOT the engine's AGPL):
#   - OpenSans-*.ttf (5) + LICENSE.txt (Apache 2.0 — font name table:
#     manufacturer Ascender Corporation, licenseURL apache.org/LICENSE-2.0)
#   - crosextra/Carlito-*.ttf (4) + LICENSE.txt (SIL OFL 1.1 — tyPoland
#     Lukasz Dziedzic, licenseURL scripts.sil.org/OFL)
#   - caladea/caladea-*.ttf (4) + LICENSE.txt (SIL OFL 1.1 — Huerta
#     Tipográfica; name table also carries an Apache-2.0 licenseURL)
#   - asana/ASANA.ttc + LICENSE.txt (SIL OFL 1.1 — Asana Math)
#   - openoffice/opens___.ttf + LICENSE.txt (SIL OFL 1.0 — OpenSymbol)
#   - EXCLUDED: ASC.ttf — license UNKNOWN (no license fields in the name
#     table, no LICENSE file shipped for it) and never fetched by the engine
#     in any gate run (oracle-obs 02.10: the consumer replaces the "ASCW3"
#     family with an inline base64 symbol font — reference.map § AllFonts
#     generator). mit-purity S6: unclear license → do not bundle.
# All bundled licenses permit redistribution WITH the license text, which is
# copied verbatim next to the fonts. The whole set is ~2 MB — bundled whole
# (not just the Carlito the smoke fixtures fetch) so real documents keep the
# app's substitution families (Cambria→Caladea, bullets→OpenSymbol,
# equations→Asana Math) on a machine without the app (T-A13).
#
# Usage: bundle-fonts.sh [source-font-dir]
#        (default: /Applications/ONLYOFFICE.app/Contents/Resources/login/fonts)
# Verify: ./harness/scripts/verify.sh  (gates 9+10 must fetch /fonts/app/…
#         from engines/fonts — the added "font serve" log line shows the path)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="${1:-/Applications/ONLYOFFICE.app/Contents/Resources/login/fonts}"
DEST="$ROOT/engines/fonts"

[ -d "$SRC" ] || { echo "bundle-fonts: no font dir at $SRC (pass the login/fonts dir of an ONLYOFFICE install)"; exit 1; }

# Pinned manifest — a new upstream font set fails loudly here instead of
# silently shipping a different (possibly unlicensed) bundle.
FILES="LICENSE.txt
OpenSans-Bold.ttf
OpenSans-ExtraBold.ttf
OpenSans-Light.ttf
OpenSans-Regular.ttf
OpenSans-Semibold.ttf
asana/ASANA.ttc
asana/LICENSE.txt
caladea/caladea-bold.ttf
caladea/caladea-bolditalic.ttf
caladea/caladea-italic.ttf
caladea/caladea-regular.ttf
caladea/LICENSE.txt
crosextra/Carlito-Bold.ttf
crosextra/Carlito-BoldItalic.ttf
crosextra/Carlito-Italic.ttf
crosextra/Carlito-Regular.ttf
crosextra/LICENSE.txt
openoffice/opens___.ttf
openoffice/LICENSE.txt"

MISSING=0
while IFS= read -r f; do
  [ -n "$f" ] || continue
  [ -f "$SRC/$f" ] || { echo "bundle-fonts: WARN missing: $f"; MISSING=$((MISSING+1)); }
done <<< "$FILES"
[ "$MISSING" -eq 0 ] || { echo "bundle-fonts: $MISSING manifest files missing — refusing to bundle an incomplete set"; exit 1; }

rm -rf "$DEST"
mkdir -p "$DEST"
while IFS= read -r f; do
  [ -n "$f" ] || continue
  mkdir -p "$DEST/$(dirname "$f")"
  cp "$SRC/$f" "$DEST/$f"
done <<< "$FILES"

SIZE="$(du -sh "$DEST" | cut -f1)"
COUNT="$(find "$DEST" -type f \( -name '*.ttf' -o -name '*.ttc' -o -name '*.otf' \) | wc -l | tr -d ' ')"
echo "bundle-fonts: done — $COUNT font files + 5 license texts = $SIZE (ASC.ttf excluded — license unknown, never fetched; see header)"
echo "bundle-fonts: verify: ./harness/scripts/verify.sh (gate 9 xlsx/pptx must still fetch /fonts/app/crosextra/Carlito-…; the font-serve log line now points at engines/fonts)"
