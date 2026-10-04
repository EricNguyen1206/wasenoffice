#!/bin/bash
# NanoOffice smoke gate 15 — Nano Light editor vòng 1 (T-A27, Track A rung A4):
# boots the app with NANO_OFFICE_NANO_LIGHT (the dedicated editor entry — NO
# engine, no x2t, no Editor.bin on this path) plus the edit driver
# NANO_OFFICE_NL_PROBE=<marker> (pattern NANO_OFFICE_SAVE_PROBE: the editor
# page sets the caret via a REAL hitTest, then drives the SAME edit()/save()
# the keydown handler uses — no synthetic keystrokes, rule 6b) and asserts
# the full round-trip:
#   1. `nano light first-draw N ms`  with N < NANO_LIGHT_MAX_MS (DD L01 §5)
#   2. the edit chain on the log: insert ×≥2 · backspace ×≥3 · enter · undo ·
#      redo (from `nano light edit:` lines, emitting end src-tauri/src/editor.rs)
#   3. `nano light saved:` — write_docx + atomic replace over the original
#   4. the SAVED FILE re-parsed by the crate CLI carries the marker text
#      AND the selection type-replace ("paragraphT28" — the probe's
#      shift+Arrow selection replaced by typing, T-A28)
#   5. media parts are byte-exact vs the pre-save snapshot (sha256 per part)
#   6. REOPEN: a second window (opened AFTER the save, via nano_light_reopen)
#      renders evidence containing the marker — "mở lại render đúng nội dung
#      đã edit" (DD L01 §5)
#   7. SELECTION evidence (T-A28): `nano light selection:` lines exist and
#      the max span ≥ NANO_LIGHT_MIN_SPAN — the shift+arrow selection really
#      happened (DD L01 §5.3e); min span == -1 pins the Cmd-A cross-para
#      select-all (T-A29, DD L01 §5.4)
#   8. COMPOSITION evidence (T-A29, DD L01 §5.4a): the telex-style chain
#      commits `Xin chào` (`nano light composition:` — last end line)
#   9. CLIPBOARD round (T-A29, DD L01 §5.4b): op=copy carries the committed
#      string, op=paste re-inserts it mid-word ("Xin Xin chàochào"), op=copy-all
#      after Cmd-A walks the WHOLE document (marker + T28-replace + paste
#      result; chars == unescaped length)
#   and no `nano light failed:` anywhere.
# LOG CONTRACT: the `nano light` lines are frozen constants SMOKE_RE_NL_* in
# harness/scripts/smoke-lib.sh — change in the SAME commit as the emitting
# ends (src-tauri/src/editor.rs + src/editor.ts).
# Rule 6b: boot via smoke_boot (background process group, window focused(false)).
#
# Usage: smoke-nano-light.sh
# Env:   SMOKE_NANO_LIGHT_TIMEOUT=360  total seconds before giving up
#        NANO_LIGHT_MAX_MS=500         first-draw budget (ms)
#        NANO_LIGHT_MARKER=…           the inserted marker (default fixed)
#        NANO_LIGHT_MIN_SPAN=3         selection span floor (gate 15, T-A28)
#        NANO_LIGHT_COMPOSE=…          the IME-committed string (default 'Xin chào')
# Negative proofs (asserts must bite; documented in docs/diagnostics.md T-A27
# and T-A28):
#   NANO_LIGHT_NEGATIVE=garbage harness/scripts/smoke-nano-light.sh
#     → a garbage .docx: the chain runs to parse → `nano light failed:` bites
#       → exit 1 fast (pattern gate 13).
#   NANO_LIGHT_NEGATIVE=doctor harness/scripts/smoke-nano-light.sh
#     → full positive run, but the marker check parses the PRE-SAVE snapshot
#       (no marker in it) → the read-the-saved-file assert bites → exit 1
#       (proves the load-bearing parse-back check, not just absence).
#   NANO_LIGHT_NEGATIVE=seldoctor harness/scripts/smoke-nano-light.sh
#     → full positive run, but the selection assert requires span == 0 —
#       contradicts the real selection evidence → exit 1 (proves the gate
#       READS the selection numbers, not just their presence — T-A28).
#   NANO_LIGHT_NEGATIVE=clipdoctor harness/scripts/smoke-nano-light.sh
#     → full positive run, but the clipboard assert requires text 'WRONG-CLIP'
#       with chars 999999 — contradicts the real clipboard evidence → exit 1
#       (proves the gate READS the clipboard numbers — T-A29).
set -u

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
. "$ROOT/harness/scripts/smoke-lib.sh"

