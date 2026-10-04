//! Nano Light editor command layer (L01, DD `docs/design/editor-a4/DD.md` §3)
//! — the A4 slice plugs in HERE, parallel to delivery (precedent C3): the
//! `Document` model lives Rust-side in window-scoped sessions, mutations
//! arrive as hit-test coordinates (T-V7 `Hit`: body block / run / utf-16
//! offset inside the run text, caret BEFORE char `offset`), and save
//! serializes straight through the A2 crate (`write_docx`) — no Editor.bin,
//! no x2t on this path. The default open path stays the engine (D1 — no
//! hard-cut); Nano Light has its own window, its own start-page button and
//! the env `NANO_OFFICE_NANO_LIGHT` as the dev/gate-15 trigger.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::State;

use nanooffice_ooxml::{Block, Document, Page, Paragraph, RenderDoc, Run};

use crate::protocol::FileRegistry;
use crate::viewer::render_payload;

/// Caret position — the hit-test coordinate system (`hitTest` T-V7, DD V01
/// §5.7): `para` = body block index, `run` = index in the paragraph's runs,
/// `offset` = utf-16 code units into the run's text (mirrors JS string
/// indexing; the caret sits BEFORE char `offset`). T-060 C3: `cell` carries
/// the hit's `[row, col]` table identity (null at top level). T-A30 §5.5a:
/// `cell_para` = the paragraph's index inside that cell's blocks (null at top
/// level / nested — refused); `#[serde(default)]` keeps old payloads parseable.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Caret {
    pub para: usize,
    pub run: usize,
    pub offset: usize,
    #[serde(default)]
    pub cell: Option<[usize; 2]>,
    #[serde(default)]
    pub cell_para: Option<usize>,
}

/// A text selection: `anchor` (where the gesture started) + `focus` (the
/// moving end) — UNORDERED, Rust orders the pair at use time by
/// `(para, run, offset)` (total order over top-level carets, DD L01 §5.3a).
/// `anchor == focus` ≡ plain caret. T-A28 slice A4 #1.
/// TS mirror: `src/editor.ts` `SelRange` (C5 — one camelCase contract home).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelRange {
    pub anchor: Caret,
    pub focus: Caret,
}

/// Snapshot-clone undo bound (DD L01 §2): the model is small, clones are
/// cheap; the oldest snapshot drops beyond this.
const UNDO_CAP: usize = 100;

/// Undo/redo snapshot: model + caret + selection OF THE MOMENT (DD L01
/// §5.3a — undo restores the highlight, not just the text).
type Snapshot = (Document, Caret, Option<SelRange>);

/// One Nano Light session, owned by its window label (pattern T-059 — every
/// command resolves its session by the calling window, never "the latest").
/// Holds the UNRESOLVED `Document` (the write-back-able model — `RenderDoc`
/// has flattened the §17.7.2 style chain and cannot round-trip), the original
/// package bytes (media parts are copied byte-exact on save, T-A26),
/// snapshot-clone undo/redo stacks carrying the caret+selection of their
/// moment, the live selection (`None` = plain caret), and the last-saved
/// snapshot (`dirty = doc != saved` — undoing back to the save point clears
/// it for free).
pub struct EditorSession {
    doc: Document,
    media_src: Vec<u8>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    saved: Option<Document>,
    caret: Caret,
    sel: Option<SelRange>,
    path: PathBuf,
}

impl EditorSession {
    fn dirty(&self) -> bool {
        match &self.saved {
            Some(s) => &self.doc != s,
            None => true,
        }
    }

    /// The session-level edit transition (the command layer is a thin wrap):
    /// apply the mutation, keep the undo/redo bookkeeping, return the new
    /// caret. Mutations push the pre-command snapshot only when they actually
    /// changed the model (a no-op backspace at the document start must not
    /// pollute the stack); undo/redo manage their own stacks. Range-aware
    /// verbs (DD L01 §5.3c) delete the selection FIRST, then mutate at the
    /// range start; the selection collapses on any model change. T-A30
    /// (§5.5b): the verb runs on the blocks the caret addresses — body or cell.
    fn apply(&mut self, cmd: EditCmd) -> Result<Caret, String> {
        // Identity rules (§5.5): a cell caret needs cellPara (null = nested,
        // unaddressable); a selection stays in ONE cell or entirely top level.
        let check = |c: Caret| -> Result<(), String> {
            if c.cell.is_some() && c.cell_para.is_none() {
                return Err(
                    "nano light: in-cell caret without cellPara — nested table content is not editable (DD L01 §5.5)"
                        .to_string(),
                );
            }
            Ok(())
        };
        if let Some(c) = cmd.caret() {
            check(c)?;
        }
        if let Some(r) = cmd.sel() {
            check(r.anchor)?;
            check(r.focus)?;
            if !same_cell(r.anchor.cell, r.focus.cell) {
                return Err(
                    "nano light: selection must stay inside one cell or entirely top level (DD L01 §5.5)"
                        .to_string(),
                );
            }
        }
        let is_mutation = !matches!(cmd, EditCmd::Undo | EditCmd::Redo);
        // The snapshot carries the caret+selection the COMMAND acted at —
        // TS-side caret movement (arrows/clicks) between commands never syncs
        // back to the session (DD L01 §5.3g ceiling), so `self.caret` would
        // be stale exactly when it matters.
        let before = (
            self.doc.clone(),
            cmd.caret().unwrap_or(self.caret),
            cmd.sel().or(self.sel),
        );
        let caret = match cmd {
            EditCmd::Insert { text, caret, sel } => {
                dispatch(&mut self.doc, caret, sel, |bl, c, s| {
                    let (c, _) = collapse_range(bl, c, s)?;
                    apply_insert(bl, c, &text)
                })
            }
            EditCmd::Backspace { caret, sel } => dispatch(&mut self.doc, caret, sel, |bl, c, s| {
                let (c, ranged) = collapse_range(bl, c, s)?;
                if ranged {
                    Ok(c) // the range delete WAS the deletion
                } else {
                    apply_backspace(bl, c)
                }
            }),
            EditCmd::Delete { caret, sel } => dispatch(&mut self.doc, caret, sel, |bl, c, s| {
                let (c, ranged) = collapse_range(bl, c, s)?;
                if ranged {
                    Ok(c)
                } else {
                    apply_delete(bl, c)
                }
            }),
            EditCmd::Enter { caret, sel } => dispatch(&mut self.doc, caret, sel, |bl, c, s| {
                let (c, _) = collapse_range(bl, c, s)?;
                apply_enter(bl, c)
            }),
            EditCmd::Undo => match self.undo.pop() {
                Some((doc, caret, sel)) => {
                    self.redo
                        .push((std::mem::replace(&mut self.doc, doc), self.caret, self.sel));
                    self.sel = sel;
                    Ok(caret)
                }
                None => Ok(self.caret), // empty stack — no-op
            },
            EditCmd::Redo => match self.redo.pop() {
                Some((doc, caret, sel)) => {
                    self.undo
                        .push((std::mem::replace(&mut self.doc, doc), self.caret, self.sel));
                    self.sel = sel;
                    Ok(caret)
                }
                None => Ok(self.caret),
            },
        }?;
        if is_mutation && before.0 != self.doc {
            self.redo.clear();
            self.undo.push(before);
            if self.undo.len() > UNDO_CAP {
                self.undo.remove(0);
            }
            self.sel = None; // a mutation collapses the selection to a caret
        }
        self.caret = caret;
        Ok(caret)
    }
}

/// Window-label → session. The single-slot `QuickViewRequest` race noted in
/// viewer.rs cannot recur here: keys are explicit from the start.
#[derive(Default)]
pub struct EditorSessions(Mutex<HashMap<String, EditorSession>>);

