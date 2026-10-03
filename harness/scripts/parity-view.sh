#!/bin/bash
# NanoOffice parity harness — A3/V4 (T-V4): viewer canvas render ↔ sdkjs render,
# measured LINE-BY-LINE (DD docs/design/viewer-a3/DD.md §4 V4 + §5, OQ-V01-1).
#
# ORACLE (clean-room oracle-obs — no upstream source is read): ONLYOFFICE.app
# boots with `--remote-debugging-port=9222` (technique docs/diagnostics.md
# 22.09/27.09); for each corpus doc the harness walks the live document via
# CDP `callCommand` (Document Builder API) and a COLLAPSED-CURSOR PROBE:
# `GetRange(i, i).Select()` + `asc_GetSelectionBounds()` returns the cursor
# point (x, line-box top) — per-character y jumps mark line breaks, so the
# engine's line count, per-line text (paragraph-text slice) and line start
# x/y come straight from the real renderer's layout. Image extents come from
# the Builder `ToJSON` (`extent.cx/cy` EMU). All coordinates are CSS px at
# zoom 100% — the same space the viewer draws in.
#
# VIEWER side: boot the app with NANO_OFFICE_QUICK_VIEW (no engine) and read
# the `viewer evidence:` JSON line (src/viewer.ts — change both in the SAME
# commit) + the `viewer media:` lines (gate-12 contract).
#
# METRIC per corpus file (embedded python3 below):
#   M0 paragraph text sequence (normalized)   — HARD (exact; numbering-mark
#      prefixes are classified: the engine GetText renders list marks, the
#      viewer does not draw numbering)
#   M1 line count per paragraph               — diff must be CLASSIFIED
#                                               (wrap ceiling: font-metric)
#   M2 per-line text (same-count paras)       — CLASSIFIED (break-point drift
#      = wrap ceiling; content integrity is M0's job) / HARD in negative mode
#   M3 within-paragraph line spacing ±ε (j>0) — HARD, ε = 2.0px (measured:
#      max 0.64px drift on the uniform corpus — DD V01 §5)
#      first-line (j=0) dy/dx drift = CLASSIFIED (w:spacing + margin/column
#      not in the model); shift-corrected: both sides anchored to their own
#      first line, re-anchored after skipped table/image blocks; the absolute
#      offset (margin + page placement) is reported as ceiling
#   M4 image extents EMU                      — HARD; image position is
#                                               CLASSIFIED (viewer own-line
#                                               ceiling, DD V01 §5.4)
# Taxonomy: HARD fail = bug-viewer; classified = ceiling đã ghi (wrap, spacing,
# numbering, margin, image own-line, table cells) — reported, not hidden.
# Tables are skipped by BOTH walkers (oracle does not enter cells; viewer
# marks `c:1`) — counted ceiling.
#
# Usage: parity-view.sh [file.docx ...]
# Env:   PARITY_VIEW_EPS_Y=2.0   baseline tolerance px (chốt bằng đo, DD §5)
#        PARITY_VIEW_EPS_X=2.0   line-start x tolerance px
#        PARITY_VIEW_NEGATIVE=1  inject fake diffs into the viewer evidence —
#                                M2/M3/M4 must ALL bite (the proof run; the
#                                script exits 1 as the asserts demand)
#        PARITY_VIEW_TIMEOUT=120 per-file viewer-boot timeout (s)
# Exit 0 = positive run: every corpus file M0/M2 exact, M3 within ε, M4 exact,
# every other diff classified. Exit 1 = any hard metric failed (negative run
# exits 1 by construction — that IS its proof when all bites landed).
set -u
set -o pipefail  # python|tee must not lose the python exit code (T-V3 gotcha)

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
. "$ROOT/harness/scripts/smoke-lib.sh"

WORK="$(mktemp -d /tmp/nanooffice-parity-view-XXXXXX)"
EPS_Y="${PARITY_VIEW_EPS_Y:-2.0}"
EPS_X="${PARITY_VIEW_EPS_X:-2.0}"
NEGATIVE="${PARITY_VIEW_NEGATIVE:-0}"
TIMEOUT="${PARITY_VIEW_TIMEOUT:-240}"

