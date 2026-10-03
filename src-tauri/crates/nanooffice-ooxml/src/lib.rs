//! NanoOffice OOXML core — Track A rung A2 (T-A21, slice S1), spec-first docx read.
//!
//! Clean-room per `harness/rules/clean-room.md`: the only inputs are the open
//! spec (`spec-ref: ECMA-376 Part 1 WordprocessingML §17 + DrawingML §20,
//! Part 2 OPC`) and black-box observation. No upstream code is read or
//! translated.
//!
//! Slice scope (T-A21 design §4 S1, T-A22 S2): full basic-text read (OPC,
//! body blocks paragraph + table, pStyle/jc/numPr, b/i/u/sz, numbering
//! resolve, image relationships, styles basedOn resolve, headers/footers)
//! and the matching write side — `write::write_docx` serializes the model
//! back to an OPC package, symmetrical to the parser, one function per
//! construct. The S4 adapter is out of scope here.

mod document;
mod opc;
pub mod resolve;
mod styles;
pub mod write;

pub use opc::try_part_bytes;
pub use resolve::{resolve, RBlock, RCell, RParagraph, RRun, RTable, RenderDoc};
pub use write::{write_docx, write_docx_file};

use std::collections::BTreeMap;

use serde::Serialize;

/// A parsed document — the S1 read model.
#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct Document {
    /// Block-level content of `w:body` (§17.2.2), in document order.
    pub body: Vec<Block>,
    /// Header parts (`w:hdr`, §17.10.4) referenced from the document
    /// relationships, in relationship-file order.
    pub headers: Vec<HeaderFooter>,
    /// Footer parts (`w:ftr`, §17.10.3), same rule as `headers`.
    pub footers: Vec<HeaderFooter>,
    /// Styles resolved through their `w:basedOn` chain (§17.7.4.3 — the
    /// child style inherits the parent's properties, child values win),
    /// keyed by styleId. Only paragraph + character styles surface here;
    /// table/numbering styles are not modelled in S1.
    pub styles: BTreeMap<String, StyleDef>,
    /// Numbering resolved per numId → ilvl (§17.9: `w:num` §17.9.16 maps to
    /// `w:abstractNum` §17.9.1 whose `w:lvl` §17.9.7 carries the format).
    pub numbering: BTreeMap<u32, BTreeMap<u32, NumLvl>>,
    /// `w:docDefaults` (§17.7.5.1) — the document-wide property defaults that
    /// sit at the BOTTOM of every style chain (§17.7.2); `None` = not declared.
    pub doc_defaults: Option<DocDefaults>,
    /// `w:pgSz` (§17.6.11) of the final section (`w:sectPr` §17.6.17 at body
    /// level); `None` = not declared. Mid-document sections (inside `w:pPr`)
    /// are out of the model.
    pub page: Option<Page>,
}

/// `w:pgSz` (§17.6.11) — page size in twips plus the orientation as written.
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct Page {
    /// `w:w` — page width, twips.
    pub w: Option<u32>,
    /// `w:h` — page height, twips.
    pub h: Option<u32>,
    /// `w:orient` ("landscape"/"portrait").
    pub orient: Option<String>,
}

/// `w:docDefaults` (§17.7.5.1): `w:rPrDefault` (§17.7.5.3) run defaults plus
/// `w:pPrDefault` (§17.7.5.2) paragraph defaults — the property subset the
/// model carries (same fields as a style's rPr/pPr).
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct DocDefaults {
    pub rfonts: Option<RFonts>,
    pub color: Option<String>,
    pub spacing: Option<i32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<String>,
    pub size: Option<u32>,
    /// From `w:pPrDefault/w:pPr/w:jc` (§17.3.1.13).
    pub align: Option<String>,
    /// From `w:pPrDefault/w:pPr/w:ind` (§17.3.1.12).
    pub ind: Option<Ind>,
}

/// One block-level element of the body/cell content stream.
#[derive(Debug, PartialEq, Serialize, Clone)]
pub enum Block {
    Paragraph(Paragraph),
    Table(Table),
}

/// One `w:p` (§17.3.1.22): its direct paragraph properties and its runs.
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct Paragraph {
    /// `w:pStyle w:val` (§17.3.1.27) — the referenced paragraph style.
    pub style: Option<String>,
    /// `w:jc w:val` (§17.3.1.13) — paragraph alignment ("left", "center",
    /// "right", "both", …).
    pub align: Option<String>,
    /// `w:numPr` (§17.3.1.19) — the numbering definition this paragraph
    /// references (raw values; resolved formats live in `Document.numbering`).
    pub numbering: Option<NumRef>,
    /// `w:ind` (§17.3.1.12) — direct paragraph indentation; style-chain
    /// resolution happens in [`crate::resolve`].
    pub ind: Option<Ind>,
    pub runs: Vec<Run>,
}