impl EditorSessions {
    /// Drop the session of a closed window (called from the same
    /// `WindowEvent::Destroyed` hook that unregisters registry files).
    pub fn remove_owner(&self, owner: &str) -> usize {
        let mut map = self.0.lock().expect("editor sessions poisoned");
        let before = map.len();
        map.retain(|k, _| k != owner);
        before - map.len()
    }

    /// Run `f` on the calling window's session (T-060 C2/C5-mini): the
    /// poison + no-session error strings have ONE home here — the commands
    /// stopped repeating the lock/get/error mold.
    pub fn with_session<T>(
        &self,
        label: &str,
        f: impl FnOnce(&mut EditorSession) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut map = self
            .0
            .lock()
            .map_err(|_| "nano light: sessions poisoned".to_string())?;
        let session = map
            .get_mut(label)
            .ok_or_else(|| format!("nano light: no session for window {label}"))?;
        f(session)
    }
}

/// Path handoff from the start-page action to the window's single
/// `nano_light_open` call (pattern `QuickViewRequest`).
#[derive(Default)]
pub struct NanoLightRequest(Mutex<Option<String>>);

impl NanoLightRequest {
    fn stash(&self, path: String) {
        *self.0.lock().expect("nano-light request mutex poisoned") = Some(path);
    }

    fn take(&self) -> Option<String> {
        self.0
            .lock()
            .expect("nano-light request mutex poisoned")
            .take()
    }
}

/// Window-label sequence for UI-opened Nano Light windows ("main" = env boot).
static WINDOW_SEQ: AtomicU32 = AtomicU32::new(0);

/// Shared open-a-window body of the two view commands (the window build
/// itself lives in `viewer::open_surface_window`, T-060 C2).
fn open_window(
    app: &tauri::AppHandle,
    request: &NanoLightRequest,
    path: String,
) -> Result<(), String> {
    let label = format!(
        "nanolight-{}",
        WINDOW_SEQ.fetch_add(1, Ordering::Relaxed) + 1
    );
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    request.stash(path.clone());
    crate::viewer::open_surface_window(
        app,
        &label,
        "editor.html",
        format!("NanoOffice — Nano Light: {name}"),
        "nano light",
    )?;
    log::info!("nano light view: {path} (window {label})");
    Ok(())
}

/// UI wiring (pattern V5 `viewer_view`): validate the path, stash it and open
/// a dedicated Nano Light window; the error stays on the start page instead
/// of opening a window just to show a failure.
#[tauri::command]
pub fn nano_light_view(
    app: tauri::AppHandle,
    request: State<'_, Arc<NanoLightRequest>>,
    path: String,
) -> Result<(), String> {
    if !std::path::Path::new(&path).is_file() {
        return Err(format!("nano light: no such file: {path}"));
    }
    open_window(&app, request.inner(), path)
}

/// Reopen THIS session's document in a new Nano Light window (reads the file
/// from disk — the disk path never crosses into JS).
#[tauri::command]
pub fn nano_light_reopen(
    app: tauri::AppHandle,
    request: State<'_, Arc<NanoLightRequest>>,
    sessions: State<'_, Arc<EditorSessions>>,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    let path = sessions.with_session(window.label(), |s| Ok(s.path.clone()))?;
    open_window(&app, request.inner(), path.display().to_string())
}

/// The Nano Light payload — same shape as the viewer's `QuickView` (the two
/// frontends consume one contract), plus the caret and the file name for the
/// window header.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NanoLightDoc {
    pub parse_ms: f64,
    pub doc: RenderDoc,
    pub fonts: BTreeMap<String, [i16; 3]>,
    pub page: Option<Page>,
    pub media_base: Option<String>,
    pub caret: Caret,
    pub name: String,
}

#[tauri::command]
pub fn nano_light_open(
    registry: State<'_, Arc<FileRegistry>>,
    request: State<'_, Arc<NanoLightRequest>>,
    sessions: State<'_, Arc<EditorSessions>>,
    window: tauri::WebviewWindow,
) -> Result<NanoLightDoc, String> {
    // UI path stashes the path and the window consumes it; the env stays as
    // the dev/gate-15 trigger (pattern V5).
    let path = request
        .take()
        .or_else(|| {
            std::env::var("NANO_OFFICE_NANO_LIGHT")
                .ok()
                .filter(|p| !p.is_empty())
        })
        .ok_or_else(|| "nano light: no source (UI action or NANO_OFFICE_NANO_LIGHT)".to_string())?;
    let p = render_payload(
        registry.inner().as_ref(),
        window.label(),
        &path,
        "nano light",
    )?;
    let name = std::path::Path::new(&path)
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.clone());
    let caret = Caret {
        para: 0,
        run: 0,
        offset: 0,
        cell: None,
        cell_para: None,
    };
    let session = EditorSession {
        saved: Some(p.doc.clone()),
        doc: p.doc,
        media_src: p.bytes,
        undo: Vec::new(),
        redo: Vec::new(),
        caret,
        sel: None,
        path: PathBuf::from(&path),
    };
    sessions
        .0
        .lock()
        .map_err(|_| "nano light: sessions poisoned".to_string())?
        .insert(window.label().to_string(), session);
    log::info!(
        "nano light: opened {path} in {:.1} ms (window {})",
        p.parse_ms,
        window.label()
    );
    Ok(NanoLightDoc {
        parse_ms: p.parse_ms,
        doc: p.render,
        fonts: p.fonts,
        page: p.page,
        media_base: p.media_base,
        caret,
        name,
    })
}

/// One mutation command — `caret` fields only where the command needs them.
/// T-A28 §5.3c: the mutating verbs carry the live selection (`sel`) — a
/// non-empty one is deleted BEFORE the verb mutates at the range start.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EditCmd {
    Insert {
        text: String,
        caret: Caret,
        #[serde(default)]
        sel: Option<SelRange>,
    },
    Backspace {
        caret: Caret,
        #[serde(default)]
        sel: Option<SelRange>,
    },
    Delete {
        caret: Caret,
        #[serde(default)]
        sel: Option<SelRange>,
    },
    Enter {
        caret: Caret,
        #[serde(default)]
        sel: Option<SelRange>,
    },
    Undo,
    Redo,
}

impl EditCmd {
    fn label(&self) -> &'static str {
        match self {
            EditCmd::Insert { .. } => "insert",
            EditCmd::Backspace { .. } => "backspace",
            EditCmd::Delete { .. } => "delete",
            EditCmd::Enter { .. } => "enter",
            EditCmd::Undo => "undo",
            EditCmd::Redo => "redo",
        }
    }

    /// The caret the command acts on (None for undo/redo — they carry their
    /// own snapshot carets).
    fn caret(&self) -> Option<Caret> {
        match self {
            EditCmd::Insert { caret, .. }
            | EditCmd::Backspace { caret, .. }
            | EditCmd::Delete { caret, .. }
            | EditCmd::Enter { caret, .. } => Some(*caret),
            EditCmd::Undo | EditCmd::Redo => None,
        }
    }

    /// The selection the command acts on (None for undo/redo).
    fn sel(&self) -> Option<SelRange> {
        match self {
            EditCmd::Insert { sel, .. }
            | EditCmd::Backspace { sel, .. }
            | EditCmd::Delete { sel, .. }
            | EditCmd::Enter { sel, .. } => *sel,
            EditCmd::Undo | EditCmd::Redo => None,
        }
    }
}

/// What every edit returns: the re-resolved render model (the frontend draws
/// from this — it merges nothing), the new caret, dirty, and the live
/// selection (collapsed after a mutation; restored by undo/redo — DD L01
/// §5.3a).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditOutcome {
    pub doc: RenderDoc,
    pub caret: Caret,
    pub dirty: bool,
    pub sel: Option<SelRange>,
}

