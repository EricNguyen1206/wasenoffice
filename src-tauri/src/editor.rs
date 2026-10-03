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
use tauri::{State, WebviewUrl, WebviewWindowBuilder};

use nanooffice_ooxml::{Block, Document, Page, Paragraph, RenderDoc, Run};

use crate::protocol::FileRegistry;

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

/// Snapshot-clone undo bound (DD L01 §2): the model is small, clones are
/// cheap; the oldest snapshot drops beyond this.
const UNDO_CAP: usize = 100;

/// One Nano Light session, owned by its window label (pattern T-059 — every
/// command resolves its session by the calling window, never "the latest").
/// Holds the UNRESOLVED `Document` (the write-back-able model — `RenderDoc`
/// has flattened the §17.7.2 style chain and cannot round-trip), the original
/// package bytes (media parts are copied byte-exact on save, T-A26),
/// snapshot-clone undo/redo stacks carrying the caret of their moment, and
/// the last-saved snapshot (`dirty = doc != saved` — undoing back to the
/// save point clears it for free).
pub struct EditorSession {
    doc: Document,
    media_src: Vec<u8>,
    undo: Vec<(Document, Caret)>,
    redo: Vec<(Document, Caret)>,
    saved: Option<Document>,
    caret: Caret,
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
    /// pollute the stack); undo/redo manage their own stacks.
    fn apply(&mut self, cmd: EditCmd) -> Result<Caret, String> {
        // Round-1 scope (DD L01 §2): in-cell carets are refused on the hit's
        // `cell` identity (T-060 C3) — the `para` index alone is ambiguous
        // inside tables (cell content shares `pi` with the following block).
        if let Some(c) = cmd.caret() {
            if c.cell.is_some() {
                return Err(
                    "nano light: editing inside a table cell is out of scope for round 1 (DD L01 §2)"
                        .to_string(),
                );
            }
        }
        let is_mutation = !matches!(cmd, EditCmd::Undo | EditCmd::Redo);
        let before = (self.doc.clone(), self.caret);
        let caret = match cmd {
            EditCmd::Insert { text, caret } => apply_insert(&mut self.doc, caret, &text),
            EditCmd::Backspace { caret } => apply_backspace(&mut self.doc, caret),
            EditCmd::Delete { caret } => apply_delete(&mut self.doc, caret),
            EditCmd::Enter { caret } => apply_enter(&mut self.doc, caret),
            EditCmd::Undo => match self.undo.pop() {
                Some((doc, caret)) => {
                    self.redo
                        .push((std::mem::replace(&mut self.doc, doc), self.caret));
                    Ok(caret)
                }
                None => Ok(self.caret), // empty stack — no-op
            },
            EditCmd::Redo => match self.redo.pop() {
                Some((doc, caret)) => {
                    self.undo
                        .push((std::mem::replace(&mut self.doc, doc), self.caret));
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

/// Shared open-a-window body of the two view commands.
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
    WebviewWindowBuilder::new(app, &label, WebviewUrl::App("editor.html".into()))
        .title(format!("NanoOffice — Nano Light: {name}"))
        .inner_size(1000.0, 700.0)
        // Rule 6b (AGENTS.md): show WITHOUT taking focus. No shim is injected
        // on this window — Nano Light has no bridge, no engine (DD L01 §2).
        .visible(true)
        .focused(false)
        .build()
        .map_err(|e| format!("nano light: open window: {e}"))?;
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
    let path = {
        let map = sessions
            .0
            .lock()
            .map_err(|_| "nano light: sessions poisoned".to_string())?;
        map.get(window.label())
            .map(|s| s.path.clone())
            .ok_or_else(|| format!("nano light: no session for window {}", window.label()))?
    };
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
    let started = std::time::Instant::now();
    let bytes = std::fs::read(&path).map_err(|e| format!("nano light: read {path}: {e}"))?;
    let doc = nanooffice_ooxml::parse_docx(&bytes)
        .map_err(|e| format!("nano light: parse {path}: {e}"))?;
    let render = nanooffice_ooxml::resolve(&doc);
    let fonts: BTreeMap<String, [i16; 3]> = crate::viewer::families(&render)
        .into_iter()
        .filter_map(|fam| crate::protocol::allfonts::line_metrics(&fam).map(|m| (fam, m)))
        .collect();
    // Registering hands the media route its fileId (`/media/<id>/<part>`,
    // filesrv reads the registry — not the session). No registration → no
    // media; text still renders (same policy as the viewer).
    let media_base = match registry.register(window.label(), std::path::Path::new(&path)) {
        Ok(id) => Some(format!(
            "http://127.0.0.1:{}/media/{id}",
            crate::protocol::filesrv::bound_port()
        )),
        Err(e) => {
            log::warn!("nano light: media disabled — cannot register file: {e}");
            None
        }
    };
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
    let page = doc.page.clone();
    let session = EditorSession {
        saved: Some(doc.clone()),
        doc,
        media_src: bytes,
        undo: Vec::new(),
        redo: Vec::new(),
        caret,
        path: PathBuf::from(&path),
    };
    sessions
        .0
        .lock()
        .map_err(|_| "nano light: sessions poisoned".to_string())?
        .insert(window.label().to_string(), session);
    let parse_ms = started.elapsed().as_secs_f64() * 1000.0;
    log::info!(
        "nano light: opened {path} in {parse_ms:.1} ms (window {})",
        window.label()
    );
    Ok(NanoLightDoc {
        parse_ms,
        doc: render,
        fonts,
        page,
        media_base,
        caret,
        name,
    })
}

/// One mutation command — `caret` fields only where the command needs them.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum EditCmd {
    Insert { text: String, caret: Caret },
    Backspace { caret: Caret },
    Delete { caret: Caret },
    Enter { caret: Caret },
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
            | EditCmd::Backspace { caret }
            | EditCmd::Delete { caret }
            | EditCmd::Enter { caret } => Some(*caret),
            EditCmd::Undo | EditCmd::Redo => None,
        }
    }
}

/// What every edit returns: the re-resolved render model (the frontend draws
/// from this — it merges nothing), the new caret and dirty.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditOutcome {
    pub doc: RenderDoc,
    pub caret: Caret,
    pub dirty: bool,
}

