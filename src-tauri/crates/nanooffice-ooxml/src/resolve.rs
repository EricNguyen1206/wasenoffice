//! Style resolution → the render model the viewer consumes (DD V01 §3.2/§3.4).
//!
//! [`resolve`] walks the raw model once and turns every run/paragraph into
//! its FINAL render values, applying the §17.7.2 property hierarchy:
//! direct formatting wins over the `w:rStyle` character style, which wins
//! over the paragraph's style chain (`w:pStyle`, or the style marked
//! `w:default` when the paragraph names none), which sits on top of
//! `w:docDefaults` (§17.7.5.1). The frontend receives this model and merges
//! nothing.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::{
    Block, DocDefaults, Document, ImageRef, Ind, NumRef, Paragraph, Run, Spacing, StyleDef,
    StyleKind, Table,
};

/// A document with every value resolved to its final render form.
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct RenderDoc {
    pub body: Vec<RBlock>,
}

/// Resolved block-level content.
#[derive(Debug, PartialEq, Serialize, Clone)]
pub enum RBlock {
    Paragraph(RParagraph),
    Table(RTable),
}

/// Resolved `w:tbl`: the row/cell structure with resolved content.
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct RTable {
    pub rows: Vec<Vec<RCell>>,
}

/// Resolved `w:tc`.
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct RCell {
    pub blocks: Vec<RBlock>,
}

/// Resolved `w:p`: alignment, indent and runs carry their final values
/// (paragraph → its style chain → docDefaults).
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct RParagraph {
    pub align: Option<String>,
    pub ind: Option<Ind>,
    /// `w:spacing` (§17.3.1.33) as written in the direct pPr (T-V6; direct
    /// only — style-level spacing is a ceiling).
    pub spacing: Option<Spacing>,
    /// Rendered list mark ("1. " / "• " — T-V6): derived from `w:numPr` +
    /// `Document.numbering` with per-`numId` sequential counters; NOT model
    /// text (the mark is not editable in Nano Light — ceiling DD V01 §5.8).
    pub mark: Option<String>,
    pub runs: Vec<RRun>,
}

/// Resolved `w:r` — the render-ready run of DD §3.4: text (with `\t`/`\n`
/// break controls, see `Run::text`), font family, size, bold/italic as final
/// booleans, underline/color/spacing. `None` means "nothing anywhere in the
/// chain declared it" — the renderer applies its own terminal fallback
/// (e.g. 11 pt when even `w:docDefaults` has no size).
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct RRun {
    pub text: String,
    /// Font family for the canvas: the first declared of `w:ascii` →
    /// `w:hAnsi` → `w:eastAsia` → `w:cs` (§17.3.2.26) through the chain.
    pub font: Option<String>,
    /// Half-points, or `None` when nothing declared a size.
    pub size: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    /// `Some("none")` from the chain collapses to `None` — nothing to draw.
    pub underline: Option<String>,
    /// Hex RGB as written; "auto" (§17.3.2.6) collapses to `None` — the
    /// renderer's default ink.
    pub color: Option<String>,
    /// Character spacing in twips (§17.3.2.33).
    pub spacing: Option<i32>,
    pub images: Vec<ImageRef>,
}

/// Resolve a whole document (see module doc). `w:default` character styles
/// are intentionally ignored — the default character style is "Default
/// Paragraph Font", i.e. no formatting of its own.
pub fn resolve(doc: &Document) -> RenderDoc {
    let defaults = doc.doc_defaults.clone().unwrap_or_default();
    let default_para = doc
        .styles
        .values()
        .find(|s| s.kind == StyleKind::Paragraph && s.default);
    let mut counters: BTreeMap<u32, [u32; 9]> = BTreeMap::new();
    RenderDoc {
        body: doc
            .body
            .iter()
            .map(|b| rblock(b, doc, &defaults, default_para, &mut counters))
            .collect(),
    }
}

