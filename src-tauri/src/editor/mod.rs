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

use nanooffice_ooxml::{Document, Page, PageMar, RenderDoc};

use crate::protocol::FileRegistry;
use crate::viewer::render_payload;

mod mutations;

use mutations::*;

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
        let is_format = matches!(cmd, EditCmd::Format { .. });
        let format_sel = if is_format { cmd.sel() } else { None };
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
            EditCmd::Format {
                bold,
                italic,
                underline,
                caret,
                sel,
            } => dispatch(&mut self.doc, caret, Some(sel), |bl, c, s| {
                apply_format(bl, c, s.unwrap(), bold, italic, underline)
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
            if is_format {
                self.sel = format_sel;
            } else {
                self.sel = None; // a mutation collapses the selection to a caret
            }
        }
        self.caret = caret;
        Ok(caret)
    }
}

/// Window-label → session. The single-slot `QuickViewRequest` race noted in
/// viewer.rs cannot recur here: keys are explicit per window from the start
/// (the single-slot stash pattern ITSELF did recur — see `NanoLightRequest`
/// below; that one is safe by construction: the window's open fires once on
/// page load, viewer.rs:29-31).
#[derive(Default)]
pub struct EditorSessions(Mutex<HashMap<String, EditorSession>>);

impl EditorSessions {
    /// Generate a lookup key for the window and optional tab index.
    pub fn session_key(window_label: &str, tab_id: Option<u32>) -> String {
        match tab_id {
            Some(0) | None => window_label.to_string(),
            Some(id) => format!("{window_label}:{id}"),
        }
    }

    /// Drop all sessions associated with a closed window (including all tabs).
    pub fn remove_owner(&self, owner: &str) -> usize {
        let mut map = self.0.lock().expect("editor sessions poisoned");
        let before = map.len();
        map.retain(|k, _| k != owner && !k.starts_with(&format!("{owner}:")));
        before - map.len()
    }

    /// Run `f` on the target session key.
    pub fn with_session<T>(
        &self,
        key: &str,
        f: impl FnOnce(&mut EditorSession) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut map = self
            .0
            .lock()
            .map_err(|_| "nano light: sessions poisoned".to_string())?;
        let session = map
            .get_mut(key)
            .ok_or_else(|| format!("nano light: no session for {key}"))?;
        f(session)
    }
}

pub(crate) use crate::window::SurfaceHandoff;

/// Window-label sequence for UI-opened Nano Light windows ("main" = env boot).
static WINDOW_SEQ: AtomicU32 = AtomicU32::new(0);

/// Shared open-a-window body of the two view commands (the window build
/// itself lives in `viewer::open_surface_window`, T-060 C2).
fn open_window(
    app: &tauri::AppHandle,
    handoff: &SurfaceHandoff,
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
    handoff.stash(&label, path.clone());
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
    handoff: State<'_, Arc<SurfaceHandoff>>,
    path: String,
) -> Result<(), String> {
    if !std::path::Path::new(&path).is_file() {
        return Err(format!("nano light: no such file: {path}"));
    }
    open_window(&app, handoff.inner(), path)
}

/// Reopen THIS session's document in a new Nano Light window (reads the file
/// from disk — the disk path never crosses into JS).
#[tauri::command]
pub fn nano_light_reopen(
    app: tauri::AppHandle,
    handoff: State<'_, Arc<SurfaceHandoff>>,
    sessions: State<'_, Arc<EditorSessions>>,
    window: tauri::WebviewWindow,
) -> Result<(), String> {
    let path = sessions.with_session(window.label(), |s| Ok(s.path.clone()))?;
    open_window(&app, handoff.inner(), path.display().to_string())
}

/// The Nano Light payload — same shape as the viewer's `QuickView` (the two
/// frontends consume one contract), plus the caret, the file name for the
/// window header and the refuse-warn list (T-A31 — constructs the save will
/// drop, DD L01 §5.6d).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NanoLightDoc {
    pub parse_ms: f64,
    pub doc: RenderDoc,
    pub fonts: BTreeMap<String, [i16; 3]>,
    pub page: Option<Page>,
    /// `w:pgMar` of the final section (T-V6) — text origin + margins.
    pub mar: Option<PageMar>,
    pub media_base: Option<String>,
    pub caret: Caret,
    pub name: String,
    pub warnings: Vec<String>,
}