#[tauri::command]
pub fn nano_light_edit(
    sessions: State<'_, Arc<EditorSessions>>,
    window: tauri::WebviewWindow,
    cmd: EditCmd,
) -> Result<EditOutcome, String> {
    let mut map = sessions
        .0
        .lock()
        .map_err(|_| "nano light: sessions poisoned".to_string())?;
    let session = map
        .get_mut(window.label())
        .ok_or_else(|| format!("nano light: no session for window {}", window.label()))?;
    let label = cmd.label();
    let caret = session.apply(cmd)?;
    let dirty = session.dirty();
    log::info!(
        "nano light edit: cmd={label} caret={}/{}/{} undo={} dirty={dirty}",
        caret.para,
        caret.run,
        caret.offset,
        session.undo.len()
    );
    Ok(EditOutcome {
        doc: nanooffice_ooxml::resolve(&session.doc),
        caret,
        dirty,
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
    let mut map = sessions
        .0
        .lock()
        .map_err(|_| "nano light: sessions poisoned".to_string())?;
    let session = map
        .get_mut(window.label())
        .ok_or_else(|| format!("nano light: no session for window {}", window.label()))?;
    save_session(session)
}

fn save_session(session: &mut EditorSession) -> Result<(), String> {
    let started = std::time::Instant::now();
    let out = nanooffice_ooxml::write_docx(&session.doc, Some(&session.media_src))
        .map_err(|e| format!("nano light: write docx: {e}"))?;
    let ext = session
        .path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("docx")
        .to_string();
    let tmp = session.path.with_extension(format!("nanooffice-nl.{ext}"));
    std::fs::write(&tmp, &out)
        .map_err(|e| format!("nano light: write tmp {}: {e}", tmp.display()))?;
    if let Err(e) = std::fs::rename(&tmp, &session.path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("nano light: rename over original: {e}"));
    }
    session.saved = Some(session.doc.clone());
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    log::info!(
        "nano light saved: {} ({} bytes) in {ms:.1} ms",
        session.path.display(),
        out.len()
    );
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
        for cmd in [
            EditCmd::Insert {
                text: "x".into(),
                caret: in_cell,
            },
            EditCmd::Backspace { caret: in_cell },
            EditCmd::Delete { caret: in_cell },
            EditCmd::Enter { caret: in_cell },
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
}