/// A `w:numPr` reference: `w:numId w:val` (§17.9.19) + `w:ilvl w:val`
/// (§17.9.3).
#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct NumRef {
    pub num_id: u32,
    pub ilvl: u32,
}

/// One `w:r` (§17.3.2.25). Text props are the direct `w:rPr` (§17.3.2.27)
/// values only — style-chain resolution lives on `StyleDef` and the render
/// resolver ([`crate::resolve`]).
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct Run {
    /// Text content: concatenated `w:t` (§17.3.3.31) plus the break controls —
    /// `w:tab` (§17.3.3.34) serializes as `\t`, `w:br` (§17.3.3.3) as `\n`
    /// (text), `\u{c}` (page) or `\u{b}` (column); those three control chars
    /// are illegal in XML content, so no `w:t` can collide with them.
    pub text: String,
    /// `w:b` (§17.3.2.1) three-state (§17.3.3 ST_OnOff): `None` = not set
    /// (inherit the style chain), `Some(true)`/`Some(false)` = set on/off —
    /// including the explicit `<w:b w:val="false"/>` override.
    pub bold: Option<bool>,
    /// `w:i` (§17.3.2.16), same three-state semantics.
    pub italic: Option<bool>,
    /// `w:u w:val` (§17.3.2.40) — underline style; spec default is "single".
    pub underline: Option<String>,
    /// `w:sz w:val` (§17.3.2.38) — font size in half-points.
    pub size: Option<u32>,
    /// `w:rStyle w:val` (§17.5.2.3) — the character style referenced by this
    /// run; resolved through `Document.styles` (§17.7.2 hierarchy: direct rPr
    /// wins over rStyle, rStyle wins over the paragraph style).
    pub rstyle: Option<String>,
    /// `w:rFonts` (§17.3.2.26) — font families per script.
    pub rfonts: Option<RFonts>,
    /// `w:color w:val` (§17.3.2.6) — hex RGB or "auto", as written.
    pub color: Option<String>,
    /// `w:spacing w:val` (§17.3.2.33) — character spacing in twips, signed
    /// (expanded > 0, condensed < 0).
    pub spacing: Option<i32>,
    /// `w:drawing` (§17.3.3.9) images, resolved to relationship targets.
    pub images: Vec<ImageRef>,
}

/// `w:rFonts` (§17.3.2.26) family attributes. Theme references (`asciiTheme`…)
/// are not modelled — only literal families (the corpus and the viewer's
/// bundled-font mapping are literal-name based).
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct RFonts {
    /// `w:ascii` — characters 0x00–0x7F.
    pub ascii: Option<String>,
    /// `w:hAnsi` — "high ANSI" (everything else the other slots don't cover).
    pub hansi: Option<String>,
    /// `w:eastAsia` — East Asian script.
    pub eastasia: Option<String>,
    /// `w:cs` — complex scripts.
    pub cs: Option<String>,
}

/// `w:ind` (§17.3.1.12) paragraph indentation, twips. `first_line` and
/// `hanging` are mutually exclusive by spec (hanging wins when both appear);
/// both are kept as written so the round-trip stays lossless.
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct Ind {
    /// `w:left` — left indent (signed: negative outdents).
    pub left: Option<i64>,
    /// `w:right` — right indent.
    pub right: Option<i64>,
    /// `w:firstLine` — first-line extra indent.
    pub first_line: Option<i64>,
    /// `w:hanging` — first-line OUTdent (removes from `left`).
    pub hanging: Option<i64>,
}

/// An embedded image reference: `a:blip r:embed` resolved through the part's
/// relationships to its OPC part name, with the `wp:extent` (§20.4.2.7)
/// size in EMU. S1 reads relationships only — image bytes are not touched.
#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct ImageRef {
    /// The `r:embed` relationship id.
    pub rel_id: String,
    /// Resolved OPC part name (e.g. `word/media/image1.png`).
    pub part: String,
    /// `wp:extent cx` (§20.4.2.8 inline wrapper), EMU.
    pub cx_emu: i64,
    /// `wp:extent cy`, EMU.
    pub cy_emu: i64,
}

/// One `w:tbl` (§17.4.38) — rows of cells plus the column grid. Cell
/// properties (tblPr, tcPr) are not modelled in S1.
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct Table {
    /// `w:tblGrid` (§17.4.4) column widths, `w:gridCol w:w` (§17.4.17) in
    /// twips, in order; `None` = the grid column declares no width (auto).
    pub grid: Vec<Option<u32>>,
    /// `w:tr` rows (§17.4.79) in order; each row's `w:tc` cells (§17.4.66)
    /// in order.
    pub rows: Vec<Vec<TableCell>>,
}