fn rblock(
    block: &Block,
    doc: &Document,
    defaults: &DocDefaults,
    default_para: Option<&StyleDef>,
    counters: &mut BTreeMap<u32, [u32; 9]>,
) -> RBlock {
    match block {
        Block::Paragraph(p) => {
            RBlock::Paragraph(rparagraph(p, doc, defaults, default_para, counters))
        }
        Block::Table(t) => RBlock::Table(rtable(t, doc, defaults, default_para, counters)),
    }
}

fn rtable(
    t: &Table,
    doc: &Document,
    defaults: &DocDefaults,
    default_para: Option<&StyleDef>,
    counters: &mut BTreeMap<u32, [u32; 9]>,
) -> RTable {
    RTable {
        rows: t
            .rows
            .iter()
            .map(|row| {
                row.iter()
                    .map(|cell| RCell {
                        blocks: cell
                            .blocks
                            .iter()
                            .map(|b| rblock(b, doc, defaults, default_para, counters))
                            .collect(),
                    })
                    .collect()
            })
            .collect(),
    }
}

fn rparagraph(
    p: &Paragraph,
    doc: &Document,
    defaults: &DocDefaults,
    default_para: Option<&StyleDef>,
    counters: &mut BTreeMap<u32, [u32; 9]>,
) -> RParagraph {
    // pStyle names a PARAGRAPH style; an unknown reference degrades to "no
    // style layer" rather than an error (a quick view must not refuse a
    // document over a dangling style name).
    let para_style = p
        .style
        .as_ref()
        .and_then(|id| style_of(doc, id, StyleKind::Paragraph))
        .or(default_para);
    RParagraph {
        align: p
            .align
            .clone()
            .or(para_style.as_ref().and_then(|s| s.align.clone()))
            .or(defaults.align.clone()),
        ind: p
            .ind
            .clone()
            .or(para_style.as_ref().and_then(|s| s.ind.clone()))
            .or(defaults.ind.clone()),
        // direct pPr only (T-V6 — style-level spacing is a ceiling)
        spacing: p.spacing.clone(),
        mark: p
            .numbering
            .as_ref()
            .and_then(|n| numbering_mark(n, doc, counters)),
        runs: p
            .runs
            .iter()
            .map(|r| rrun(r, doc, defaults, para_style))
            .collect(),
    }
}

fn rrun(r: &Run, doc: &Document, defaults: &DocDefaults, para_style: Option<&StyleDef>) -> RRun {
    // rStyle names a CHARACTER style (§17.5.2.3); unknown/degraded → skip the
    // layer (same policy as pStyle above).
    let char_style = r
        .rstyle
        .as_ref()
        .and_then(|id| style_of(doc, id, StyleKind::Character));
    // §17.7.2 hierarchy: direct → rStyle → paragraph style → docDefaults.
    let chain = [char_style.as_ref(), para_style.as_ref()];
    let rfonts = r
        .rfonts
        .clone()
        .or(chain.iter().flatten().find_map(|s| s.rfonts.clone()))
        .or_else(|| defaults.rfonts.clone());
    RRun {
        text: r.text.clone(),
        font: rfonts.and_then(|f| f.ascii.or(f.hansi).or(f.eastasia).or(f.cs)),
        size: r
            .size
            .or(chain.iter().flatten().find_map(|s| s.size))
            .or(defaults.size),
        bold: r
            .bold
            .or(chain.iter().flatten().find_map(|s| s.bold))
            .or(defaults.bold)
            .unwrap_or(false),
        italic: r
            .italic
            .or(chain.iter().flatten().find_map(|s| s.italic))
            .or(defaults.italic)
            .unwrap_or(false),
        underline: r
            .underline
            .clone()
            .or(chain.iter().flatten().find_map(|s| s.underline.clone()))
            .or_else(|| defaults.underline.clone())
            .filter(|v| v != "none"),
        color: r
            .color
            .clone()
            .or(chain.iter().flatten().find_map(|s| s.color.clone()))
            .or_else(|| defaults.color.clone())
            .filter(|v| v != "auto"),
        spacing: r
            .spacing
            .or(chain.iter().flatten().find_map(|s| s.spacing))
            .or(defaults.spacing),
        images: r.images.clone(),
    }
}

