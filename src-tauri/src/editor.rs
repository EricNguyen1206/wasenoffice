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
/// the hit's `[row, col]` table identity (null at top level) — in-cell carets
/// are refused on it, not on heuristics; `#[serde(default)]` keeps payloads
/// from before the field additive-parseable.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Caret {
    pub para: usize,
    pub run: usize,
    pub offset: usize,
    #[serde(default)]
    pub cell: Option<[usize; 2]>,
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
    /// range start; the selection collapses on any model change.
    fn apply(&mut self, cmd: EditCmd) -> Result<Caret, String> {
        // Round-1 scope (DD L01 §2/§5.3a): in-cell carets are refused on the
        // hit's `cell` identity (T-060 C3) — the `para` index alone is
        // ambiguous inside tables (cell content shares `pi` with the block
        // after the table). A selection's endpoints ride the same identity.
        let in_cell = |c: Caret| c.cell.is_some();
        if cmd.caret().is_some_and(in_cell)
            || cmd
                .sel()
                .is_some_and(|r| in_cell(r.anchor) || in_cell(r.focus))
        {
            return Err(
                "nano light: editing inside a table cell is out of scope for round 1 (DD L01 §2)"
                    .to_string(),
            );
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
                let (caret, _) = collapse_range(&mut self.doc, caret, sel)?;
                apply_insert(&mut self.doc, caret, &text)
            }
            EditCmd::Backspace { caret, sel } => {
                let (caret, ranged) = collapse_range(&mut self.doc, caret, sel)?;
                if ranged {
                    Ok(caret) // the range delete WAS the deletion
                } else {
                    apply_backspace(&mut self.doc, caret)
                }
            }
            EditCmd::Delete { caret, sel } => {
                let (caret, ranged) = collapse_range(&mut self.doc, caret, sel)?;
                if ranged {
                    Ok(caret)
                } else {
                    apply_delete(&mut self.doc, caret)
                }
            }
            EditCmd::Enter { caret, sel } => {
                let (caret, _) = collapse_range(&mut self.doc, caret, sel)?;
                apply_enter(&mut self.doc, caret)
            }
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
// mutations — free functions over `Document`, hit-test coordinates in/out
// ---------------------------------------------------------------------------

/// Resolve `para` to a top-level paragraph. Round-1 scope (DD L01 §2): only
/// top-level paragraphs are editable — in-cell hits are refused upstream on
/// the caret's `cell` identity (T-060 C3); this arm catches stale carets
/// whose `para` points straight at a table block.
fn paragraph_at(doc: &mut Document, para: usize) -> Result<&mut Paragraph, String> {
    match doc.body.get_mut(para) {
        Some(Block::Paragraph(p)) => Ok(p),
        Some(Block::Table(_)) => Err(format!(
            "nano light: block {para} is a table — editing inside tables is out of scope for round 1 (DD L01 §2)"
        )),
        None => Err(format!("nano light: no block {para}")),
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
    doc: &mut Document,
    caret: Caret,
    sel: Option<SelRange>,
) -> Result<(Caret, bool), String> {
    match sel {
        Some(r) if r.anchor != r.focus => Ok((apply_delete_range(doc, r.anchor, r.focus)?, true)),
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
fn apply_delete_range(doc: &mut Document, a: Caret, b: Caret) -> Result<Caret, String> {
    // document-order key (para, run, offset) — a total order over top-level
    // carets (cell carets are refused upstream, DD L01 §5.3a)
    let (start, end) = if (a.para, a.run, a.offset) <= (b.para, b.run, b.offset) {
        (a, b)
    } else {
        (b, a)
    };
    if start.para >= doc.body.len() || end.para >= doc.body.len() {
        return Err(format!(
            "nano light: range endpoints {}..{} outside the document ({} blocks)",
            start.para,
            end.para,
            doc.body.len()
        ));
    }
    if doc.body[start.para..=end.para]
        .iter()
        .any(|blk| matches!(blk, Block::Table(_)))
    {
        return Err(format!(
            "nano light: range {}..{} spans a table — selection cannot enter tables (round 1, DD L01 §5.3)",
            start.para, end.para
        ));
    }
    if start.para == end.para {
        let p = paragraph_at(doc, start.para)?;
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
        return Ok(Caret {
            para: start.para,
            run: start.run,
            offset: start.offset,
            cell: None,
        });
    }
    // head paragraph: keep its prefix (its pPr wins the merge), drop the runs
    // after the caret run — they are inside the range. An empty head
    // paragraph (selection starting at a `w:p` with no runs) has nothing to
    // truncate; it survives with its pPr and adopts the tail runs.
    {
        let p = paragraph_at(doc, start.para)?;
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
        let p = paragraph_at(doc, end.para)?;
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
    doc.body.drain(start.para + 1..=end.para);
    if let Some(Block::Paragraph(p)) = doc.body.get_mut(start.para) {
        p.runs.extend(tail_runs);
    }
    Ok(Caret {
        para: start.para,
        run: start.run,
        offset: start.offset,
        cell: None,
    })
}

fn apply_insert(doc: &mut Document, caret: Caret, text: &str) -> Result<Caret, String> {
    // Break controls are round-tripped inside run text by the parser, but
    // typed input must not smuggle them in (they would desync the layout's
    // tokenize) — `enter` is the only sanctioned paragraph break.
    if text
        .chars()
        .any(|c| matches!(c, '\t' | '\n' | '\u{c}' | '\u{b}'))
    {
        return Err("nano light: insert rejects control characters (use enter for breaks)".into());
    }
    let p = paragraph_at(doc, caret.para)?;
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

fn apply_backspace(doc: &mut Document, caret: Caret) -> Result<Caret, String> {
    if caret.offset > 0 {
        let p = paragraph_at(doc, caret.para)?;
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
        let p = paragraph_at(doc, caret.para)?;
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
        return Ok(caret); // start of the document — no-op
    }
    if !matches!(doc.body.get(caret.para - 1), Some(Block::Paragraph(_)))
        || !matches!(doc.body.get(caret.para), Some(Block::Paragraph(_)))
    {
        return Ok(caret); // a table sits on the boundary — refuse, no-op
    }
    // Word semantics: deleting a paragraph mark adopts the FOLLOWING
    // paragraph's properties — the caret paragraph's pPr survives (DD §3.1).
    let cur = doc.body.remove(caret.para);
    let prev = doc.body.remove(caret.para - 1);
    let (mut cur_p, prev_p) = match (cur, prev) {
        (Block::Paragraph(c), Block::Paragraph(p)) => (c, p),
        _ => return Ok(caret), // unreachable: both checked above
    };
    let at_run = cur_p.runs.len().saturating_sub(1);
    let at = cur_p
        .runs
        .last()
        .map(|r| byte_to_utf16(&r.text, r.text.len()))
        .unwrap_or(0);
    let mut runs = prev_p.runs;
    runs.extend(cur_p.runs);
    cur_p.runs = runs;
    doc.body.insert(caret.para - 1, Block::Paragraph(cur_p));
    Ok(Caret {
        para: caret.para - 1,
        run: at_run,
        offset: at,
        cell: None,
    })
}

fn apply_delete(doc: &mut Document, caret: Caret) -> Result<Caret, String> {
    {
        let p = paragraph_at(doc, caret.para)?;
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
        let p = paragraph_at(doc, caret.para)?;
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
    if matches!(doc.body.get(caret.para), Some(Block::Paragraph(_)))
        && matches!(doc.body.get(caret.para + 1), Some(Block::Paragraph(_)))
    {
        let next = doc.body.remove(caret.para + 1);
        if let (Block::Paragraph(p), Block::Paragraph(next_p)) = (&mut doc.body[caret.para], next) {
            p.runs.extend(next_p.runs);
        }
    }
    Ok(caret) // end of the document — no-op
}

fn apply_enter(doc: &mut Document, caret: Caret) -> Result<Caret, String> {
    let p = paragraph_at(doc, caret.para)?;
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
    doc.body.insert(caret.para + 1, Block::Paragraph(new_para));
    Ok(Caret {
        para: caret.para + 1,
        run: 0,
        offset: 0,
        cell: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanooffice_ooxml::ImageRef;

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
        }; // "wo|orld"
        let c = apply_insert(&mut s.doc, c0, "XY").unwrap();
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

        let c = apply_backspace(&mut s.doc, c).unwrap(); // drop "Y"
        assert_eq!((c.para, c.run, c.offset), (0, 1, 3));
        let c = apply_backspace(&mut s.doc, c).unwrap(); // drop "X" → "world"
        assert_eq!((c.para, c.run, c.offset), (0, 1, 2));

        // backspace at the run start joins into the previous run
        let c = apply_backspace(
            &mut s.doc,
            Caret {
                para: 0,
                run: 1,
                offset: 0,
                cell: None,
            },
        )
        .unwrap();
        assert_eq!((c.para, c.run, c.offset), (0, 0, 6));
        assert_eq!(s.doc.body.len(), 1);

        // enter splits the paragraph at the caret
        let c = apply_enter(&mut s.doc, c).unwrap();
        assert_eq!((c.para, c.run, c.offset), (1, 0, 0));
        assert_eq!(s.doc.body.len(), 2);

        // delete at the end of the last run of the last paragraph joins the
        // nothing → no-op; then merge the two paragraphs back via delete
        let before = s.doc.clone();
        let c = apply_delete(
            &mut s.doc,
            Caret {
                para: 1,
                run: 0,
                offset: 5,
                cell: None,
            },
        )
        .unwrap();
        assert_eq!(s.doc, before, "delete at document end is a no-op");
        apply_delete(
            &mut s.doc,
            Caret {
                para: 0,
                run: 0,
                offset: 6,
                cell: None,
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
            &mut s.doc,
            Caret {
                para: 0,
                run: 0,
                offset: 3,
                cell: None,
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
            &mut s.doc,
            Caret {
                para: 0,
                run: 0,
                offset: 3,
                cell: None,
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
            &mut s.doc,
            Caret {
                para: 0,
                run: 0,
                offset: 0,
                cell: None,
            },
            "x",
        )
        .unwrap_err();
        assert!(err.contains("table"), "refusal names the table: {err}");
        let mut s2 = session(doc_one_para());
        assert!(apply_insert(
            &mut s2.doc,
            Caret {
                para: 0,
                run: 0,
                offset: 0,
                cell: None,
            },
            "a\tb"
        )
        .is_err());
    }

    /// T-060 C3: a caret whose hit landed INSIDE a table cell (`cell`
    /// identity from the layout's hitTest) is refused by every mutating verb
    /// BEFORE `para` is resolved — `para` alone is ambiguous there (cell
    /// content shares `pi` with the block after the table), which is exactly
    /// why the identity rides on the caret.
    #[test]
    fn in_cell_carets_refused() {
        let mut s = session(doc_one_para());
        let in_cell = Caret {
            para: 0,
            run: 0,
            offset: 1,
            cell: Some([0, 1]),
        };
        let top_caret = Caret {
            para: 0,
            run: 0,
            offset: 0,
            cell: None,
        };
        for cmd in [
            EditCmd::Insert {
                text: "x".into(),
                caret: in_cell,
                sel: None,
            },
            EditCmd::Backspace {
                caret: in_cell,
                sel: None,
            },
            EditCmd::Delete {
                caret: in_cell,
                sel: None,
            },
            EditCmd::Enter {
                caret: in_cell,
                sel: None,
            },
            // a selection endpoint inside a cell refuses the same way
            EditCmd::Insert {
                text: "x".into(),
                caret: top_caret,
                sel: Some(SelRange {
                    anchor: top_caret,
                    focus: in_cell,
                }),
            },
        ] {
            let err = s.apply(cmd).unwrap_err();
            assert!(
                err.contains("table cell"),
                "in-cell {err:?} must refuse on the cell identity"
            );
        }
        assert!(!s.dirty(), "no refused command touched the model");
        assert!(s.undo.is_empty(), "no refused command pushed a snapshot");
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
            },
            sel: None,
            path: path.clone(),
        };
        let c = apply_insert(&mut s.doc, s.caret, "MARKER-NL-01 ").unwrap();
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
            &mut s.doc,
            Caret {
                para: 0,
                run: 0,
                offset: 2,
                cell: None,
            },
            Caret {
                para: 0,
                run: 1,
                offset: 3,
                cell: None,
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
            &mut s2.doc,
            Caret {
                para: 0,
                run: 1,
                offset: 1,
                cell: None,
            },
            Caret {
                para: 0,
                run: 1,
                offset: 4,
                cell: None,
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
            &mut s.doc,
            Caret {
                para: 0,
                run: 0,
                offset: 1,
                cell: None,
            },
            Caret {
                para: 2,
                run: 0,
                offset: 1,
                cell: None,
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
            &mut s2.doc,
            Caret {
                para: 0,
                run: 0,
                offset: 1,
                cell: None,
            },
            Caret {
                para: 2,
                run: 0,
                offset: 1,
                cell: None,
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
                },
                focus: Caret {
                    para: 0,
                    run: 1,
                    offset: fo,
                    cell: None,
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
                },
                sel: Some(SelRange {
                    anchor: Caret {
                        para: 0,
                        run: 0,
                        offset: 1,
                        cell: None,
                    },
                    focus: Caret {
                        para: 0,
                        run: 1,
                        offset: 1,
                        cell: None,
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
        };
        let focus = Caret {
            para: 0,
            run: 1,
            offset: 1,
            cell: None,
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
            },
            "undo returns to the cmd's caret, not the stale session one"
        );
    }
}