/// One `w:tc` (§17.4.66) — a cell holds block-level content (paragraphs and
/// nested tables, per the CT_Tc content model).
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct TableCell {
    pub blocks: Vec<Block>,
}

/// A header or footer part: its OPC part name plus its block content. The
/// part's own relationships are used for its content (e.g. header images).
#[derive(Debug, PartialEq, Serialize, Clone)]
pub struct HeaderFooter {
    /// OPC part name (e.g. `word/header1.xml`).
    pub part: String,
    pub blocks: Vec<Block>,
}

/// A style after basedOn-chain resolution (§17.7.4.17 `w:style`,
/// §17.7.4.3 `w:basedOn`, §17.7.4.9 `w:name`). `None` = no value declared
/// anywhere on the chain; a declared value always wins over the parent's.
#[derive(Debug, Default, PartialEq, Serialize, Clone)]
pub struct StyleDef {
    /// WordprocessingML style type (§17.7.4.17 @w:type — a required
    /// attribute, so the writer cannot emit styles.xml without it). S1 only
    /// surfaces paragraph + character styles.
    pub kind: StyleKind,
    /// `w:default` (§17.7.2.4) — this style is the default of its type: the
    /// default PARAGRAPH style backs every paragraph without a `w:pStyle`.
    pub default: bool,
    pub name: Option<String>,
    /// From the style's `w:pPr/w:jc` (§17.7.8.2-style pPr, jc §17.3.1.13).
    pub align: Option<String>,
    /// From the style's `w:pPr/w:ind` (§17.3.1.12).
    pub ind: Option<Ind>,
    /// From the style's `w:rPr/w:b` (§17.7.9.1-style rPr, b §17.3.2.1).
    pub bold: Option<bool>,
    /// From the style's `w:rPr/w:i` (i §17.3.2.16).
    pub italic: Option<bool>,
    /// From the style's `w:rPr/w:u` (u §17.3.2.40).
    pub underline: Option<String>,
    /// From the style's `w:rPr/w:sz` (sz §17.3.2.38), half-points.
    pub size: Option<u32>,
    /// From the style's `w:rPr/w:rFonts` (§17.3.2.26).
    pub rfonts: Option<RFonts>,
    /// From the style's `w:rPr/w:color` (§17.3.2.6).
    pub color: Option<String>,
    /// From the style's `w:rPr/w:spacing` (§17.3.2.33), twips.
    pub spacing: Option<i32>,
}

/// WordprocessingML style type (§17.7.4.17 @w:type): the two S1-modelled
/// kinds. Paragraph styles are referenced from `w:pPr/w:pStyle`
/// (§17.3.1.27), character styles from `w:rPr/w:rStyle` (§17.5.2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub enum StyleKind {
    #[default]
    Paragraph,
    Character,
}

/// One resolved numbering level: `w:numFmt w:val` (§17.9.18) and
/// `w:lvlText w:val` (§17.9.12) — the format string with `%1..%9` placeholders.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NumLvl {
    pub num_fmt: String,
    pub lvl_text: String,
}

/// Parse failure — malformed zip, missing part, malformed XML, or an OPC
/// violation. Never a panic: parser input is a trust boundary (user files).
#[derive(Debug)]
pub struct ParseError(pub String);

/// Write failure — a model that cannot be serialized into a valid OPC
/// package (duplicate relationship ids pointing at different parts, an image
/// part of an unknown extension). Never a panic.
#[derive(Debug)]
pub struct WriteError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

impl std::fmt::Display for WriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for WriteError {}

pub(crate) fn xml_err(e: quick_xml::Error) -> ParseError {
    ParseError(format!("xml: {e}"))
}