#[tauri::command]
pub fn nano_light_edit(
    sessions: State<'_, Arc<EditorSessions>>,
    window: tauri::WebviewWindow,
    cmd: EditCmd,
) -> Result<EditOutcome, String> {
    let label = cmd.label();
    let (caret, dirty, undo_len, sel, render) = sessions.with_session(window.label(), |s| {
        let caret = s.apply(cmd)?;
        Ok((
            caret,
            s.dirty(),
            s.undo.len(),
            s.sel,
            nanooffice_ooxml::resolve(&s.doc),
        ))
    })?;
    log::info!(
        "nano light edit: cmd={label} caret={}/{}/{} undo={undo_len} dirty={dirty}",
        caret.para,
        caret.run,
        caret.offset
    );
    Ok(EditOutcome {
        doc: render,
        caret,
        dirty,
        sel,
    })
}

/// Serialize the session's `Document` straight through the A2 crate and swap
/// it over the original atomically (tmp next to the target, then rename —
/// the `bridge/save.rs` pattern). Media parts are copied byte-exact from the
/// ORIGINAL package bytes the session kept (`media_src`, T-A26).
#[tauri::command]
pub fn nano_light_save(
    sessions: State<'_, Arc<EditorSessions>>,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    sessions.with_session(window.label(), save_session)
}

fn save_session(session: &mut EditorSession) -> Result<(), String> {
    let started = std::time::Instant::now();
    let out = nanooffice_ooxml::write_docx(&session.doc, Some(&session.media_src))
        .map_err(|e| format!("nano light: write docx: {e}"))?;
    atomic_replace(&session.path, &out, "nanooffice-nl", "nano light")?;
    session.saved = Some(session.doc.clone());
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    log::info!(
        "nano light saved: {} ({} bytes) in {ms:.1} ms",
        session.path.display(),
        out.len()
    );
    Ok(())
}