TIMEOUT="${SMOKE_NANO_LIGHT_TIMEOUT:-360}"
MAX_MS="${NANO_LIGHT_MAX_MS:-500}"
MARKER="${NANO_LIGHT_MARKER:-NL01-EDITED-BY-NANOLIGHT}"
MIN_SPAN="${NANO_LIGHT_MIN_SPAN:-3}"
COMPOSE="${NANO_LIGHT_COMPOSE:-Xin chào}"
REPL='T28'
NEGATIVE="${NANO_LIGHT_NEGATIVE:-}"
FIXTURE=/tmp/nanooffice-nanolight.docx
SNAPSHOT=/tmp/nanooffice-nanolight-pre.docx
LOG="$(mktemp /tmp/nanooffice-smoke-nanolight-XXXXXX)"

if [ "$NEGATIVE" = "garbage" ]; then
  printf 'not a docx package' > "$FIXTURE"
  echo "smoke: NEGATIVE proof (garbage) — expecting 'nano light failed:' to bite"
else
  python3 "$ROOT/harness/fixtures/make_docx_layout.py" "$FIXTURE" >/dev/null || {
    echo "smoke: cannot generate fixture"; exit 1;
  }
fi
# pre-save snapshot: media byte-exact compares against it; the doctor
# negative parses it instead of the saved file (no marker inside by design)
cp "$FIXTURE" "$SNAPSHOT"

smoke_init "$FIXTURE"
echo "smoke: fixture=$FIXTURE log=$LOG timeout=${TIMEOUT}s budget=${MAX_MS}ms marker=$MARKER compose=$COMPOSE (NL probe driver${NEGATIVE:+, negative=$NEGATIVE})"

smoke_boot "NANO_OFFICE_NANO_LIGHT=$FIXTURE" "NANO_OFFICE_NL_PROBE=$MARKER" "NANO_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$LOG"

# The CLI is the crate's parse face (parity-a2.sh uses the same binary).
CLI="$ROOT/src-tauri/target/debug/nanooffice-ooxml"
cargo build -p nanooffice-ooxml --manifest-path "$ROOT/src-tauri/Cargo.toml" --quiet || {
  echo "smoke: cannot build the nanooffice-ooxml CLI"; exit 1;
}