#[tauri::command]
pub fn nano_light_open(
    registry: State<'_, Arc<FileRegistry>>,
    handoff: State<'_, Arc<SurfaceHandoff>>,
    sessions: State<'_, Arc<EditorSessions>>,
    window: tauri::WebviewWindow,
    tab_id: Option<u32>,
    doc_path: Option<String>,
) -> Result<NanoLightDoc, String> {
    let path = match doc_path {
        Some(p) if !p.is_empty() => p,
        _ => handoff
            .take(window.label())
            .or_else(|| {
                std::env::var("NANO_OFFICE_NANO_LIGHT")
                    .ok()
                    .filter(|p| !p.is_empty())
            })
            .ok_or_else(|| {
                "nano light: no source (UI action or NANO_OFFICE_NANO_LIGHT)".to_string()
            })?,
    };
    let p = render_payload(
        registry.inner().as_ref(),
        window.label(),
        &path,
        "nano light",
    )?;
    // Refuse-warn (OQ-L01-2 closed: open + warn, never refuse — D1 keeps the
    // engine as the advanced mode for everything the model drops on save).
    let warnings = nanooffice_ooxml::detect_save_losses(&p.bytes);
    if !warnings.is_empty() {
        log::warn!("nano light warn: save will drop: {}", warnings.join(", "));
    }
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
        .insert(EditorSessions::session_key(window.label(), tab_id), session);
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
        mar: p.mar,
        media_base: p.media_base,
        caret,
        name,
        warnings,
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
    Format {
        #[serde(default)]
        bold: Option<bool>,
        #[serde(default)]
        italic: Option<bool>,
        #[serde(default)]
        underline: Option<bool>,
        caret: Caret,
        sel: SelRange,
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
            EditCmd::Format { .. } => "format",
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
            | EditCmd::Enter { caret, .. }
            | EditCmd::Format { caret, .. } => Some(*caret),
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
            EditCmd::Format { sel, .. } => Some(*sel),
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
    tab_id: Option<u32>,
) -> Result<EditOutcome, String> {
    let key = EditorSessions::session_key(window.label(), tab_id);
    let label = cmd.label();
    let (caret, dirty, undo_len, sel, render) = sessions.with_session(&key, |s| {
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
    tab_id: Option<u32>,
) -> Result<(), String> {
    let key = EditorSessions::session_key(window.label(), tab_id);
    sessions.with_session(&key, save_session)
}

#[tauri::command]
pub fn nano_light_close_tab(
    sessions: State<'_, Arc<EditorSessions>>,
    window: tauri::WebviewWindow,
    tab_id: u32,
) -> Result<(), String> {
    let key = EditorSessions::session_key(window.label(), Some(tab_id));
    let mut map = sessions
        .0
        .lock()
        .map_err(|_| "nano light: sessions poisoned".to_string())?;
    map.remove(&key);
    Ok(())
}

fn save_session(session: &mut EditorSession) -> Result<(), String> {
    let started = std::time::Instant::now();
    let out = nanooffice_ooxml::write_docx(&session.doc, Some(&session.media_src))
        .map_err(|e| format!("nano light: write docx: {e}"))?;
    crate::storage::atomic_write_bytes(&session.path, &out, "nanooffice-nl")
        .map_err(|e| format!("nano light: {e}"))?;
    session.saved = Some(session.doc.clone());
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    log::info!(
        "nano light saved: {} ({} bytes) in {ms:.1} ms",
        session.path.display(),
        out.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nanooffice_ooxml::{Block, ImageRef, Paragraph, Run, Table, TableCell};

    fn document(body: Vec<Block>) -> Document {
        Document {
            body,
            headers: Vec::new(),
            footers: Vec::new(),
            styles: Default::default(),
            numbering: Default::default(),
            doc_defaults: None,
            page: None,
            page_mar: None,
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

    #[test]
    fn test_editor_sessions_multi_tab_keying_and_cleanup() {
        let sessions = EditorSessions::default();
        let key0 = EditorSessions::session_key("nanolight-1", None);
        let key1 = EditorSessions::session_key("nanolight-1", Some(1));
        let key2 = EditorSessions::session_key("nanolight-1", Some(2));
        assert_eq!(key0, "nanolight-1");
        assert_eq!(key1, "nanolight-1:1");
        assert_eq!(key2, "nanolight-1:2");

        let s0 = session(doc_one_para());
        let s1 = session(doc_one_para());
        sessions.0.lock().unwrap().insert(key0, s0);
        sessions.0.lock().unwrap().insert(key1, s1);

        assert!(sessions
            .with_session("nanolight-1", |s| Ok(s.path.clone()))
            .is_ok());
        assert!(sessions
            .with_session("nanolight-1:1", |s| Ok(s.path.clone()))
            .is_ok());

        let dropped = sessions.remove_owner("nanolight-1");
        assert_eq!(dropped, 2);
        assert!(sessions
            .with_session("nanolight-1", |s| Ok(s.path.clone()))
            .is_err());
        assert!(sessions
            .with_session("nanolight-1:1", |s| Ok(s.path.clone()))
            .is_err());
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

    // ── T-A31 §5.6: hyperlink + section ride the edits ────────────────────

    fn link_run(text: &str) -> Run {
        Run {
            text: text.into(),
            link: Some(nanooffice_ooxml::Hyperlink {
                rel_id: Some("rIdL".into()),
                anchor: None,
                target: Some("https://example.com/nl".into()),
                external: true,
            }),
            ..Default::default()
        }
    }

    /// Typing INSIDE a hyperlink run keeps the group on the grown run; enter
    /// splits the link to both halves and leaves the section mark on the HEAD
    /// paragraph; undo restores the model; the crate round-trip keeps the
    /// link (rel + target) and the section (pgSz + pgMar).
    #[test]
    fn link_and_section_ride_edits_and_roundtrip() {
        let original = document(vec![
            Block::Paragraph(Paragraph {
                runs: vec![
                    Run {
                        text: "see ".into(),
                        ..Default::default()
                    },
                    link_run("the link"),
                ],
                ..Default::default()
            }),
            Block::Paragraph(Paragraph {
                runs: vec![Run {
                    text: "sect para".into(),
                    ..Default::default()
                }],
                sect: Some(Box::new(nanooffice_ooxml::Section {
                    page: Some(nanooffice_ooxml::Page {
                        w: Some(11906),
                        ..Default::default()
                    }),
                    mar: Some(nanooffice_ooxml::PageMar {
                        top: Some(1134),
                        ..Default::default()
                    }),
                })),
                ..Default::default()
            }),
        ]);
        let mut s = session(original.clone());
        let c = s
            .apply(EditCmd::Insert {
                text: "X".into(),
                caret: Caret {
                    para: 0,
                    run: 1,
                    offset: 4,
                    cell: None,
                    cell_para: None,
                },
                sel: None,
            })
            .unwrap();
        assert_eq!(c.offset, 5);
        let p0 = match &s.doc.body[0] {
            Block::Paragraph(p) => p,
            _ => unreachable!(),
        };
        assert_eq!(p0.runs[1].text, "the Xlink");
        assert!(
            p0.runs[1].link.is_some(),
            "typed into the link run — link stays"
        );

        // enter mid-link: both halves carry the link, the section mark does
        // NOT move to the new paragraph
        let c = s
            .apply(EditCmd::Enter {
                caret: Caret {
                    para: 0,
                    run: 1,
                    offset: 5,
                    cell: None,
                    cell_para: None,
                },
                sel: None,
            })
            .unwrap();
        assert_eq!((c.para, c.run, c.offset), (1, 0, 0));
        let head = match &s.doc.body[0] {
            Block::Paragraph(p) => p,
            _ => unreachable!(),
        };
        let tail = match &s.doc.body[1] {
            Block::Paragraph(p) => p,
            _ => unreachable!(),
        };
        assert_eq!(
            head.runs[1].link, tail.runs[0].link,
            "link rides both halves"
        );
        assert!(
            head.sect.is_none(),
            "the split head never carried the section"
        );
        assert!(
            tail.sect.is_none(),
            "a split must not duplicate the section break"
        );
        assert!(matches!(&s.doc.body[2], Block::Paragraph(p) if p.sect.is_some()));

        // undo → the exact pre-edit model
        s.apply(EditCmd::Undo).unwrap();
        s.apply(EditCmd::Undo).unwrap();
        assert_eq!(s.doc, original, "undo restores link + section intact");

        // save through the REAL crate: link (rel + target) + section survive
        let dir = std::env::temp_dir().join("nanooffice-test-nl-fidelity");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fidelity.docx");
        let bytes = nanooffice_ooxml::write_docx(&s.doc, None).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        s.media_src = bytes;
        s.path = path.clone();
        save_session(&mut s).unwrap();
        let back = nanooffice_ooxml::parse_docx_file(&path).unwrap();
        let bp = match &back.body[0] {
            Block::Paragraph(p) => p,
            _ => unreachable!(),
        };
        assert_eq!(
            bp.runs[1].link.as_ref().unwrap().target.as_deref(),
            Some("https://example.com/nl")
        );
        let bs = match &back.body[1] {
            Block::Paragraph(p) => p,
            _ => unreachable!(),
        };
        assert_eq!(
            bs.sect.as_ref().unwrap().mar.as_ref().unwrap().top,
            Some(1134)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

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
