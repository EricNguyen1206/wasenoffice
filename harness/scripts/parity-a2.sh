#!/bin/bash
# NanoOffice parity harness — A2/S3 (T-A23): crate nanooffice-ooxml ↔ x2t,
# measured on the MODEL (docs/design/converter-a2/DD.md §5) — never on bytes.
#
# For each corpus docx:
#   (a) x2t round-trip (DEV-ORACLE, 2 calls of the bundled binary — the same
#       two shapes the shell uses, sidecar/mod.rs): docx → Editor.bin
#       (positional; success = bin exists) → docx (params XML; success = exit 0
#       + non-empty output).
#   (b) crate round-trip: parse → write via the nanooffice-ooxml CLI.
#   (c) 3-way model compare on canonical JSON (CLI parse output; BTreeMaps are
#       key-sorted, so the JSON is a canonical model encoding):
#         m-orig  = parse(input)
#         m-crate = parse(crate round-trip output)
#         m-x2t   = parse(x2t round-trip output)
#       INVARIANT  m-orig == m-crate  — must be EXACT (crate round-trip, T-A22)
#       PARITY     m-crate vs m-x2t   — the parity measurement; every diff must
#                                       be classified (bug crate / limitation /
#                                       harmless x2t) in DD §5 + diagnostics.
#   (d) media byte-exactness (T-A26): every media part the model references
#       must survive the crate round-trip byte-identical — the CLI round-trip
#       face passes the source package back in, so a placeholder swap or any
#       re-encode is a regression. The x2t side is the oracle's own
#       serialization and is not gated here.
#
# Corpus: crate fixtures (tests/fixtures/*.docx) + harness/fixtures/corpus-a2
# (NOTES.md carries the per-file expected-model notes). Fuzz stays in the crate
# tests (T-A21/A22, 500-round mutation, 0-panic) — parity here is corpus-only.
#
# Exit 0 = every file parsed 3/3, crate round-trip invariant held everywhere,
# x2t steps succeeded. Parity diffs are REPORTED (they are the measurement —
# classified by hand in the DD, not hidden by an exit code).
#
# Usage: parity-a2.sh [file.docx ...]
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
CLI="$ROOT/src-tauri/target/debug/nanooffice-ooxml"

# x2t resolution mirrors sidecar::x2t_path branch 1+2 (env override, then the
# bundled converter; no app fallback here — parity measures OUR bundle).
X2T="${NANO_OFFICE_X2T:-$ROOT/engines/converter/$(rustc -vV | sed -n 's/^host: *//p')/x2t}"
[ -f "$X2T" ] || { echo "parity-a2: x2t not found at $X2T (bundle first: harness/scripts/bundle-x2t.sh)"; exit 2; }

cargo build -p nanooffice-ooxml --manifest-path "$ROOT/src-tauri/Cargo.toml" --quiet \
  || { echo "parity-a2: cargo build failed"; exit 2; }