nano_light_signals() {
  if grep -q "$SMOKE_RE_NL_FAILED" "$LOG" 2>/dev/null; then
    SMOKE_ABORT=1
    SMOKE_SUMMARY="nano light error ($(grep -m1 "$SMOKE_RE_NL_FAILED" "$LOG" | cut -c1-120))"
    return 1
  fi
  grep -q "$SMOKE_RE_NL_PROBE_DONE" "$LOG" 2>/dev/null || { SMOKE_SUMMARY="probe chain not done yet"; return 1; }
  # full asserts as the poll body (pattern gate 12): the reopen evidence may
  # trail the probe-done line, so keep polling until the python check passes
  SMOKE_NL_REPORT=$(python3 - "$LOG" "$MAX_MS" "$MARKER" "$SNAPSHOT" "$FIXTURE" "$CLI" "$NEGATIVE" "$MIN_SPAN" "$REPL" "$COMPOSE" <<'PY' 2>&1
import hashlib
import json
import re
import subprocess
import sys
import zipfile

log_path, max_ms, marker, snapshot, fixture, cli, negative, min_span, repl, compose = sys.argv[1:11]
max_ms = float(max_ms)
min_span = int(min_span)

def need(cond, msg):
    if not cond:
        print(f"NL FAIL {msg}")
        sys.exit(1)

lines = open(log_path, errors="replace").read().splitlines()

m = re.search(r"nano light first-draw ([\d.]+) ms", "\n".join(lines))
need(m, "no first-draw line")
ms = float(m.group(1))
need(ms < max_ms, f"first-draw {ms} ms exceeds budget {max_ms} ms")

edits = re.findall(r"nano light edit: cmd=(\w+)", "\n".join(lines))
for cmd, n in [("insert", 2), ("backspace", 3), ("enter", 1), ("undo", 1), ("redo", 1)]:
    need(edits.count(cmd) >= n, f"edit chain incomplete: {cmd} x{edits.count(cmd)} < {n} ({edits})")

# (7) SELECTION evidence (T-A28): the shift+arrow selection really happened.
# The seldoctor negative requires span == 0 — contradicting the real chain —
# so the assert bites (the gate READS the selection numbers, not just their
# presence). T-A29: min span == -1 pins the Cmd-A cross-para select-all.
sels = re.findall(
    r"nano light selection: anchor=(\d+)/(\d+)/(\d+) focus=(\d+)/(\d+)/(\d+) span=(-?\d+)",
    "\n".join(lines),
)
need(bool(sels), "no `nano light selection:` evidence line")
spans = [int(s[6]) for s in sels]
if negative == "seldoctor":
    need(max(spans) == 0, f"seldoctor: selection span {max(spans)} != 0 — the assert must bite here")
else:
    need(max(spans) >= min_span, f"selection span {max(spans)} < {min_span} ({sels[-1]})")
    need(min(spans) == -1, f"no cross-para selection (Cmd-A select-all) — spans {spans}")

# (8) COMPOSITION (T-A29, DD L01 §5.4a): the telex-style preedit committed.
compose_ends = re.findall(r"nano light composition: state=end data=(.*)", "\n".join(lines))
need(bool(compose_ends), "no `nano light composition:` end evidence")
need(compose_ends[-1] == compose, f"composition committed {compose_ends[-1]!r} != {compose!r}")

# (9) CLIPBOARD round (T-A29, DD L01 §5.4b): copy carries the committed string,
# paste re-inserts it mid-word, copy-all after Cmd-A walks the whole document.
clips = re.findall(r"nano light clipboard: op=(\S+) chars=(\d+) sys=(\S+) text=(.*)", "\n".join(lines))
need(clips, "no `nano light clipboard:` evidence")
copies = [c for c in clips if c[0] == "copy"]
need(bool(copies), "no op=copy evidence")
copy_text, copy_chars = copies[-1][3], int(copies[-1][1])
if negative == "clipdoctor":
    need(copy_text == "WRONG-CLIP" and copy_chars == 999999,
         f"clipdoctor: clipboard copy {copy_text!r}/{copy_chars} != 'WRONG-CLIP'/999999 — the assert must bite here")
else:
    need(copy_text == compose and copy_chars == len(compose),
         f"clipboard copy {copy_text!r} ({copy_chars}) != {compose!r} ({len(compose)})")
pastes = [c for c in clips if c[0] == "paste"]
need(bool(pastes), "no op=paste evidence")
need(int(pastes[-1][1]) == len(compose), f"paste chars {pastes[-1][1]} != {len(compose)}")
copied_all = [c for c in clips if c[0] == "copy-all"]
need(bool(copied_all), "no op=copy-all evidence (Cmd-A select-all)")
all_text = copied_all[-1][3].replace("\\n", "\n")
need(int(copied_all[-1][1]) == len(all_text),
     f"copy-all chars {copied_all[-1][1]} != unescaped length {len(all_text)}")
need(marker in all_text, "copy-all text misses the marker")
need(f"paragraph{repl}" in all_text, "copy-all text misses the T28 replace")
need(f"Xin {compose}chào" in all_text, f"copy-all text misses the paste result 'Xin {compose}chào'")

need(any("nano light saved:" in l for l in lines), "no saved line")

# (4) the saved file re-parsed by the crate carries the marker AND the
# selection type-replace. The DOCTOR negative parses the PRE-SAVE snapshot
# instead — no marker by design, so the assert itself is proven to bite (it
# reads the file, not the log).
parse_target = snapshot if negative == "doctor" else fixture
model = subprocess.run([cli, parse_target], capture_output=True, text=True)
need(model.returncode == 0, f"crate CLI failed on {parse_target}: {model.stderr[:200]}")
need(marker in model.stdout, f"marker {marker!r} not found in the crate-parsed model of {parse_target}")
if negative != "doctor":
    need(f"paragraph{repl}" in model.stdout, f"selection replace not joined to 'paragraph' in the parse-back of {parse_target} — expected 'paragraph{repl}' (the probe replaces the selected ' two')")
    need(compose in model.stdout, f"composed string {compose!r} not in the parse-back of {parse_target}")
    need(f"Xin {compose}chào" in model.stdout, f"paste result 'Xin {compose}chào' not in the parse-back of {parse_target}")

# (5) media byte-exact vs the pre-save snapshot (per part sha256)
def media_shas(path):
    with zipfile.ZipFile(path) as z:
        return {
            n: hashlib.sha256(z.read(n)).hexdigest()
            for n in z.namelist()
            if n.startswith("word/media/")
        }

before, after = media_shas(snapshot), media_shas(fixture)
need(before, "snapshot carries no media parts (fixture regression?)")
need(before == after, f"media parts changed on save: {sorted(set(before.items()) ^ set(after.items()))[:4]}")

# (6) REOPEN: the LAST `nano light: opened` after the `nano light saved:`
# line is the second window; an evidence JSON AFTER it must carry the marker
# in some rendered line — the saved file re-renders with the edited content.
saved_idx = max(i for i, l in enumerate(lines) if "nano light saved:" in l)
opened_idx = max(
    (i for i, l in enumerate(lines) if i > saved_idx and "nano light: opened" in l),
    default=None,
)
need(opened_idx is not None, "no reopen after the save (nano_light_reopen window)")
evidences = []
for l in lines[opened_idx:]:
    if "nano light evidence: " in l:
        try:
            evidences.append(json.loads(l.split("nano light evidence: ", 1)[1]))
        except json.JSONDecodeError:
            pass
need(evidences, "reopen window logged no evidence")
need(
    any(marker in ln.get("t", "") for ev in evidences for ln in ev.get("lines", [])),
    f"reopen evidence does not contain the marker {marker!r} — saved file did not re-render the edit",
)
print(
    f"NL OK first-draw={ms}ms edits={len(edits)} sel-span={min(spans)}..{max(spans)} "
    f"compose={compose_ends[-1]!r} clip-ops={len(clips)} media={len(after)}B-exact "
    f"reopen-evidence-marker=yes (parse-back via crate CLI)"
)
PY
)
  local rc=$?
  SMOKE_SUMMARY="$SMOKE_NL_REPORT"
  [ "$rc" -ne 0 ] && SMOKE_ABORT=1 # asserts bit — stop polling (negatives land here)
  return "$rc"
}

if smoke_poll "$TIMEOUT" nano_light_signals "Nano Light edit round-trip evidence"; then
  echo "smoke: ✓ first-draw < ${MAX_MS} ms · ✓ edit chain (insert/backspace/delete/enter/undo/redo) · ✓ selection span ≥ ${MIN_SPAN} + Cmd-A cross-para · ✓ composition commits '${COMPOSE}' (1 undo unit) · ✓ clipboard copy→paste→copy-all round · ✓ type-replace → save (write_docx + atomic replace) · ✓ parse-back contains marker + 'paragraph${REPL}' + '${COMPOSE}' + paste result · ✓ media byte-exact · ✓ reopen renders the edit"
  grep -m1 "$SMOKE_RE_NL_FIRST_DRAW" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  grep -m1 "$SMOKE_RE_NL_SEL" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  grep -m1 "$SMOKE_RE_NL_COMPOSE" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  grep -m1 "$SMOKE_RE_NL_CLIP" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  grep -m1 "$SMOKE_RE_NL_SAVED" "$LOG" | sed 's/^/smoke:   /' | cut -c1-160
  echo "smoke: PASSED — full log kept at $LOG"
  exit 0
fi
smoke_fail_tail "$LOG"
exit 1
