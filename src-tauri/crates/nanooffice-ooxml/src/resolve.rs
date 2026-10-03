//! Style resolution → the render model the viewer consumes (DD V01 §3.2/§3.4).
//!
//! [`resolve`] walks the raw model once and turns every run/paragraph into
//! its FINAL render values, applying the §17.7.2 property hierarchy:
//! direct formatting wins over the `w:rStyle` character style, which wins
//! over the paragraph's style chain (`w:pStyle`, or the style marked
//! `w:default` when the paragraph names none), which sits on top of
//! `w:docDefaults` (§17.7.5.1). The frontend receives this model and merges
//! nothing.

use serde::Serialize;

use crate::{
    Block, DocDefaults, Document, ImageRef, Ind, Paragraph, Run, StyleDef, StyleKind, Table,
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
    RenderDoc {
        body: doc
            .body
            .iter()
            .map(|b| rblock(b, doc, &defaults, default_para))
            .collect(),
    }
}

fn rblock(
    block: &Block,
    doc: &Document,
    defaults: &DocDefaults,
    default_para: Option<&StyleDef>,
) -> RBlock {
    match block {
        Block::Paragraph(p) => RBlock::Paragraph(rparagraph(p, doc, defaults, default_para)),
        Block::Table(t) => RBlock::Table(rtable(t, doc, defaults, default_para)),
    }
}

fn rtable(
    t: &Table,
    doc: &Document,
    defaults: &DocDefaults,
    default_para: Option<&StyleDef>,
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
                            .map(|b| rblock(b, doc, defaults, default_para))
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

/// Style lookup restricted to the kind the referencing slot expects — an
/// rStyle pointing at a paragraph style (or vice versa) is not applied.
fn style_of<'a>(doc: &'a Document, id: &str, kind: StyleKind) -> Option<&'a StyleDef> {
    doc.styles.get(id).filter(|s| s.kind == kind)
}