/// Atomic file swap (T-060 C2, pattern `bridge/save.rs:251-254,323`): write
/// to a tmp sibling in the SAME directory (same volume), rename over the
/// target, clean the tmp up when the rename fails. The engine save path does
/// NOT use this — its tmp is written by x2t on disk, not from memory bytes.
fn atomic_replace(
    path: &std::path::Path,
    bytes: &[u8],
    tmp_tag: &str,
    err_tag: &str,
) -> Result<(), String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("docx")
        .to_string();
    let tmp = path.with_extension(format!("{tmp_tag}.{ext}"));
    std::fs::write(&tmp, bytes)
        .map_err(|e| format!("{err_tag}: write tmp {}: {e}", tmp.display()))?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("{err_tag}: rename over original: {e}"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// mutations — free functions over a block list (the body, or one cell's
// blocks — T-A30 §5.5b), hit-test coordinates in/out
// ---------------------------------------------------------------------------

/// Both selection endpoints share one edit surface: both top level, or the
/// SAME cell (cross-paragraph within a cell is fine — DD L01 §5.5c).
fn same_cell(a: Option<[usize; 2]>, b: Option<[usize; 2]>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// Resolve the block list a CELL caret edits (§17.4.38 → §17.4.79 → §17.4.66
/// through the caret's identity): `body[para]` must be a table, `[row, col]` exist.
fn cell_blocks_mut(doc: &mut Document, c: Caret) -> Result<&mut Vec<Block>, String> {
    let [row_idx, col_idx] = c.cell.ok_or_else(|| {
        format!(
            "nano light: caret {}/{}/{} lost its cell identity",
            c.para, c.run, c.offset
        )
    })?;
    let table = match doc.body.get_mut(c.para) {
        Some(Block::Table(t)) => t,
        Some(Block::Paragraph(_)) => {
            return Err(format!(
                "nano light: block {} is a paragraph but the caret carries cell identity",
                c.para
            ));
        }
        None => return Err(format!("nano light: no block {}", c.para)),
    };
    let row = table
        .rows
        .get_mut(row_idx)
        .ok_or_else(|| format!("nano light: no table row {row_idx}"))?;
    let tc = row.get_mut(col_idx).ok_or_else(|| {
        format!(
            "nano light: no cell [{row_idx},{col_idx}] in table block {}",
            c.para
        )
    })?;
    Ok(&mut tc.blocks)
}

/// Run a verb on the blocks the caret addresses (§5.5b): cell carets rebase
/// `para` → cellPara; the full identity (fresh cellPara) rides the returned caret.
fn dispatch(
    doc: &mut Document,
    caret: Caret,
    sel: Option<SelRange>,
    verb: impl FnOnce(&mut Vec<Block>, Caret, Option<SelRange>) -> Result<Caret, String>,
) -> Result<Caret, String> {
    if caret.cell.is_none() {
        return verb(&mut doc.body, caret, sel);
    }
    let blocks = cell_blocks_mut(doc, caret)?;
    let rebase = |c: Caret| Caret {
        para: c.cell_para.unwrap_or(c.para),
        ..c
    };
    let local_sel = sel.map(|r| SelRange {
        anchor: rebase(r.anchor),
        focus: rebase(r.focus),
    });
    let out = verb(blocks, rebase(caret), local_sel)?;
    Ok(Caret {
        para: caret.para,
        // inside the cell `out.para` IS the paragraph index (verbs may move it)
        cell_para: Some(out.para),
        ..out
    })
}

/// Resolve `idx` to a paragraph within `blocks` — a table block here means a
/// stale top-level caret or nested content; both refuse loudly (§5.5).
fn paragraph_at(blocks: &mut [Block], idx: usize) -> Result<&mut Paragraph, String> {
    match blocks.get_mut(idx) {
        Some(Block::Paragraph(p)) => Ok(p),
        Some(Block::Table(_)) => Err(format!(
            "nano light: block {idx} is a table — nested tables are not editable (DD L01 §5.5)"
        )),
        None => Err(format!("nano light: no block {idx}")),
    }
}

/// Map a utf-16 code-unit index (JS string indexing) to a byte offset.
/// Surrogate pairs count 2 units — mirrors `String.length` on the JS side.
fn utf16_to_byte(s: &str, units: usize) -> Result<usize, String> {
    let mut u = 0;
    for (b, c) in s.char_indices() {
        if u >= units {
            return Ok(b);
        }
        u += c.len_utf16();
    }
    if u == units {
        return Ok(s.len());
    }
    Err(format!(
        "nano light: caret offset {units} out of bounds ({u} utf-16 units)"
    ))
}

/// Map a byte offset to utf-16 code units (for caret positions Rust returns).
fn byte_to_utf16(s: &str, byte: usize) -> usize {
    s.char_indices()
        .take_while(|(b, _)| *b < byte)
        .map(|(_, c)| c.len_utf16())
        .sum()
}

/// A non-empty selection collapses to: delete the range, mutate at the range
/// start. Returns `(caret, ranged)` — `ranged` tells the single-char verbs to
/// skip their own step (the range delete WAS the deletion, DD L01 §5.3c).
fn collapse_range(
    blocks: &mut Vec<Block>,
    caret: Caret,
    sel: Option<SelRange>,
) -> Result<(Caret, bool), String> {
    match sel {
        Some(r) if r.anchor != r.focus => {
            Ok((apply_delete_range(blocks, r.anchor, r.focus)?, true))
        }
        _ => Ok((caret, false)),
    }
}

/// Delete everything between two carets (ordered) — cross run AND cross
/// paragraph (DD L01 §5.3c). Semantics: the FIRST paragraph's properties
/// survive the merge (Word mark-deletion direction); a run's images sit at
/// the END of its text (parser flatten, DD §3.1), so a run's images die
/// exactly when the range covers that run's text end. Any table inside the
/// span refuses the WHOLE command — a silent table drop would be data loss.
/// Empty runs are kept (no splicing) so the returned caret
/// `(start.para, start.run, start.offset)` always addresses a valid spot.
fn apply_delete_range(blocks: &mut Vec<Block>, a: Caret, b: Caret) -> Result<Caret, String> {
    // document-order key (para, run, offset) — a total order over top-level
    // carets (cell carets are refused upstream, DD L01 §5.3a)
    let (start, end) = if (a.para, a.run, a.offset) <= (b.para, b.run, b.offset) {
        (a, b)
    } else {
        (b, a)
    };
    if start.para >= blocks.len() || end.para >= blocks.len() {
        return Err(format!(
            "nano light: range endpoints {}..{} outside the block list ({} blocks)",
            start.para,
            end.para,
            blocks.len()
        ));
    }
    if blocks[start.para..=end.para]
        .iter()
        .any(|blk| matches!(blk, Block::Table(_)))
    {
        return Err(format!(
            "nano light: range {}..{} spans a table — selection cannot enter tables (round 1, DD L01 §5.3)",
            start.para, end.para
        ));
    }
    if start.para == end.para {
        let p = paragraph_at(blocks, start.para)?;
        if end.run >= p.runs.len() {
            return Err(format!(
                "nano light: no run {} in paragraph {}",
                end.run, start.para
            ));
        }
        let end_len = p.runs[end.run].text.encode_utf16().count();
        if start.run == end.run {
            let run = &mut p.runs[start.run];
            let sb = utf16_to_byte(&run.text, start.offset)?;
            let eb = utf16_to_byte(&run.text, end.offset)?;
            run.text.replace_range(sb..eb, "");
            if end.offset == end_len {
                run.images.clear(); // the range reached the run's text end
            }
        } else {
            {
                let head = &mut p.runs[start.run];
                let sb = utf16_to_byte(&head.text, start.offset)?;
                head.text.truncate(sb);
                head.images.clear(); // the range runs past this run's text
            }
            {
                let tail = &mut p.runs[end.run];
                let eb = utf16_to_byte(&tail.text, end.offset)?;
                tail.text.replace_range(0..eb, "");
                if end.offset == end_len {
                    tail.images.clear();
                }
            }
            p.runs.drain(start.run + 1..end.run);
        }
        return Ok(start);
    }
    // head paragraph: keep its prefix (its pPr wins the merge), drop the runs
    // after the caret run — they are inside the range. An empty head
    // paragraph (selection starting at a `w:p` with no runs) has nothing to
    // truncate; it survives with its pPr and adopts the tail runs.
    {
        let p = paragraph_at(blocks, start.para)?;
        if p.runs.is_empty() {
            // nothing to truncate — the paragraph mark merge only
        } else {
            let head = p.runs.get_mut(start.run).ok_or_else(|| {
                format!(
                    "nano light: no run {} in paragraph {}",
                    start.run, start.para
                )
            })?;
            let sb = utf16_to_byte(&head.text, start.offset)?;
            head.text.truncate(sb);
            head.images.clear(); // the range runs past this run's text
            p.runs.truncate(start.run + 1);
        }
    }
    // tail paragraph: keep from end.offset on (its images sit past the cut —
    // kept unless the cut reaches the text end); the runs before end.run die
    // with the paragraph's own pPr.
    let tail_runs: Vec<Run> = {
        let p = paragraph_at(blocks, end.para)?;
        if p.runs.is_empty() {
            Vec::new() // empty tail paragraph: only its block mark is deleted
        } else {
            let tail = p.runs.get_mut(end.run).ok_or_else(|| {
                format!("nano light: no run {} in paragraph {}", end.run, end.para)
            })?;
            let eb = utf16_to_byte(&tail.text, end.offset)?;
            let keep_images = end.offset < tail.text.encode_utf16().count();
            tail.text.replace_range(0..eb, "");
            if !keep_images {
                tail.images.clear();
            }
            p.runs.split_off(end.run)
        }
    };
    // the blocks strictly between (head exclusive, tail inclusive) go away;
    // the tail's surviving runs join the head paragraph
    blocks.drain(start.para + 1..=end.para);
    if let Some(Block::Paragraph(p)) = blocks.get_mut(start.para) {
        p.runs.extend(tail_runs);
    }
    Ok(start)
}

fn apply_insert(blocks: &mut [Block], caret: Caret, text: &str) -> Result<Caret, String> {
    // Break controls are round-tripped inside run text by the parser, but
    // typed input must not smuggle them in (they would desync the layout's
    // tokenize) — `enter` is the only sanctioned paragraph break.
    if text
        .chars()
        .any(|c| matches!(c, '\t' | '\n' | '\u{c}' | '\u{b}'))
    {
        return Err("nano light: insert rejects control characters (use enter for breaks)".into());
    }
    let p = paragraph_at(blocks, caret.para)?;
    if p.runs.is_empty() {
        if caret.run != 0 || caret.offset != 0 {
            return Err(format!(
                "nano light: caret {}/{}/{} on an empty paragraph",
                caret.para, caret.run, caret.offset
            ));
        }
        let units = text.encode_utf16().count();
        p.runs.push(Run {
            text: text.to_string(),
            ..Default::default()
        });
        return Ok(Caret {
            offset: units,
            ..caret
        });
    }
    let run = p.runs.get_mut(caret.run).ok_or_else(|| {
        format!(
            "nano light: no run {} in paragraph {}",
            caret.run, caret.para
        )
    })?;
    let byte = utf16_to_byte(&run.text, caret.offset)?;
    run.text.insert_str(byte, text);
    Ok(Caret {
        offset: caret.offset + text.encode_utf16().count(),
        ..caret
    })
}

fn apply_backspace(blocks: &mut Vec<Block>, caret: Caret) -> Result<Caret, String> {
    if caret.offset > 0 {
        let p = paragraph_at(blocks, caret.para)?;
        let run = p.runs.get_mut(caret.run).ok_or_else(|| {
            format!(
                "nano light: no run {} in paragraph {}",
                caret.run, caret.para
            )
        })?;
        // remove the char BEFORE the caret — a surrogate pair is one char
        // (2 utf-16 units): the returned caret drops by its unit length
        let end = utf16_to_byte(&run.text, caret.offset)?;
        let start = run.text[..end]
            .char_indices()
            .next_back()
            .map(|(b, _)| b)
            .unwrap_or(0);
        let units = run.text[start..end]
            .chars()
            .map(|c| c.len_utf16())
            .sum::<usize>();
        run.text.replace_range(start..end, "");
        return Ok(Caret {
            offset: caret.offset - units,
            ..caret
        });
    }
    if caret.run > 0 {
        // join this run's text+images into the previous run, drop it
        let p = paragraph_at(blocks, caret.para)?;
        let cur = p.runs.remove(caret.run);
        let prev = &mut p.runs[caret.run - 1];
        let at = byte_to_utf16(&prev.text, prev.text.len());
        prev.text.push_str(&cur.text);
        prev.images.extend(cur.images);
        return Ok(Caret {
            run: caret.run - 1,
            offset: at,
            ..caret
        });
    }
    if caret.para == 0 {
        return Ok(caret); // start of the block list (document/cell) — no-op
    }
    if !matches!(blocks.get(caret.para - 1), Some(Block::Paragraph(_)))
        || !matches!(blocks.get(caret.para), Some(Block::Paragraph(_)))
    {
        return Ok(caret); // a table sits on the boundary — refuse, no-op
    }
    // Word semantics: deleting a paragraph mark adopts the FOLLOWING
    // paragraph's properties — the caret paragraph's pPr survives (DD §3.1).
    let cur = blocks.remove(caret.para);
    let prev = blocks.remove(caret.para - 1);
    let (mut cur_p, prev_p) = match (cur, prev) {
        (Block::Paragraph(c), Block::Paragraph(p)) => (c, p),
        _ => return Ok(caret), // unreachable: both checked above
    };
    // the caret lands at the junction — the end of the PREVIOUS paragraph's
    // content (runs stay separate; only the block marks merge)
    let at_run = prev_p.runs.len().saturating_sub(1);
    let at = prev_p
        .runs
        .last()
        .map(|r| byte_to_utf16(&r.text, r.text.len()))
        .unwrap_or(0);
    let mut runs = prev_p.runs;
    runs.extend(cur_p.runs);
    cur_p.runs = runs;
    blocks.insert(caret.para - 1, Block::Paragraph(cur_p));
    Ok(Caret {
        para: caret.para - 1,
        run: at_run,
        offset: at,
        ..caret
    })
}

fn apply_delete(blocks: &mut Vec<Block>, caret: Caret) -> Result<Caret, String> {
    {
        let p = paragraph_at(blocks, caret.para)?;
        if let Some(run) = p.runs.get(caret.run) {
            let len = run.text.encode_utf16().count();
            if caret.offset < len {
                let run = &mut p.runs[caret.run];
                let start = utf16_to_byte(&run.text, caret.offset)?;
                let end = start
                    + run.text[start..]
                        .chars()
                        .next()
                        .map(|c| c.len_utf8())
                        .unwrap_or(0);
                run.text.replace_range(start..end, "");
                return Ok(caret);
            }
        }
    }
    {
        // end of the run: join the NEXT run into this one
        let p = paragraph_at(blocks, caret.para)?;
        if caret.run + 1 < p.runs.len() {
            let next = p.runs.remove(caret.run + 1);
            let run = &mut p.runs[caret.run];
            run.text.push_str(&next.text);
            run.images.extend(next.images);
            return Ok(caret);
        }
    }
    // end of the paragraph: join the NEXT paragraph's runs in (this
    // paragraph's pPr survives — same mark-deletion semantics as backspace)
    if matches!(blocks.get(caret.para), Some(Block::Paragraph(_)))
        && matches!(blocks.get(caret.para + 1), Some(Block::Paragraph(_)))
    {
        let next = blocks.remove(caret.para + 1);
        if let (Block::Paragraph(p), Block::Paragraph(next_p)) = (&mut blocks[caret.para], next) {
            p.runs.extend(next_p.runs);
        }
    }
    Ok(caret) // end of the document — no-op
}

fn apply_enter(blocks: &mut Vec<Block>, caret: Caret) -> Result<Caret, String> {
    let p = paragraph_at(blocks, caret.para)?;
    let mut tail: Vec<Run> = Vec::new();
    if !p.runs.is_empty() {
        // split the caret's run; images follow the run's right half — the
        // parser flattens intra-run children and images always render after
        // the text, so they belong to the tail (DD L01 §3.1 ceiling)
        let run = p.runs.get_mut(caret.run).ok_or_else(|| {
            format!(
                "nano light: no run {} in paragraph {}",
                caret.run, caret.para
            )
        })?;
        let byte = utf16_to_byte(&run.text, caret.offset)?;
        let right_text = run.text[byte..].to_string();
        run.text.truncate(byte);
        let mut right = run.clone();
        right.text = right_text;
        right.images = std::mem::take(&mut run.images);
        tail.push(right);
        tail.extend(p.runs.split_off(caret.run + 1));
    }
    let new_para = Paragraph {
        style: p.style.clone(),
        align: p.align.clone(),
        numbering: p.numbering.clone(),
        ind: p.ind.clone(),
        runs: tail,
    };
    blocks.insert(caret.para + 1, Block::Paragraph(new_para));
    Ok(Caret {
        para: caret.para + 1,
        run: 0,
        offset: 0,
        ..caret
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanooffice_ooxml::{ImageRef, Table, TableCell};

    fn document(body: Vec<Block>) -> Document {
        Document {
            body,
            headers: Vec::new(),
            footers: Vec::new(),
            styles: Default::default(),
            numbering: Default::default(),
            doc_defaults: None,
            page: None,
        }
    }

    fn doc_one_para() -> Document {
        document(vec![Block::Paragraph(Paragraph {
            runs: vec![
                Run {
                    text: "hello ".into(),
                    ..Default::default()
                },
                Run {
                    text: "world".into(),
                    bold: Some(true),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })])
    }

    fn session(doc: Document) -> EditorSession {
        EditorSession {
            saved: Some(doc.clone()),
            doc,
            media_src: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            caret: Caret {
                para: 0,
                run: 0,
                offset: 0,
                cell: None,
                cell_para: None,
            },
            sel: None,
            path: std::env::temp_dir().join("nanooffice-test-nl-unused.docx"),
        }
    }

    /// The full round-1 verb set over the model: insert → backspace (incl.
    /// the run-boundary join) → enter (split) → undo → redo — with caret
    /// positions asserted at every step.
    #[test]
    fn edit_verbs_and_carets() {
        let mut s = session(doc_one_para());
        let c0 = Caret {
            para: 0,
            run: 1,
            offset: 2,
            cell: None,
            cell_para: None,
        }; // "wo|orld"
        let c = apply_insert(&mut s.doc.body, c0, "XY").unwrap();
        assert_eq!(
            (c.para, c.run, c.offset),
            (0, 1, 4),
            "caret moves past the inserted text"
        );
        assert_eq!(
            s.doc.body[0],
            Block::Paragraph(Paragraph {
                runs: vec![
                    Run {
                        text: "hello ".into(),
                        ..Default::default()
                    },
                    Run {
                        text: "woXYrld".into(),
                        bold: Some(true),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            })
        );

        let c = apply_backspace(&mut s.doc.body, c).unwrap(); // drop "Y"
        assert_eq!((c.para, c.run, c.offset), (0, 1, 3));
        let c = apply_backspace(&mut s.doc.body, c).unwrap(); // drop "X" → "world"
        assert_eq!((c.para, c.run, c.offset), (0, 1, 2));

        // backspace at the run start joins into the previous run
        let c = apply_backspace(
            &mut s.doc.body,
            Caret {
                para: 0,
                run: 1,
                offset: 0,
                cell: None,
                cell_para: None,
            },
        )
        .unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 0, 6));
        assert_eq!(s.doc.body.len(), 1);

        // enter splits the paragraph at the caret
        let c = apply_enter(&mut s.doc.body, c).unwrap();
        assert_eq!((c.para, c.run, c.offset), (1, 0, 0));
        assert_eq!(s.doc.body.len(), 2);

        // delete at the end of the last run of the last paragraph joins the
        // nothing → no-op; then merge the two paragraphs back via delete
        let before = s.doc.clone();
        let c = apply_delete(
            &mut s.doc.body,
            Caret {
                para: 1,
                run: 0,
                offset: 5,
                cell: None,
                cell_para: None,
            },
        )
        .unwrap();
        assert_eq!(s.doc, before, "delete at document end is a no-op");
        apply_delete(
            &mut s.doc.body,
            Caret {
                para: 0,
                run: 0,
                offset: 6,
                cell: None,
                cell_para: None,
            },
        )
        .unwrap();
        assert_eq!(s.doc.body.len(), 1, "forward delete merges the paragraphs");
    }

    /// utf-16 vs char indexing: an emoji is one char / two utf-16 units —
    /// offsets from JS are units, carets returned must be units too.
    #[test]
    fn utf16_offsets_through_astral_chars() {
        let mut s = session(document(vec![Block::Paragraph(Paragraph {
            runs: vec![Run {
                text: "a\u{1f600}b".into(), // a, 😀, b — 4 utf-16 units
                ..Default::default()
            }],
            ..Default::default()
        })]));
        // insert between the emoji and 'b' (offset 3 = after the surrogate pair)
        let c = apply_insert(
            &mut s.doc.body,
            Caret {
                para: 0,
                run: 0,
                offset: 3,
                cell: None,
                cell_para: None,
            },
            "Q",
        )
        .unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 0, 4));
        assert_eq!(
            match &s.doc.body[0] {
                Block::Paragraph(p) => p.runs[0].text.clone(),
                _ => unreachable!(),
            },
            "a\u{1f600}Qb"
        );
        // backspace over the emoji removes BOTH units at once
        let c = apply_backspace(
            &mut s.doc.body,
            Caret {
                para: 0,
                run: 0,
                offset: 3,
                cell: None,
                cell_para: None,
            },
        )
        .unwrap();
        assert_eq!(c.offset, 1, "one surrogate pair = one backspace");
        assert_eq!(
            match &s.doc.body[0] {
                Block::Paragraph(p) => p.runs[0].text.clone(),
                _ => unreachable!(),
            },
            "aQb"
        );
    }

    /// Editing a table block is refused loudly; insert rejects control chars.
    #[test]
    fn table_and_control_chars_refused() {
        let mut s = session(document(vec![Block::Table(Default::default())]));
        let err = apply_insert(
            &mut s.doc.body,
            Caret {
                para: 0,
                run: 0,
                offset: 0,
                cell: None,
                cell_para: None,
            },
            "x",
        )
        .unwrap_err();
        assert!(err.contains("table"), "refusal names the table: {err}");
        let mut s2 = session(doc_one_para());
        assert!(apply_insert(
            &mut s2.doc.body,
            Caret {
                para: 0,
                run: 0,
                offset: 0,
                cell: None,
                cell_para: None,
            },
            "a\tb"
        )
        .is_err());
    }

    // ── T-A30 §5.5: single-cell editing — identity, verbs, selection ──────

    fn doc_with_table() -> Document {
        let cell = |text: &str| {
            Block::Paragraph(Paragraph {
                runs: vec![Run {
                    text: text.into(),
                    ..Default::default()
                }],
                ..Default::default()
            })
        };
        document(vec![
            cell("before"),
            Block::Table(Table {
                rows: vec![vec![
                    TableCell {
                        blocks: vec![cell("Alpha cell")],
                    },
                    TableCell {
                        blocks: vec![cell("Beta cell")],
                    },
                ]],
                ..Default::default()
            }),
            cell("after"),
        ])
    }

    /// A caret inside table cell [0, col] (the fixture's table is block 1).
    fn cc(col: usize, cell_para: usize, run: usize, offset: usize) -> Caret {
        Caret {
            para: 1,
            run,
            offset,
            cell: Some([0, col]),
            cell_para: Some(cell_para),
        }
    }

    fn cell_texts(doc: &Document, col: usize) -> Vec<String> {
        match &doc.body[1] {
            Block::Table(t) => t.rows[0][col]
                .blocks
                .iter()
                .map(|b| match b {
                    Block::Paragraph(p) => p.runs.iter().map(|r| r.text.clone()).collect(),
                    _ => unreachable!("fixture cells hold paragraphs only"),
                })
                .collect(),
            _ => unreachable!(),
        }
    }

    /// Stale / unaddressable cell identities refuse LOUDLY before any model
    /// change: cell without cellPara (nested-table content), row/col or
    /// cellPara out of bounds, cell identity on a paragraph block, and a
    /// selection mixing cells with top level or another cell.
    #[test]
    fn stale_and_mixed_cell_identities_refused() {
        let mut s = session(doc_with_table());
        let cases: Vec<(EditCmd, &str)> = vec![
            (
                EditCmd::Insert {
                    text: "x".into(),
                    caret: Caret {
                        para: 1,
                        run: 0,
                        offset: 0,
                        cell: Some([0, 0]),
                        cell_para: None,
                    },
                    sel: None,
                },
                "cellPara",
            ),
            (
                EditCmd::Insert {
                    text: "x".into(),
                    caret: cc(9, 0, 0, 0),
                    sel: None,
                },
                "no cell [0,9]",
            ),
            (
                EditCmd::Insert {
                    text: "x".into(),
                    caret: cc(0, 5, 0, 0),
                    sel: None,
                },
                "no block 5",
            ),
            (
                EditCmd::Insert {
                    text: "x".into(),
                    caret: Caret {
                        para: 0,
                        run: 0,
                        offset: 0,
                        cell: Some([0, 0]),
                        cell_para: Some(0),
                    },
                    sel: None,
                },
                "carries cell identity",
            ),
        ];
        for (cmd, what) in cases {
            let err = s.apply(cmd).unwrap_err();
            assert!(err.contains(what), "{what}: refusal must name it: {err:?}");
        }
        let top = Caret {
            para: 0,
            run: 0,
            offset: 1,
            cell: None,
            cell_para: None,
        };
        let mixed = vec![
            (top, cc(0, 0, 0, 1), "cell↔top"),
            (cc(0, 0, 0, 1), cc(1, 0, 0, 1), "cell↔cell"),
        ];
        for (a, b, what) in mixed {
            let err = s
                .apply(EditCmd::Insert {
                    text: "x".into(),
                    caret: b,
                    sel: Some(SelRange {
                        anchor: a,
                        focus: b,
                    }),
                })
                .unwrap_err();
            assert!(
                err.contains("one cell or entirely top level"),
                "{what}: {err:?}"
            );
        }
        assert!(!s.dirty(), "no refused command touched the model");
        assert!(s.undo.is_empty(), "no refused command pushed a snapshot");
    }

    /// The opened path: insert / enter (paragraph MỚI trong cell) / backspace
    /// (merge về paragraph trước — caret tại điểm nối) all address THROUGH
    /// the cell identity, keep the other cell intact, and round-trip the
    /// whole document through the real crate on save.
    #[test]
    fn cell_verbs_edit_within_one_cell() {
        let mut s = session(doc_with_table());
        let c = s
            .apply(EditCmd::Insert {
                text: "T30".into(),
                caret: cc(0, 0, 0, 10),
                sel: None,
            })
            .unwrap();
        assert_eq!(
            (c.para, c.run, c.offset, c.cell, c.cell_para),
            (1, 0, 13, Some([0, 0]), Some(0)),
            "caret keeps the cell identity and rides past the insert"
        );
        assert_eq!(cell_texts(&s.doc, 0), vec!["Alpha cellT30".to_string()]);
        assert_eq!(
            cell_texts(&s.doc, 1),
            vec!["Beta cell".to_string()],
            "cell B untouched"
        );

        // enter inside the cell appends a paragraph WITHIN the cell
        let c = s
            .apply(EditCmd::Enter {
                caret: c,
                sel: None,
            })
            .unwrap();
        assert_eq!(
            (c.para, c.cell, c.cell_para),
            (1, Some([0, 0]), Some(1)),
            "enter moves to the second paragraph of the SAME cell"
        );
        assert_eq!(cell_texts(&s.doc, 0).len(), 2);
        let c = s
            .apply(EditCmd::Insert {
                text: "T30-2".into(),
                caret: c,
                sel: None,
            })
            .unwrap();
        assert_eq!(cell_texts(&s.doc, 0)[1], "T30-2");
        assert_eq!(s.doc.body.len(), 3, "the table still holds one body block");

        // backspace at the second paragraph's start merges WITHIN the cell —
        // caret lands at the junction (end of the first paragraph's content)
        let c = s
            .apply(EditCmd::Backspace {
                caret: cc(0, 1, 0, 0),
                sel: None,
            })
            .unwrap();
        assert_eq!(
            (c.para, c.cell, c.cell_para, c.run, c.offset),
            (1, Some([0, 0]), Some(0), 0, 13),
            "junction caret = end of the first cell paragraph"
        );
        assert_eq!(
            cell_texts(&s.doc, 0),
            vec!["Alpha cellT30T30-2".to_string()]
        );
        assert_eq!(cell_texts(&s.doc, 0).len(), 1);

        // undo twice walks back through the cell mutations (identity rides
        // the snapshots)
        let c = s.apply(EditCmd::Undo).unwrap();
        assert_eq!(
            (c.cell, c.cell_para),
            (Some([0, 0]), Some(1)),
            "undo restores the in-cell caret"
        );
        let c = s.apply(EditCmd::Undo).unwrap();
        assert_eq!(
            (c.cell, c.cell_para),
            (Some([0, 0]), Some(1)),
            "second undo = the insert T30-2 acted at the post-enter caret"
        );

        // save through the REAL crate and parse back: content + structure
        let dir = std::env::temp_dir().join("nanooffice-test-nl-cell");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cell.docx");
        let bytes = nanooffice_ooxml::write_docx(&s.doc, None).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        s.media_src = bytes;
        s.path = path.clone();
        save_session(&mut s).unwrap();
        let back = nanooffice_ooxml::parse_docx_file(&path).unwrap();
        // the undos left cell A at "after enter, before insert T30-2": the
        // second (empty) paragraph survives the real round-trip
        assert_eq!(
            cell_texts(&back, 0),
            vec!["Alpha cellT30".to_string(), String::new()]
        );
        assert_eq!(cell_texts(&back, 1), vec!["Beta cell".to_string()]);
        assert!(
            matches!(&back.body[2], Block::Paragraph(p) if p.runs[0].text == "after"),
            "the block after the table survives the cell edits"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A selection whose endpoints share ONE cell (even cross-paragraph) is
    /// deleted inside that cell by the ranged verbs; a same-cell word replace
    /// keeps the surrounding text.
    #[test]
    fn cell_selection_same_cell_edits() {
        let mut s = session(doc_with_table());
        // replace "Beta" with "T30B" inside cell B: (0,0,0)..(0,0,4)
        let c = s
            .apply(EditCmd::Insert {
                text: "T30B".into(),
                caret: cc(1, 0, 0, 0),
                sel: Some(SelRange {
                    anchor: cc(1, 0, 0, 0),
                    focus: cc(1, 0, 0, 4),
                }),
            })
            .unwrap();
        assert_eq!(
            (c.cell, c.cell_para, c.offset),
            (Some([0, 1]), Some(0), 4),
            "caret = range start + len(text), identity kept"
        );
        assert_eq!(cell_texts(&s.doc, 1), vec!["T30B cell".to_string()]);
        assert_eq!(cell_texts(&s.doc, 0), vec!["Alpha cell".to_string()]);
    }

    /// Session semantics through the REAL transition (`apply` — the command
    /// layer is a thin wrap over it): dirty tracks doc != saved (undo to the
    /// save point clears it), undo/redo restore BOTH model and caret, a
    /// mutation clears redo, and no-op commands do not pollute the undo stack.
    #[test]
    fn session_undo_dirty_semantics() {
        let mut s = session(doc_one_para());
        assert!(!s.dirty(), "fresh session is clean");

        let c0 = s.caret;
        let c = s
            .apply(EditCmd::Insert {
                text: "Z".into(),
                caret: c0,
                sel: None,
            })
            .unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 0, 1));
        assert!(s.dirty());
        assert_eq!(s.undo.len(), 1, "the mutation pushed its snapshot");
        assert!(s.redo.is_empty());

        // undo → back to the saved model, caret of that moment restored
        let c = s.apply(EditCmd::Undo).unwrap();
        assert_eq!(c, c0, "undo restores the caret of the snapshot");
        assert!(!s.dirty(), "undo to the save point is clean");
        assert_eq!(s.redo.len(), 1);

        // redo → dirty again, caret forward again
        let c = s.apply(EditCmd::Redo).unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 0, 1));
        assert!(s.dirty());

        // a no-op backspace at the document start must not push a snapshot
        let depth = s.undo.len();
        let c = s
            .apply(EditCmd::Backspace {
                caret: Caret {
                    para: 0,
                    run: 0,
                    offset: 0,
                    cell: None,
                    cell_para: None,
                },
                sel: None,
            })
            .unwrap();
        assert_eq!(s.undo.len(), depth, "no-op did not pollute the undo stack");
        assert_eq!(
            c,
            Caret {
                para: 0,
                run: 0,
                offset: 0,
                cell: None,
                cell_para: None,
            }
        );
    }

    /// Save round-trip through the real crate: write to a temp file, parse
    /// it back — the marker text survives and the file is a valid package.
    #[test]
    fn save_roundtrips_through_the_crate() {
        let dir = std::env::temp_dir().join("nanooffice-test-nl-save");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("nl.docx");
        // write a minimal real docx via the crate itself (writer is proven;
        // this test only needs a parseable package on disk)
        let doc = doc_one_para();
        let bytes = nanooffice_ooxml::write_docx(&doc, None).unwrap();
        std::fs::write(&path, &bytes).unwrap();

        let mut s = EditorSession {
            saved: Some(doc.clone()),
            doc,
            media_src: bytes,
            undo: Vec::new(),
            redo: Vec::new(),
            caret: Caret {
                para: 0,
                run: 0,
                offset: 0,
                cell: None,
                cell_para: None,
            },
            sel: None,
            path: path.clone(),
        };
        let c = apply_insert(&mut s.doc.body, s.caret, "MARKER-NL-01 ").unwrap();
        s.caret = c;
        save_session(&mut s).unwrap();
        assert!(!s.dirty(), "save clears dirty");

        let back = nanooffice_ooxml::parse_docx_file(&path).unwrap();
        let text: String = match &back.body[0] {
            Block::Paragraph(p) => p.runs.iter().map(|r| r.text.clone()).collect(),
            _ => unreachable!(),
        };
        assert!(text.contains("MARKER-NL-01"), "saved file carries the edit");
        // the tmp sibling is gone (rename, not a copy)
        assert!(!path.with_extension("nanooffice-nl.docx").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ── T-A28 §5.3c: range-aware mutations ────────────────────────────────

    fn img(rel: &str) -> ImageRef {
        ImageRef {
            rel_id: rel.into(),
            part: format!("word/media/{rel}.png"),
            cx_emu: 95250,
            cy_emu: 47625,
        }
    }

    fn run_text(doc: &Document, para: usize) -> String {
        match &doc.body[para] {
            Block::Paragraph(p) => p.runs.iter().map(|r| r.text.clone()).collect(),
            _ => unreachable!("test docs are paragraphs"),
        }
    }

    /// Same-paragraph range deletion: per-run formatting survives, only the
    /// covered text dies; the image rule (a run's images sit at its text END)
    /// drops the head's covered images and keeps the tail's when the cut
    /// stops before its text end.
    #[test]
    fn range_delete_same_para() {
        let mut s = session(document(vec![Block::Paragraph(Paragraph {
            runs: vec![
                Run {
                    text: "hello ".into(),
                    images: vec![img("head")],
                    ..Default::default()
                },
                Run {
                    text: "world".into(),
                    bold: Some(true),
                    images: vec![img("tail")],
                    ..Default::default()
                },
            ],
            ..Default::default()
        })]));
        // delete (0,0,2)..(0,1,3) = "llo wor" → "he" + "ld"
        let c = apply_delete_range(
            &mut s.doc.body,
            Caret {
                para: 0,
                run: 0,
                offset: 2,
                cell: None,
                cell_para: None,
            },
            Caret {
                para: 0,
                run: 1,
                offset: 3,
                cell: None,
                cell_para: None,
            },
        )
        .unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 0, 2), "caret = range start");
        let p = match &s.doc.body[0] {
            Block::Paragraph(p) => p,
            _ => unreachable!(),
        };
        assert_eq!(
            p.runs.len(),
            2,
            "middle-less range keeps both boundary runs"
        );
        assert_eq!(p.runs[0].text, "he");
        assert!(
            p.runs[0].images.is_empty(),
            "head images die with its covered text end"
        );
        assert_eq!(p.runs[1].text, "ld");
        assert_eq!(p.runs[1].bold, Some(true), "tail keeps its own formatting");
        assert_eq!(
            p.runs[1].images,
            vec![img("tail")],
            "tail cut stops before its text end — images live"
        );

        // same-run cut [1..4) of "ld"... (fresh doc): "world" → "wd", images live
        let mut s2 = session(doc_one_para());
        let c = apply_delete_range(
            &mut s2.doc.body,
            Caret {
                para: 0,
                run: 1,
                offset: 1,
                cell: None,
                cell_para: None,
            },
            Caret {
                para: 0,
                run: 1,
                offset: 4,
                cell: None,
                cell_para: None,
            },
        )
        .unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 1, 1));
        assert_eq!(run_text(&s2.doc, 0), "hello wd");
    }

    /// Cross-paragraph range deletion: the FIRST paragraph's properties win
    /// the merge, the tail's runs (with their own run formatting) join it,
    /// middle blocks vanish — and a table anywhere in the span refuses the
    /// whole command with the model untouched.
    #[test]
    fn range_delete_cross_para() {
        let para_with = |text: &str, style: &str, images: Vec<ImageRef>| {
            Block::Paragraph(Paragraph {
                style: Some(style.into()),
                runs: vec![Run {
                    text: text.into(),
                    images,
                    ..Default::default()
                }],
                ..Default::default()
            })
        };
        let mut s = session(document(vec![
            para_with("AB", "P0", vec![img("h")]),
            para_with("CD", "P1", Vec::new()),
            para_with("EF", "P2", vec![img("t")]),
        ]));
        // delete (0,0,1)..(2,0,1) → "A" + "F" merged into para 0 (style P0)
        let c = apply_delete_range(
            &mut s.doc.body,
            Caret {
                para: 0,
                run: 0,
                offset: 1,
                cell: None,
                cell_para: None,
            },
            Caret {
                para: 2,
                run: 0,
                offset: 1,
                cell: None,
                cell_para: None,
            },
        )
        .unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 0, 1));
        assert_eq!(s.doc.body.len(), 1, "middle + tail blocks vanished");
        match &s.doc.body[0] {
            Block::Paragraph(p) => {
                assert_eq!(
                    p.style.as_deref(),
                    Some("P0"),
                    "the FIRST paragraph's pPr wins"
                );
                assert_eq!(p.runs.len(), 2, "head prefix + tail suffix");
                assert_eq!(p.runs[0].text, "A");
                assert!(
                    p.runs[0].images.is_empty(),
                    "head images die (range runs past)"
                );
                assert_eq!(p.runs[1].text, "F");
                assert_eq!(
                    p.runs[1].images,
                    vec![img("t")],
                    "tail images live (cut before text end)"
                );
            }
            _ => unreachable!(),
        }

        // a table anywhere in the span refuses the WHOLE range (data loss guard)
        let mut s2 = session(document(vec![
            para_with("AB", "P0", Vec::new()),
            Block::Table(Default::default()),
            para_with("EF", "P2", Vec::new()),
        ]));
        let before = s2.doc.clone();
        let err = apply_delete_range(
            &mut s2.doc.body,
            Caret {
                para: 0,
                run: 0,
                offset: 1,
                cell: None,
                cell_para: None,
            },
            Caret {
                para: 2,
                run: 0,
                offset: 1,
                cell: None,
                cell_para: None,
            },
        )
        .unwrap_err();
        assert!(err.contains("table"), "refusal names the table: {err}");
        assert_eq!(s2.doc, before, "refused range left the model untouched");
    }

    /// The mutating verbs through `apply` with a selection: insert replaces
    /// the range, backspace/delete delete the WHOLE range (not one char),
    /// enter deletes the range then splits — every returned caret is the
    /// range start (advanced by the verb's own action).
    #[test]
    fn verbs_replace_selection() {
        let mut s = session(doc_one_para()); // "hello " + "world"(bold)
        let sel = |so: usize, fo: usize| {
            Some(SelRange {
                anchor: Caret {
                    para: 0,
                    run: 1,
                    offset: so,
                    cell: None,
                    cell_para: None,
                },
                focus: Caret {
                    para: 0,
                    run: 1,
                    offset: fo,
                    cell: None,
                    cell_para: None,
                },
            })
        };
        // unordered anchor/focus (anchor AFTER focus) — ordering must handle it
        let c = s
            .apply(EditCmd::Insert {
                text: "X".into(),
                caret: Caret {
                    para: 0,
                    run: 1,
                    offset: 1,
                    cell: None,
                    cell_para: None,
                },
                sel: sel(4, 1), // covers "orl"
            })
            .unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 1, 2), "start + len(X)");
        assert_eq!(run_text(&s.doc, 0), "hello wXd");

        // backspace with a range deletes the range, not one char
        let c = s
            .apply(EditCmd::Backspace {
                caret: c,
                sel: sel(2, 1), // "X"
            })
            .unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 1, 1));
        assert_eq!(run_text(&s.doc, 0), "hello wd");

        // delete with a range behaves identically forward
        let c = s
            .apply(EditCmd::Delete {
                caret: c,
                sel: sel(0, 1), // "w"
            })
            .unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 1, 0));
        assert_eq!(run_text(&s.doc, 0), "hello d");

        // enter with a range: range gone, paragraph split at its start
        let c = s
            .apply(EditCmd::Enter {
                caret: Caret {
                    para: 0,
                    run: 0,
                    offset: 3,
                    cell: None,
                    cell_para: None,
                },
                sel: Some(SelRange {
                    anchor: Caret {
                        para: 0,
                        run: 0,
                        offset: 1,
                        cell: None,
                        cell_para: None,
                    },
                    focus: Caret {
                        para: 0,
                        run: 1,
                        offset: 1,
                        cell: None,
                        cell_para: None,
                    },
                }), // "ello " + "d"
            })
            .unwrap();
        assert_eq!((c.para, c.run, c.offset), (1, 0, 0));
        assert_eq!(run_text(&s.doc, 0), "h");
        assert_eq!(run_text(&s.doc, 1), "");
        assert_eq!(s.doc.body.len(), 2);
    }

    /// Undo/redo restore BOTH model and the selection+caret of the moment —
    /// and the snapshot records the CMD's caret/sel, not the session's stale
    /// ones (TS-side caret movement between commands never syncs back —
    /// DD L01 §5.3a/g).
    #[test]
    fn undo_restores_selection_and_cmd_state() {
        let mut s = session(doc_one_para());
        let anchor = Caret {
            para: 0,
            run: 1,
            offset: 4,
            cell: None,
            cell_para: None,
        };
        let focus = Caret {
            para: 0,
            run: 1,
            offset: 1,
            cell: None,
            cell_para: None,
        };
        let sel = Some(SelRange { anchor, focus });
        let c = s
            .apply(EditCmd::Insert {
                text: "XY".into(),
                caret: focus,
                sel,
            })
            .unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 1, 3));
        assert!(s.sel.is_none(), "a mutation collapses the selection");

        let c = s.apply(EditCmd::Undo).unwrap();
        assert_eq!(c, focus, "undo restores the caret the command acted at");
        assert_eq!(
            s.sel, sel,
            "undo restores the selection (highlight comes back)"
        );
        assert!(!s.dirty(), "undo to the save point is clean");

        s.apply(EditCmd::Redo).unwrap();
        assert!(
            s.sel.is_none(),
            "redo returns to the collapsed post-mutation state"
        );

        // a mutation whose cmd caret differs from the session caret snapshots
        // the CMD's state — the next undo jumps to the mutate position
        let c = s
            .apply(EditCmd::Insert {
                text: "Q".into(),
                caret: Caret {
                    para: 0,
                    run: 0,
                    offset: 0,
                    cell: None,
                    cell_para: None,
                },
                sel: None,
            })
            .unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 0, 1));
        assert_eq!(s.caret, c, "session caret follows the outcome");
        let c = s.apply(EditCmd::Undo).unwrap();
        assert_eq!(
            c,
            Caret {
                para: 0,
                run: 0,
                offset: 0,
                cell: None,
                cell_para: None,
            },
            "undo returns to the cmd's caret, not the stale session one"
        );
    }
}