/// Parse a docx from its zip bytes (OPC package, ECMA-376 Part 2).
pub fn parse_docx(zip_bytes: &[u8]) -> Result<Document, ParseError> {
    // Part 2 OPC: Content_Types stream + package-level relationships locate
    // the main document part — its name is data, never hard-coded.
    let ct = opc::ContentTypes::parse(&utf8(
        opc::part_bytes(zip_bytes, "[Content_Types].xml")?,
        "[Content_Types].xml",
    )?)?;
    let root_rels = opc::parse_rels(&utf8(
        opc::part_bytes(zip_bytes, "_rels/.rels")?,
        "_rels/.rels",
    )?)?;
    let doc_rel = root_rels
        .iter()
        .find(|(_, r)| r.rel_type == opc::RT_OFFICE_DOCUMENT)
        .map(|(_, r)| r)
        .ok_or_else(|| ParseError("no officeDocument relationship in _rels/.rels".into()))?;
    let doc_part = opc::resolve_target("", &doc_rel.target)?;
    match ct.get(&doc_part) {
        Some(t) if t == opc::CT_MAIN_DOCUMENT => {}
        Some(t) => {
            return Err(ParseError(format!(
                "part {doc_part} has content type {t:?} — refusing a non-WordprocessingML main document"
            )))
        }
        None => return Err(ParseError(format!("no content type declared for part {doc_part}"))),
    }

    let (dir, file) = doc_part
        .rsplit_once('/')
        .map(|(d, f)| (d.to_string(), f.to_string()))
        .unwrap_or_default();
    let rels = load_rels(zip_bytes, &dir, &file)?;

    // Accessory parts resolve through the document part's relationships
    // (Part 2: part-level relationships at `<dir>/_rels/<file>.rels`).
    let (styles, doc_defaults) = match find_target(&rels, opc::RT_STYLES, &dir)? {
        Some(part) => styles::parse_styles_xml(&utf8(opc::part_bytes(zip_bytes, &part)?, &part)?)?,
        None => (BTreeMap::new(), None),
    };
    let numbering = match find_target(&rels, opc::RT_NUMBERING, &dir)? {
        Some(part) => {
            styles::parse_numbering_xml(&utf8(opc::part_bytes(zip_bytes, &part)?, &part)?)?
        }
        None => BTreeMap::new(),
    };

    // Headers/footers surface in relationship-file order; each part parses
    // against its OWN relationships (a header image lives in header rels).
    let mut headers = Vec::new();
    let mut footers = Vec::new();
    for (id, rel) in &rels {
        let (list, root) = match rel.rel_type.as_str() {
            t if t == opc::RT_HEADER => (&mut headers, b"w:hdr".as_slice()),
            t if t == opc::RT_FOOTER => (&mut footers, b"w:ftr".as_slice()),
            _ => continue,
        };
        let part = opc::resolve_target(&dir, &rel.target)?;
        let part_rels = load_rels(zip_bytes, &part_dir(&part), &file_name(&part))?;
        let xml = utf8(opc::part_bytes(zip_bytes, &part)?, &part)?;
        let blocks = document::parse_container(&xml, &part_rels, &part_dir(&part), root)?;
        list.push(HeaderFooter { part, blocks });
        let _ = id; // relationship id kept out of the model; order is the contract
    }

    let (body, page) = document::parse_body(
        &utf8(opc::part_bytes(zip_bytes, &doc_part)?, &doc_part)?,
        &rels,
        &dir,
    )?;
    Ok(Document {
        body,
        headers,
        footers,
        styles,
        numbering,
        doc_defaults,
        page,
    })
}

/// Parse a docx from a file path.
pub fn parse_docx_file(path: impl AsRef<std::path::Path>) -> Result<Document, ParseError> {
    let bytes = std::fs::read(path).map_err(|e| ParseError(format!("read file: {e}")))?;
    parse_docx(&bytes)
}

/// All relationships of one part, in relationship-file order.
type RelsList = Vec<(String, opc::Rel)>;

fn load_rels(zip_bytes: &[u8], dir: &str, file: &str) -> Result<RelsList, ParseError> {
    let rels_part = if dir.is_empty() {
        format!("_rels/{file}.rels")
    } else {
        format!("{dir}/_rels/{file}.rels")
    };
    match opc::try_part_bytes(zip_bytes, &rels_part) {
        Some(bytes) => opc::parse_rels(&utf8(bytes, &rels_part)?),
        None => Ok(Vec::new()), // a part may legally carry no relationships
    }
}

/// Target of the first relationship of the given type, resolved against the
/// source part's directory.
fn find_target(rels: &RelsList, rel_type: &str, dir: &str) -> Result<Option<String>, ParseError> {
    rels.iter()
        .find(|(_, r)| r.rel_type == rel_type)
        .map(|(_, r)| opc::resolve_target(dir, &r.target))
        .transpose()
}

fn part_dir(part: &str) -> String {
    part.rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default()
}

fn file_name(part: &str) -> String {
    part.rsplit_once('/')
        .map(|(_, f)| f.to_string())
        .unwrap_or_else(|| part.to_string())
}

fn utf8(bytes: Vec<u8>, what: &str) -> Result<String, ParseError> {
    String::from_utf8(bytes).map_err(|e| ParseError(format!("utf-8 {what}: {e}")))
}