# ── Corpus (≥5: layout V2 + media V3 + tiếng Việt/full + 2 edge + engine-save)
mkdir -p "$WORK/corpus"
python3 "$ROOT/harness/fixtures/make_docx_layout.py" "$WORK/corpus/layout-fixture.docx" >/dev/null
python3 "$ROOT/harness/fixtures/make_docx_full.py" "$WORK/corpus/full-fixture.docx" >/dev/null
python3 "$ROOT/harness/fixtures/make_docx_media.py" "$WORK/corpus/media-fixture.docx" >/dev/null
python3 "$ROOT/harness/fixtures/make_docx_edge.py" "$WORK/corpus" >/dev/null
cp "$ROOT/harness/fixtures/corpus-a2/engine-save-t011.docx" "$WORK/corpus/"
CORPUS=("$@")
if [ ${#CORPUS[@]} -eq 0 ]; then
  # nested-table + image-multi excluded: nested tables and per-run inline
  # images are structural ceilings of BOTH sides (documented, DD V01 §5.4) —
  # kept out of the default corpus so every row measures real text layout.
  CORPUS=("$WORK/corpus/layout-fixture.docx"
          "$WORK/corpus/full-fixture.docx"
          "$WORK/corpus/media-fixture.docx"
          "$WORK/corpus/edge-multilevel-list.docx"
          "$WORK/corpus/edge-style-override.docx"
          "$WORK/corpus/engine-save-t011.docx")
fi
if [ "$NEGATIVE" = 1 ]; then
  # negative proof runs on the FIRST file only — it is the richest (multi-line
  # wrap + tab + 2 body images), so all three asserts (M2 text, M3 baseline,
  # M4 EMU) bite there; the positive corpus covers the rest (gate-12 pattern:
  # negatives live on thresholds, not on every file)
  CORPUS=("${CORPUS[0]}")
fi

# ── ORACLE extractor (node ≥22, global WebSocket; CDP on 127.0.0.1:9222)
EXTRACT="$WORK/extract.mjs"
cat >"$EXTRACT" <<'NODE'
// Oracle line-walk extractor — T-V4. Usage: node extract.mjs <docx> <out.json>
// Attaches to the ONLYOFFICE CDP target showing <docx>, waits for the doc,
// pins zoom 100%, then walks every body paragraph with collapsed-cursor
// probes (see harness header). Output: { stem, els } JSON.
import { writeFileSync } from "node:fs";

const [, , DOCX, OUT] = process.argv;
const STEM = DOCX.split("/").pop().replace(/\.docx$/i, "");
const BASE = "http://127.0.0.1:9222";
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const findTarget = async (deadlineMs) => {
  const end = Date.now() + deadlineMs;
  for (;;) {
    for (const t of await fetch(`${BASE}/json`).then((r) => r.json())) {
      // doc windows carry `title=<file>.docx` in the boot-page query (the
      // window title itself is the generic "ONLYOFFICE Documents")
      if (t.type === "page" && t.url.includes(`title=${encodeURIComponent(STEM)}.docx`)) return t;
    }
    if (Date.now() > end) throw new Error(`no CDP page target for ${STEM}`);
    await sleep(1000);
  }
};

const target = await findTarget(60000);

// attach + locate the editor frame's main world; the window may still be
// loading when the target appears — retry until the frame and its context exist
let send, evalIn;
for (let attempt = 0; ; attempt++) {
  if (attempt > 30) throw new Error(`editor frame for ${STEM} never appeared`);
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  let id = 0;
  const pending = new Map();
  const events = [];
  send = (method, params = {}) =>
    new Promise((res) => { const mid = ++id; pending.set(mid, res); ws.send(JSON.stringify({ id: mid, method, params })); });
  ws.onmessage = (ev) => {
    const m = JSON.parse(ev.data);
    if (m.id && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); } else events.push(m);
  };
  await send("Runtime.enable");
  await new Promise((r) => setTimeout(r, 1200));
  const ctxs = events.filter((m) => m.method === "Runtime.executionContextCreated").map((m) => m.params.context);
  const tree = await send("Page.getFrameTree");
  const frames = [];
  const walkFrames = (f) => { frames.push(f.frame); (f.childFrames || []).forEach(walkFrames); };
  walkFrames(tree.result.frameTree);
  // the sdkjs editor frame — NOT the boot page (its doctype=word query would
  // also match a lazy /word/ test)
  const frame = frames.find((f) => f.url.includes("documenteditor"));
  const ctx = frame && ctxs.find((c) => c.auxData?.frameId === frame.id && c.auxData?.isDefault !== false && c.name === "");
  if (ctx) {
    evalIn = async (expression) => {
      const r = await send("Runtime.evaluate", { expression, contextId: ctx.id, returnByValue: true, awaitPromise: true });
      if (r.result?.exceptionDetails) throw new Error("eval: " + JSON.stringify(r.result.exceptionDetails).slice(0, 400));
      return r.result?.result?.value;
    };
    break;
  }
  await new Promise((r) => setTimeout(r, 1000)).then(() => ws.close());
}
// one builder command; fnSrc has its ${} holes pre-interpolated (closures do
// not survive serialization into the editor sandbox)
const pcmd = (fnSrc) => evalIn(
  `(async () => await new Promise((res) => Asc.editor.callCommand(function() { ${fnSrc} }, res, res)))()`,
);

// wait for the document to be parsed (GetElementsCount > 0)
const ready = await evalIn(
  `(async () => { for (let i = 0; i < 60; i++) {
     const n = await new Promise((res) => Asc.editor.callCommand(function() { try { return Api.GetDocument().GetElementsCount(); } catch (e) { return 0; } }, res, res));
     if (n > 0) return n; await new Promise((r) => setTimeout(r, 1000));
   } return 0; })()`,
);
if (!ready) throw new Error(`document ${STEM} never became ready`);
await evalIn(`Asc.editor.zoom100(); 1`); // pin CSS px @96dpi — viewer space
await sleep(800);

// 1. enumerate body elements: paragraphs (+text +image extents) / tables
const els = JSON.parse(await pcmd(`
  const d = Api.GetDocument(); const els = [];
  for (let i = 0; i < d.GetElementsCount(); i++) {
    const el = d.GetElement(i); const cls = el.GetClassType();
    if (cls === 'paragraph') {
      let images = [];
      try {
        const find = (o) => { if (!o || typeof o !== 'object') return;
          if (o.extent && o.graphic) images.push([o.extent.cx, o.extent.cy]);
          for (const k in o) find(o[k]); };
        find(JSON.parse(el.ToJSON()));
      } catch (e) {}
      els.push({ i, cls, text: el.GetText(), images });
    } else els.push({ i, cls });
  }
  return JSON.stringify(els);
`));

// 2. collapsed-cursor probe: one CDP eval = select char (command 1) + wait a
//    drawn frame + read the cursor point (command 2). The bounds cache is
//    refreshed ASYNC by the cursor draw — a same-tick read returns the
//    PREVIOUS position. rAF + settle usually suffices; verifyBoundary catches
//    the residual staleness deterministically (retry, then fail loudly).
const probe = async (pi, ci) =>
  evalIn(
    `(async () => {
       await new Promise((res) => Asc.editor.callCommand(function() { Api.GetDocument().GetElement(${pi}).GetRange(${ci}, ${ci}).Select(); return 1; }, res, res));
       await new Promise((r) => requestAnimationFrame(() => setTimeout(r, 60)));
       return await new Promise((res) => Asc.editor.callCommand(function() { const b = editor.asc_GetSelectionBounds(); return b[0][0] + "," + b[0][1]; }, res, res));
     })()`,
  ).then((s) => s.split(",").map(Number));
const sleepMs = (ms) => new Promise((r) => setTimeout(r, ms));
// one verification triple: caret s on line cy, p NOT on it, p−1 on it. A
// stale read anywhere breaks the triple; the caller then adopts the fresh
// line-start reading as the new baseline and redoes the bisect.
const verifyTriple = async (pi, s, p) => {
  const [xs, ys] = await probe(pi, s);
  const [, yp] = await probe(pi, p);
  const [, yp1] = await probe(pi, p - 1);
  return { x: xs, y: ys, on: Math.abs(yp - ys) > 0.51 && Math.abs(yp1 - ys) <= 0.51 };
};

// 3. line walk per non-empty paragraph: bisect line boundaries in
//    CARET-POSITION space (y flips), then read each line's TEXT via its own
//    selection (`GetRange(s, e)` + `asc_GetSelectedText`). Position semantics
//    (measured oracle-obs): positions count invisible content elements the
//    engine inserts when it re-splits runs, so a position NEVER maps to a
//    GetText index — the selection text read is the only sound text source.
const lineText = async (pi, s, e) =>
  evalIn(
    `(async () => {
       await new Promise((res) => Asc.editor.callCommand(function() { Api.GetDocument().GetElement(${pi}).GetRange(${s}, ${e}).Select(); return 1; }, res, res));
       return await new Promise((res) => Asc.editor.callCommand(function() { return editor.asc_GetSelectedText(); }, res, res));
     })()`,
  );
for (const el of els) {
  if (el.cls !== "paragraph") continue;
  const text = el.text.replace(/[\r\n]/g, "");
  el.lines = [];
  if (!text) continue;
  const T = text.length;
  let s = 0;
  let [x, y] = await probe(el.i, s);
  const bounds = [];
  while (s < T) {
    // first caret position on a different line (y = line-box top). A stale
    // read (initial baseline or mid-bisect) poisons the boundary — each
    // attempt re-bisects against the freshest line-start reading and verifies
    // with the triple; 4 attempts, then fail loudly.
    let got = null;
    for (let attempt = 0; attempt < 4 && !got; attempt++) {
      let lo = s + 1;
      let hi = T;
      let hasBoundary = true;
      if (lo < hi) {
        const [, ly] = await probe(el.i, hi);
        if (Math.abs(ly - y) <= 0.51) {
          lo = hi; // whole remainder on one line
          hasBoundary = false;
        } else {
          while (lo < hi) {
            const mid = (lo + hi) >> 1;
            const [, my] = await probe(el.i, mid);
            if (Math.abs(my - y) <= 0.51) lo = mid + 1; else hi = mid;
          }
        }
      }
      const t = await verifyTriple(el.i, s, lo);
      // boundary: s on the line, lo NOT, lo−1 on it. No boundary (whole
      // remainder one line): s and lo must be on the SAME line.
      const ok = hasBoundary
        ? (Math.abs(t.y - y) <= 0.51 && t.on)
        : Math.abs(t.y - y) <= 0.51 && !t.on;
      if (ok) {
        got = { end: hasBoundary ? lo : T, x: t.x, y: t.y };
      } else {
        [x, y] = [t.x, t.y]; // stale baseline — adopt fresh, redo
        await sleepMs(300);
      }
    }
    if (!got) {
      throw new Error(`unconfirmed line boundary in ${STEM} para ${el.i} at caret ${lo} — oracle reads are stale`);
    }
    bounds.push({ s, end: got.end, x: got.x, y: got.y });
    s = got.end;
    if (s < T) [x, y] = await probe(el.i, s);
  }
  for (const b of bounds) {
    b.t = await lineText(el.i, b.s, b.end);
  }
  el.lines = bounds.map((b) => ({ x: b.x, y: b.y, t: b.t }));
}

writeFileSync(OUT, JSON.stringify({ stem: STEM, els }));
console.log(`oracle: ${STEM} els=${els.length} lines=${els.reduce((a, e) => a + (e.lines?.length ?? 0), 0)} images=${els.reduce((a, e) => a + (e.images?.length ?? 0), 0)}`);
process.exit(0);
NODE

# quit the oracle app and WAIT for the CDP port to close (osascript quit can
# silently no-op on a never-activated CEF app — measured 03.10 — so TERM on
# the listening pid is the fallback; graceful close first, hard last)
quit_oracle() {
  osascript -e 'quit app "ONLYOFFICE"' >/dev/null 2>&1
  for _ in $(seq 1 10); do
    lsof -nP -iTCP:9222 -sTCP:LISTEN >/dev/null 2>&1 || return 0
    sleep 1
  done
  local pid
  pid="$(lsof -nP -iTCP:9222 -sTCP:LISTEN -t 2>/dev/null | head -1)"
  [ -n "$pid" ] && kill -TERM "$pid" 2>/dev/null
  for _ in $(seq 1 5); do
    lsof -nP -iTCP:9222 -sTCP:LISTEN >/dev/null 2>&1 || return 0
    sleep 1
  done
  return 1
}

# ── ORACLE app: fresh instance every run (stale windows would shadow the
# target lookup) — quit any CDP-listening leftover, boot in background (rule 6b)
if lsof -nP -iTCP:9222 -sTCP:LISTEN >/dev/null 2>&1; then
  quit_oracle || { echo "parity-view: port 9222 still busy after quit — close ONLYOFFICE and retry"; exit 2; }
fi
open -g -a ONLYOFFICE --args --remote-debugging-port=9222
for _ in $(seq 1 30); do
  lsof -nP -iTCP:9222 -sTCP:LISTEN >/dev/null 2>&1 && break
  sleep 1
done
lsof -nP -iTCP:9222 -sTCP:LISTEN >/dev/null 2>&1 \
  || { echo "parity-view: ONLYOFFICE CDP :9222 did not come up"; exit 2; }
trap 'quit_oracle >/dev/null 2>&1; [ "${PARITY_VIEW_KEEP:-0}" = 1 ] || rm -rf "$WORK"' EXIT

# ── per corpus file: viewer evidence (boot our app) + oracle walk (open -g)
TABLE="$WORK/table.txt"
FAILS=0
for f in "${CORPUS[@]}"; do
  name="$(basename "$f" .docx)"

  # viewer side — boot app nền (rule 6b), poll for the evidence line
  VLOG="$(mktemp /tmp/nanooffice-parity-viewer-XXXXXX)"
  smoke_init "$f"
  smoke_boot "NANO_OFFICE_QUICK_VIEW=$f" "NANO_OFFICE_DEVTOOLS=0" "RUST_LOG=info" "$VLOG"
  parity_view_signals() {
    if grep -q "$SMOKE_RE_QV_FAILED" "$VLOG" 2>/dev/null; then
      SMOKE_ABORT=1
      SMOKE_SUMMARY="viewer error page"
      return 1
    fi
    grep -q "$SMOKE_RE_QV_FIRST_DRAW" "$VLOG" 2>/dev/null || { SMOKE_SUMMARY="no first-draw yet"; return 1; }
    grep -q "viewer evidence:" "$VLOG" 2>/dev/null || { SMOKE_SUMMARY="no evidence line yet"; return 1; }
    SMOKE_SUMMARY="evidence ok"
    return 0
  }
  if ! smoke_poll "$TIMEOUT" parity_view_signals "viewer evidence for $name"; then
    echo "parity-view: FAILED — viewer side ($name): $SMOKE_SUMMARY"
    smoke_fail_tail "$VLOG"
    exit 1
  fi
  sed -n 's/^.*viewer evidence: //p' "$VLOG" | tail -1 >"$WORK/viewer-$name.json"
  # media lines (gate-12 contract) ride along for M4
  grep -o 'viewer media: part=[^ ]* emu=[0-9]*x[0-9]*' "$VLOG" \
    | sed -E 's/.*part=([^ ]+) emu=([0-9]+)x([0-9]+).*/{"part":"\1","cx":\2,"cy":\3}/' \
    | paste -sd, - | sed 's/^/[/; s/$/]/' >"$WORK/media-$name.json"
  smoke_cleanup
  sleep 1

  # oracle side — open the file (background) + walk it
  open -g -a ONLYOFFICE "$f"
  if ! node "$EXTRACT" "$f" "$WORK/oracle-$name.json"; then
    echo "parity-view: FAILED — oracle side ($name)"
    exit 1
  fi

  # compare (python writes the detail log; its exit code decides — captured
  # directly, never through a pipe)
  PARITY_VIEW_NAME="$name" PARITY_VIEW_EPS_Y="$EPS_Y" PARITY_VIEW_EPS_X="$EPS_X" \
    PARITY_VIEW_NEGATIVE="$NEGATIVE" python3 - "$WORK" "$name" >"$WORK/cmp-$name.log" 2>&1 <<'PY'
import json, os, re, sys

work, name = sys.argv[1], sys.argv[2]
eps_y = float(os.environ["PARITY_VIEW_EPS_Y"])
eps_x = float(os.environ["PARITY_VIEW_EPS_X"])
negative = os.environ["PARITY_VIEW_NEGATIVE"] == "1"

def norm(s):
    s = s.replace("\r", " ").replace("\t", " ").replace("\n", " ")
    return re.sub(r" +", " ", s).strip()

viewer = json.load(open(f"{work}/viewer-{name}.json"))
oracle = json.load(open(f"{work}/oracle-{name}.json"))
try:
    media = json.load(open(f"{work}/media-{name}.json"))
except Exception:
    media = []

vlines = [l for l in viewer["lines"] if not l.get("c")]
cell_lines = [l for l in viewer["lines"] if l.get("c")]

# viewer paragraphs: consecutive same-pi text lines grouped
vparas = []
for l in vlines:
    if vparas and vparas[-1]["pi"] == l["pi"]:
        vparas[-1]["lines"].append(l)
        vparas[-1]["text"] += l["t"]
    else:
        vparas.append({"pi": l["pi"], "text": l["t"], "lines": [l]})

oparas = [e for e in oracle["els"] if e["cls"] == "paragraph"]
otables = [e for e in oracle["els"] if e["cls"] != "paragraph"]
# image-only paragraphs are judged by M4 (no line walk either side)
oparas_cmp = [e for e in oparas if norm(e["text"]) or not e.get("images")]

if negative:
    # fake-diff injection, aimed at lines that ARE compared: corrupt + shift a
    # line of the second comparable paragraph (the first comparable line is
    # the shift anchor), break one image EMU. The checks below count bites ON
    # THESE TARGETS only — natural wrap drift also tags [M2 BITE] in negative
    # mode and must not count as proof.
    inj = {}
    comparable = [k for k, (o, v) in enumerate(zip(oparas_cmp, vparas))
                  if len(o.get("lines") or []) == len(v["lines"])]
    if len(comparable) >= 2:
        tgt = vparas[comparable[1]]["lines"]
        li = 0 if len(tgt) == 1 else 1
        tgt[li]["y"] += 3.0
        tgt[li]["t"] += " XQ"
        vparas[comparable[1]]["text"] += " XQ"
        inj = {"pi": comparable[1], "li": li}
        print("NEGATIVE injected: +3.0px baseline shift + line text corruption")
    if media:
        media[0]["cx"] += 9525  # one px of EMU
        print("NEGATIVE injected: image EMU off by one px")

# doc-order block classes for flow re-anchoring: "t"able / "i"mage / "p"lain
block_kind = [("t" if e["cls"] != "paragraph" else ("i" if e.get("images") else "p"))
              for e in oracle["els"]]
opar_docidx = []  # doc index of each oparas_cmp entry
_k = list(block_kind)
for _di, e in enumerate(oracle["els"]):
    if e["cls"] == "paragraph" and (norm(e["text"]) or not e.get("images")):
        opar_docidx.append(_di)

hard, classified = [], []
maxdy = maxdx = 0.0
n_eq = n_lines = 0
shift = xshift = None

# M0 — paragraph text sequence (numbering-mark prefix = classified ceiling:
# the engine's GetText renders the list mark, the viewer does not draw marks)
ov = [norm(e["text"]) for e in oparas_cmp]
vv = [norm(p["text"]) for p in vparas]
NUMBERING_HEAD = re.compile(r"^[0-9a-z ivx.)\u2022\u2013-]{1,8}$", re.I)
numbered = []
if ov != vv:
    for i, (a, b) in enumerate(zip(ov, vv)):
        head, _, tail = a.partition(" ")
        if tail == b and head and NUMBERING_HEAD.match(head):
            numbered.append(i)
            ov[i] = b
        elif a != b:
            hard.append(f"M0 para#{i}: oracle={a[:60]!r} viewer={b[:60]!r}")
            break
    if not numbered and len(ov) != len(vv):
        hard.append(f"M0 para count oracle={len(ov)} viewer={len(vv)}")
    if numbered:
        classified.append(f"{len(numbered)} numbered paragraph(s): engine GetText carries the"
                          " list mark, viewer does not render numbering — numbering ceiling")

# M1/M2/M3 — per paragraph
for i, (o, v) in enumerate(zip(oparas_cmp, vparas)):
    ol, vl = o.get("lines") or [], v["lines"]
    n_lines += len(vl)
    # blocks skipped between this and the previous compared paragraph
    # (tables, image-only paragraphs) break the flow — re-anchor after them
    lo_bound = opar_docidx[i - 1] + 1 if i > 0 else 0
    structural_between = any(k in ("t", "i") for k in block_kind[lo_bound:opar_docidx[i]])
    if len(ol) != len(vl):
        classified.append(
            f"para#{i} {norm(o['text'])[:28]!r}: lines oracle={len(ol)} viewer={len(vl)}"
            " — wrap ceiling (font-metric), text content identical (M0)")
        continue
    n_eq += len(vl)
    for j, (lo, lv) in enumerate(zip(ol, vl)):
        if norm(lo["t"]) != norm(lv["t"]):
            if negative:
                hard.append(f"M2 para#{i} line#{j}: oracle={norm(lo['t'])[:50]!r}"
                            f" viewer={norm(lv['t'])[:50]!r} [M2 BITE]")
            else:
                # same paragraph text (M0), different break points — the
                # advance metrics differ slightly between the two text stacks
                classified.append(f"para#{i} line#{j}: break-point drift (wrap ceiling,"
                                  f" font-metric): oracle={norm(lo['t'])[:36]!r}"
                                  f" viewer={norm(lv['t'])[:36]!r}")
        if structural_between and j == 0:
            # flow restarts after a skipped table / image line — re-anchor the
            # shift here (classified: the blocks in between are ceilings)
            shift, xshift = lv["y"] - lo["y"], lv["x"] - lo["x"]
            classified.append(f"para#{i}: shift re-anchored after skipped table/image"
                              " block — flow-offset ceiling")
            continue
        if shift is None:
            shift, xshift = lv["y"] - lo["y"], lv["x"] - lo["x"]
            continue
        dy, dx = abs((lv["y"] - shift) - lo["y"]), abs((lv["x"] - xshift) - lo["x"])
        maxdy, maxdx = max(maxdy, dy), max(maxdx, dx)
        if dy > eps_y:
            if negative:
                hard.append(f"M3 para#{i} line#{j}: |dy|={dy:.2f}>{eps_y} (oracle y={lo['y']}"
                            f" viewer y={lv['y']}) [M3 BITE]")
            elif j > 0 and i not in numbered:
                # WITHIN a plain paragraph the line-height must track the
                # oracle — this is the core font-metric claim (ε measured, DD §5)
                hard.append(f"M3 para#{i} line#{j}: |dy|={dy:.2f}>{eps_y} within paragraph"
                            f" (oracle y={lo['y']} viewer y={lv['y']})")
            elif i in numbered:
                classified.append(f"para#{i} line#{j}: |dy|={dy:.2f}px under numbering —"
                                  " numbering ceiling (level spacing/indent not modeled)")
            else:
                # first line of a paragraph: w:spacing before/after is not in
                # the model (DD V01 §3.2) — the flow gap at paragraph bounds
                # is a documented ceiling
                classified.append(f"para#{i} line#{j}: |dy|={dy:.2f}px at paragraph bound —"
                                  " spacing ceiling (w:spacing not in model)")
        if dx > eps_x:
            if i in numbered:
                # the mark + its indent shift every line — numbering ceiling
                classified.append(f"para#{i} line#{j}: |dx|={dx:.2f}px under numbering — numbering ceiling")
            elif lv.get("al") in ("center", "right") and not negative:
                # centered/right x depends on the column box (pgMar + page
                # placement) — margin-model ceiling, DD V01 (pgMar not in model)
                classified.append(f"para#{i} line#{j}: |dx|={dx:.2f}px on {lv.get('al')}-aligned"
                                  " line — margin/column ceiling")
            else:
                hard.append(f"M3x para#{i} line#{j}: |dx|={dx:.2f}>{eps_x}"
                            f"{' [M3 BITE]' if negative else ''}"
                            + ("" if negative else " (left-aligned line)"))

# M4 — image extents (EMU exact); position = classified own-line ceiling
oimgs = [im for e in oparas for im in e.get("images", [])]
if len(media) != len(oimgs):
    hard.append(f"M4 image count oracle={len(oimgs)} viewer={len(media)}{' [M4 BITE]' if negative else ''}")
else:
    for k, (im, me) in enumerate(zip(oimgs, media)):
        if im[0] != me["cx"] or im[1] != me["cy"]:
            hard.append(f"M4 image#{k} extent oracle={im} viewer emu={me['cx']}x{me['cy']}"
                        f"{' [M4 BITE]' if negative else ''}")

if cell_lines:
    classified.append(f"{len(cell_lines)} table-cell lines skipped by both walkers (table ceiling)")
if otables:
    classified.append(f"{len(otables)} table block(s) not line-walked (table ceiling)")
# absolute first-line offset: margin model ceiling (viewer 1in+16px fixed
# margin, engine real pgMar) — reported, not gated
offset = ""
if shift is not None:
    offset = f" offset(y={shift:+.1f},x={xshift:+.1f}px margin-ceiling)"

bites = 0
if negative:
    # count bites on the INJECTED diffs: the corrupted text always surfaces as
    # an M0 paragraph mismatch (the [:50]-truncated M2 message may hide the
    # "XQ" marker), the shifted baseline as an M3 report on the injected line
    # (absolute |dy| varies with natural drift — match the line), the broken
    # EMU as M4. Natural wrap drift also tags [M2 BITE] and never counts.
    if any(h.startswith("M0") for h in hard):
        bites += 1
    if inj and any(h.startswith(f"M2 para#{inj['pi']} line#{inj['li']}")
                   or h.startswith(f"M3 para#{inj['pi']} line#{inj['li']}") for h in hard):
        bites += 1
    if any(h.startswith("M4") for h in hard):
        bites += 1
    need = 2 + (1 if media else 0)
    if bites < need:
        hard.append(f"NEGATIVE INCOMPLETE: only {bites} injected diffs were caught"
                    f" (of {need}) — gate is blind somewhere")
        print("\n".join(f"parity-view[{name}]: {h}" for h in hard))
        print(f"ROW {name} NEGATIVE-BROKEN")
        sys.exit(1)
    print(f"parity-view[{name}]: negative proof — injected diffs caught"
          f" (M0 text, M3 baseline{', M4 EMU' if media else ''})")
    print(f"ROW {name} NEGATIVE-OK")
    sys.exit(0)

print("\n".join(f"parity-view[{name}]: HARD {h}" for h in hard))
print("\n".join(f"parity-view[{name}]: classified {c}" for c in classified))
verdict = "PASS" if not hard else "FAIL"
print(f"ROW {name} paras={len(vv)}/{len(ov)} lines={n_eq}/{n_lines}"
      f" maxdy={maxdy:.2f} maxdx={maxdx:.2f}px imgs={len(media)}/{len(oimgs)}{offset} {verdict}")
sys.exit(0 if not hard else 1)
PY
  cmp_rc=$?
  tee -a "$WORK/detail.txt" <"$WORK/cmp-$name.log"
  [ "$cmp_rc" -ne 0 ] && FAILS=$((FAILS + 1))
done

echo
echo "parity-view: corpus=${#CORPUS[@]} hard-fails=$FAILS eps=(${EPS_X}, ${EPS_Y})px negative=${NEGATIVE}"
[ -f "$WORK/detail.txt" ] && grep '^ROW' "$WORK/detail.txt" | sed 's/^/  /'
# WORK is removed by the trap — print the artifact dir before exit
if [ "$FAILS" -eq 0 ] && [ "$NEGATIVE" != "1" ]; then
  echo "parity-view: PASSED"
  exit 0
fi
if [ "$NEGATIVE" = "1" ] && [ "$FAILS" -eq 0 ]; then
  echo "parity-view: NEGATIVE proven (all bites landed, exit 1 as designed)"
fi
exit 1
