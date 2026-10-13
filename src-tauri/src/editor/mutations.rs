//! Mutation semantics of the Nano Light editor (review #6 C8 — split from
//! editor.rs): free functions over a block list (the body, or one cell's
//! blocks — T-A30 §5.5b), hit-test coordinates in/out. No Tauri, no session
//! state, no IO — the test suite is the second consumer of this interface
//! (`Vec<Block>` + `Caret` → `Caret`); the only production consumer is
//! `apply()` in the parent module.

use nanooffice_ooxml::{Block, Document, Paragraph, Run};

use super::{Caret, SelRange};

// ---------------------------------------------------------------------------
// mutations — free functions over a block list (the body, or one cell's
// blocks — T-A30 §5.5b), hit-test coordinates in/out
// ---------------------------------------------------------------------------

/// Both selection endpoints share one edit surface: both top level, or the
/// SAME cell (cross-paragraph within a cell is fine — DD L01 §5.5c).
pub(super) fn same_cell(a: Option<[usize; 2]>, b: Option<[usize; 2]>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// Resolve the block list a CELL caret edits (§17.4.38 → §17.4.79 → §17.4.66
/// through the caret's identity): `body[para]` must be a table, `[row, col]` exist.
pub(super) fn cell_blocks_mut(doc: &mut Document, c: Caret) -> Result<&mut Vec<Block>, String> {
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
pub(super) fn dispatch(
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
pub(super) fn paragraph_at(blocks: &mut [Block], idx: usize) -> Result<&mut Paragraph, String> {
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
pub(super) fn utf16_to_byte(s: &str, units: usize) -> Result<usize, String> {
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
pub(super) fn byte_to_utf16(s: &str, byte: usize) -> usize {
    s.char_indices()
        .take_while(|(b, _)| *b < byte)
        .map(|(_, c)| c.len_utf16())
        .sum()
}

/// A non-empty selection collapses to: delete the range, mutate at the range
/// start. Returns `(caret, ranged)` — `ranged` tells the single-char verbs to
/// skip their own step (the range delete WAS the deletion, DD L01 §5.3c).
pub(super) fn collapse_range(
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
pub(super) fn apply_delete_range(
    blocks: &mut Vec<Block>,
    a: Caret,
    b: Caret,
) -> Result<Caret, String> {
    // document-order key (para, run, offset) — a total order over top-level
    // carets (cell carets are refused upstream, DD L01 §5.3a)
    let (start, end) = ordered_carets(a, b);
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

pub(super) fn apply_insert(
    blocks: &mut [Block],
    caret: Caret,
    text: &str,
) -> Result<Caret, String> {
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

pub(super) fn apply_backspace(blocks: &mut Vec<Block>, caret: Caret) -> Result<Caret, String> {
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

pub(super) fn apply_delete(blocks: &mut Vec<Block>, caret: Caret) -> Result<Caret, String> {
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

pub(super) fn apply_enter(blocks: &mut Vec<Block>, caret: Caret) -> Result<Caret, String> {
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
        spacing: p.spacing.clone(),
        // the section mark stays with the HEAD paragraph — a split must not
        // duplicate the section break (DD L01 §5.6b)
        sect: None,
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

/// Order two carets by document position (para, run, offset).
pub(super) fn ordered_carets(a: Caret, b: Caret) -> (Caret, Caret) {
    if (a.para, a.run, a.offset) <= (b.para, b.run, b.offset) {
        (a, b)
    } else {
        (b, a)
    }
}

pub(super) fn apply_format(
    blocks: &mut [Block],
    caret: Caret,
    sel: SelRange,
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
) -> Result<Caret, String> {
    let (start, end) = ordered_carets(sel.anchor, sel.focus);
    if start == end {
        return Ok(caret);
    }

    for p_idx in start.para..=end.para {
        let p = match paragraph_at(blocks, p_idx) {
            Ok(p) => p,
            Err(_) => continue,
        };

        let mut new_runs = Vec::new();
        for (ri, run) in p.runs.drain(..).enumerate() {
            let r_len = run.text.encode_utf16().count();
            let r_start = if p_idx == start.para && ri == start.run {
                start.offset
            } else if p_idx > start.para || ri > start.run {
                0
            } else {
                r_len
            };
            let r_end = if p_idx == end.para && ri == end.run {
                end.offset
            } else if p_idx < end.para || ri < end.run {
                r_len
            } else {
                0
            };

            if r_start >= r_end || r_start >= r_len || r_end == 0 {
                new_runs.push(run);
                continue;
            }

            let b_start = utf16_to_byte(&run.text, r_start)?;
            let b_end = utf16_to_byte(&run.text, r_end)?;

            if b_start > 0 {
                let mut pre = run.clone();
                pre.text = run.text[..b_start].to_string();
                pre.images.clear();
                new_runs.push(pre);
            }

            let mut mid = run.clone();
            mid.text = run.text[b_start..b_end].to_string();
            if b_end < run.text.len() {
                mid.images.clear();
            }
            if bold.is_some() {
                mid.bold = match mid.bold {
                    Some(true) => Some(false),
                    _ => Some(true),
                };
            }
            if italic.is_some() {
                mid.italic = match mid.italic {
                    Some(true) => Some(false),
                    _ => Some(true),
                };
            }
            if underline.is_some() {
                mid.underline = if mid.underline.is_some() {
                    None
                } else {
                    Some("single".into())
                };
            }
            new_runs.push(mid);

            if b_end < run.text.len() {
                let mut post = run.clone();
                post.text = run.text[b_end..].to_string();
                new_runs.push(post);
            }
        }
        p.runs = new_runs;
    }

    Ok(caret)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_format_toggle_bold_split_run() {
        let mut blocks = vec![Block::Paragraph(Paragraph {
            runs: vec![Run {
                text: "Hello world of formatting".into(),
                ..Default::default()
            }],
            ..Default::default()
        })];

        let caret = Caret {
            para: 0,
            run: 0,
            offset: 6,
            cell: None,
            cell_para: None,
        };
        let sel = SelRange {
            anchor: Caret {
                para: 0,
                run: 0,
                offset: 6,
                cell: None,
                cell_para: None,
            },
            focus: Caret {
                para: 0,
                run: 0,
                offset: 11,
                cell: None,
                cell_para: None,
            },
        };

        let res = apply_format(&mut blocks, caret, sel, Some(true), None, None).unwrap();
        assert_eq!(res.offset, 6);

        if let Block::Paragraph(p) = &blocks[0] {
            assert_eq!(p.runs.len(), 3, "run should be split into 3");
            assert_eq!(p.runs[0].text, "Hello ");
            assert_eq!(p.runs[0].bold, None);

            assert_eq!(p.runs[1].text, "world");
            assert_eq!(p.runs[1].bold, Some(true));

            assert_eq!(p.runs[2].text, " of formatting");
            assert_eq!(p.runs[2].bold, None);
        } else {
            panic!("expected paragraph");
        }

        let sel2 = SelRange {
            anchor: Caret {
                para: 0,
                run: 1,
                offset: 0,
                cell: None,
                cell_para: None,
            },
            focus: Caret {
                para: 0,
                run: 1,
                offset: 5,
                cell: None,
                cell_para: None,
            },
        };
        let _ = apply_format(&mut blocks, caret, sel2, Some(true), None, None).unwrap();
        if let Block::Paragraph(p) = &blocks[0] {
            assert_eq!(p.runs[1].bold, Some(false));
        }
    }
}