/// Bijective base-26 letters (1→a … 27→aa) — `lowerLetter`/`upperLetter`
/// (§17.18.59).
fn letters(mut n: u32, upper: bool) -> String {
    let mut out = Vec::new();
    while n > 0 {
        out.push(b'a' + ((n - 1) % 26) as u8);
        n = (n - 1) / 26;
    }
    out.reverse();
    let s = String::from_utf8(out).unwrap_or_default();
    if upper {
        s.to_uppercase()
    } else {
        s
    }
}

/// Classic roman numerals (1→i … 3999) — `lowerRoman`/`upperRoman` (§17.18.59).
fn roman(mut n: u32, upper: bool) -> String {
    const PAIRS: [(u32, &str); 13] = [
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut out = String::new();
    for (value, sym) in PAIRS {
        while n >= value {
            out.push_str(sym);
            n -= value;
        }
    }
    if upper {
        out.to_uppercase()
    } else {
        out
    }
}

/// Mark text of a numbered paragraph (T-V6): per-`numId` sequential counters
/// (9 levels — deeper levels reset when a shallower one advances);
/// `decimal` formats the `lvlText` `%N` tokens, every other `numFmt` (bullets
/// included) uses the `lvlText` as written, `none` → no mark. `w:start` is
/// not modelled — counters begin at 1 (DD V01 §5.8 ceiling).
fn numbering_mark(
    num: &NumRef,
    doc: &Document,
    counters: &mut BTreeMap<u32, [u32; 9]>,
) -> Option<String> {
    if num.ilvl >= 9 {
        return None;
    }
    let levels = doc.numbering.get(&num.num_id)?;
    let lvl = levels.get(&num.ilvl)?;
    if lvl.num_fmt == "none" {
        return None;
    }
    let row = counters.entry(num.num_id).or_default();
    row[num.ilvl as usize] += 1;
    row[(num.ilvl as usize + 1)..9].fill(0);
    // `%N` token → the level-N counter, rendered per ST_NumberFormat
    // (§17.18.59) — the classic set the corpus exercises (decimal/letters/
    // romans; oracle-obs T-V6: "a)" lowerLetter, "i." lowerRoman); unknown
    // numeric formats fall back to decimal (ceiling). Non-`%` lvlText
    // (bullets) is the symbol as written.
    let token = |idx: usize| -> String {
        let n = row.get(idx).copied().unwrap_or(0);
        match lvl.num_fmt.as_str() {
            "lowerLetter" => letters(n, false),
            "upperLetter" => letters(n, true),
            "lowerRoman" => roman(n, false),
            "upperRoman" => roman(n, true),
            _ => n.to_string(), // decimal + unknown numeric formats
        }
    };
    let mut out = if lvl.lvl_text.contains('%') {
        let mut out = String::new();
        let mut rest = lvl.lvl_text.as_str();
        while let Some(pct) = rest.find('%') {
            out.push_str(&rest[..pct]);
            let digit = rest[pct + 1..].chars().next()?;
            let idx = digit.to_digit(10)? as usize;
            if idx == 0 || idx > 9 {
                return None;
            }
            out.push_str(&token(idx - 1));
            rest = &rest[pct + 2..];
        }
        out.push_str(rest);
        out
    } else {
        lvl.lvl_text.clone()
    };
    // oracle-obs (CDP, T-V6): the engine's GetText renders the mark FOLLOWED
    // BY ONE SPACE ("1. Gạch…" / "• mục") — the separator is not part of
    // lvlText; the viewer matches that text sequence exactly.
    if !out.ends_with(' ') {
        out.push(' ');
    }
    Some(out)
}

/// Style lookup restricted to the kind the referencing slot expects — an
/// rStyle pointing at a paragraph style (or vice versa) is not applied.
fn style_of<'a>(doc: &'a Document, id: &str, kind: StyleKind) -> Option<&'a StyleDef> {
    doc.styles.get(id).filter(|s| s.kind == kind)
}
