#!/bin/bash
# T-A10 (Track A rung A1, QĐ9): bundle the x2t converter into
# engines/converter/<target>/ — the first runtime reference pulled off the
# installed ONLYOFFICE.app. The SCRIPT is committed; the bundled CONTENT under
# engines/ NEVER is (.gitignore engines/* — mit-purity rule 4).
#
# What it copies (subset proven by measurement + a real run, 30.09):
#   - x2t binary + DoctRenderer.config
#   - ALL 17 @rpath frameworks — otool -L shows x2t strong-links every one
#     via LC_LOAD_DYLIB (0 weak entries) and the graph is closed among them,
#     so dyld refuses to start with any missing; the "dlopen on-demand"
#     hypothesis is disproven at the dylib level.
#   - a symlink engines/converter/editors → ../editors: DoctRenderer.config
#     points at ../editors/sdkjs/... (native.js defines InitNativeEditors,
#     needed for from-changes saves — exit 86 TypeError without it), which in
#     the installed app resolves to Resources/editors; under engines/ the
#     editors build sits at engines/editors (sibling of converter/), and the
#     symlink keeps the app's relative layout intact. Breaks loudly when the
#     engines/editors build is missing.
#   - NO templates/ (207 MB — per-locale NEW-document templates, unused for
#     open→edit→save of an existing file; x2t ran fine without them), NO
#     empty/ (2.9 MB — same; add back only if a task needs blank-file
#     creation), NO dictionaries/ (spell only — conversion never reads it).
# Trimming the remaining ~113 MB below the 100 MB A1 budget = T-A12.
#
# Target dir name: `rustc -vV` host triple (e.g. aarch64-apple-darwin) —
# MUST match src-tauri/src/sidecar/mod.rs target_dir_name() at runtime
# (composed there from std::env::consts::ARCH/OS — keep the tables in sync).
#
# Usage: bundle-x2t.sh [source-converter-dir]
#        (default: /Applications/ONLYOFFICE.app/Contents/Resources/converter)
#
# Other platforms (note only — do NOT wire up here): the macOS bundle cannot
# be reused; fetch the official ONLYOFFICE installer for Windows / the
# build_tools output for Linux, copy the equivalent converter dir, and pass it
# as the argument — the <target> dir name still comes from the host rustc.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
SRC="${1:-/Applications/ONLYOFFICE.app/Contents/Resources/converter}"

X2T="$SRC/x2t"
[ -f "$X2T" ] || { echo "bundle-x2t: no x2t at $X2T (pass the converter dir of an ONLYOFFICE install)"; exit 1; }

command -v rustc >/dev/null 2>&1 || { echo "bundle-x2t: rustc not found (needed for the host target triple)"; exit 1; }
TARGET="$(rustc -vV | sed -n 's/^host: //p')"
[ -n "$TARGET" ] || { echo "bundle-x2t: cannot determine rustc host triple"; exit 1; }

DEST="$ROOT/engines/converter/$TARGET"
echo "bundle-x2t: src=$SRC dest=$DEST"

# 17 frameworks — hard-linked from x2t (LC_LOAD_DYLIB, see header). The list
# is pinned so a new upstream converter version fails loudly here instead of
# silently shipping a broken bundle.
FRAMEWORKS="DjVuFile doctrenderer DocxRenderer EpubFile Fb2File HWPFile
HtmlFile2 IWorkFile OFDFile PdfFile StarMathConverter UnicodeConverter XpsFile
graphics kernel kernel_network ooxmlsignature"

FW_COUNT=$(echo $FRAMEWORKS | wc -w | tr -d ' ')
FOUND=0
for fw in $FRAMEWORKS; do
  [ -d "$SRC/$fw.framework" ] && FOUND=$((FOUND+1)) || echo "bundle-x2t: WARN missing framework: $fw.framework"
done
[ "$FOUND" -eq "$FW_COUNT" ] || { echo "bundle-x2t: only $FOUND/$FW_COUNT frameworks present — refusing to bundle a broken subset"; exit 1; }

rm -rf "$DEST"
mkdir -p "$DEST"
cp "$X2T" "$DEST/x2t"
cp "$SRC/DoctRenderer.config" "$DEST/DoctRenderer.config"
for fw in $FRAMEWORKS; do
  cp -R "$SRC/$fw.framework" "$DEST/"
done

# DoctRenderer relative base fix (see header) — engines/editors must exist:
# the from-changes DoctRenderer loads native.js/jquery_native.js/xregexp
# through ../editors/… resolved from the config dir.
EDITORS_BUILD="$ROOT/engines/editors"
[ -d "$EDITORS_BUILD/sdkjs" ] || { echo "bundle-x2t: engines/editors/sdkjs missing — build the editors payload first (see conventions.md)"; exit 1; }
ln -sfn ../editors "$ROOT/engines/converter/editors"

# Sibling probe: /Applications/ONLYOFFICE.app keeps templates/ + empty/ next to
# the frameworks — intentionally NOT copied (see header).
SIZE="$(du -sh "$DEST" | cut -f1)"
echo "bundle-x2t: done — x2t + DoctRenderer.config + $FOUND frameworks = $SIZE (+ engines/converter/editors → ../editors symlink)"
echo "bundle-x2t: verify: NANO_OFFICE_X2T unset + ./harness/scripts/smoke-save.sh (gate 10 must log the engines/ path)"