CORPUS=("$@")
if [ ${#CORPUS[@]} -eq 0 ]; then
  CORPUS=("$ROOT"/src-tauri/crates/nanooffice-ooxml/tests/fixtures/*.docx
          "$ROOT"/harness/fixtures/corpus-a2/*.docx)
fi

WORK="$(mktemp -d /tmp/nanooffice-parity-a2-XXXXXX)"
FORMAT_TO_DOCX=65   # src-tauri/src/formats.rs — docx save code (65 = Wired)

row() { printf '%-34s %-9s %-10s %s\n' "$1" "$2" "$3" "$4"; }

# sha256 of one part of a docx (python3 stdlib — no unzip/jq dependency);
# prints MISSING when the part is absent.
part_sha() {
  python3 - "$1" "$2" <<'EOF'
import hashlib, sys, zipfile
try:
    print(hashlib.sha256(zipfile.ZipFile(sys.argv[1]).read(sys.argv[2])).hexdigest())
except KeyError:
    print("MISSING")
EOF
}
row "file" "parse" "invariant" "parity-diff (crate-rt vs x2t-rt)"
FAILS=0
PARITY_CLEAN=0

for f in "${CORPUS[@]}"; do
  name="$(basename "$f" .docx)"
  dir="$WORK/$name"
  mkdir -p "$dir"

  # (a) x2t round-trip — call 1: docx → Editor.bin (OpenBinary shape:
  # success = bin exists; x2t is routinely non-zero with valid output)
  "$X2T" "$f" "$dir/Editor.bin" >"$dir/x2t-open.log" 2>&1
  if [ ! -f "$dir/Editor.bin" ]; then
    row "$name" "x2t-open FAIL" "-" "see $dir/x2t-open.log"
    FAILS=$((FAILS + 1)); continue
  fi
  # call 2: Editor.bin → docx (SaveFromChanges shape: success = exit 0 +
  # non-empty output; no changes file → no m_bFromChanges, no font params —
  # pure C++ conversion, DoctRenderer not involved, proven in T-A23 probe)
  printf '<?xml version="1.0" encoding="utf-8"?><TaskQueueDataConvert><m_sFileFrom>%s</m_sFileFrom><m_sFileTo>%s</m_sFileTo><m_nFormatTo>%s</m_nFormatTo><m_bDontSaveAdditional>true</m_bDontSaveAdditional></TaskQueueDataConvert>' \
    "$dir/Editor.bin" "$dir/x2t-rt.docx" "$FORMAT_TO_DOCX" > "$dir/params.xml"
  "$X2T" "$dir/params.xml" >"$dir/x2t-save.log" 2>&1
  rc=$?
  if [ $rc -ne 0 ] || [ ! -s "$dir/x2t-rt.docx" ]; then
    row "$name" "x2t-rt FAIL" "-" "exit=$rc, see $dir/x2t-save.log"
    FAILS=$((FAILS + 1)); continue
  fi

  # (b) crate round-trip + (c) three parses to canonical JSON
  if ! "$CLI" "$f" "$dir/crate-rt.docx" 2>"$dir/crate-rt.err"; then
    row "$name" "crate-rt FAIL" "-" "$(cat "$dir/crate-rt.err")"
    FAILS=$((FAILS + 1)); continue
  fi
  ok=1
  for pair in "$f:m-orig" "$dir/crate-rt.docx:m-crate" "$dir/x2t-rt.docx:m-x2t"; do
    src="${pair%%:*}"; dst="${pair##*:}"
    if ! "$CLI" "$src" >"$dir/$dst.json" 2>"$dir/$dst.err"; then
      row "$name" "parse FAIL ($dst)" "-" "$(head -1 "$dir/$dst.err")"
      ok=0
    fi
  done
  [ $ok -eq 1 ] || { FAILS=$((FAILS + 1)); continue; }

  if diff -q "$dir/m-orig.json" "$dir/m-crate.json" >/dev/null; then
    INV="ok"
  else
    INV="BROKEN"; FAILS=$((FAILS + 1))
    diff "$dir/m-orig.json" "$dir/m-crate.json" > "$dir/INVARIANT.diff"
  fi

  # (d) media byte-exactness through the crate round-trip (T-A26): the parts
  # the model references are exactly the word/media/* names in the model JSON
  changed=""
  for p in $(grep -o 'word/media/[A-Za-z0-9._-]*' "$dir/m-orig.json" | sort -u); do
    [ "$(part_sha "$f" "$p")" = "$(part_sha "$dir/crate-rt.docx" "$p")" ] \
      || changed="$changed $p"
  done
  if [ -n "$changed" ]; then
    row "$name" "media FAIL" "$INV" "bytes changed:$changed"
    FAILS=$((FAILS + 1))
  fi

  # PARITY: crate round-trip model vs x2t round-trip model
  if diff -q "$dir/m-crate.json" "$dir/m-x2t.json" >/dev/null; then
    PARITY_CLEAN=$((PARITY_CLEAN + 1))
    row "$name" "ok" "$INV" "IDENTICAL"
  else
    diff "$dir/m-crate.json" "$dir/m-x2t.json" > "$dir/PARITY.diff"
    row "$name" "ok" "$INV" "$(grep -c '^[<>]' "$dir/PARITY.diff") diff line(s) → $dir/PARITY.diff"
  fi
done

echo
echo "parity-a2: files=${#CORPUS[@]} parity-clean=$PARITY_CLEAN hard-fails=$FAILS"
echo "parity-a2: artifacts kept at $WORK (m-orig/m-crate/m-x2t JSON, PARITY.diff per file)"
[ "$FAILS" -eq 0 ] || exit 1
exit 0
