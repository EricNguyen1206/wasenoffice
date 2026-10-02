//! NanoOffice OOXML core — Track A rung A2 (T-A21, slice S1), spec-first docx read.
//!
//! Clean-room per `harness/rules/clean-room.md`: the only inputs are the open
//! spec (`spec-ref: ECMA-376 Part 1 WordprocessingML §17 + DrawingML §20,
//! Part 2 OPC`) and black-box observation. No upstream code is read or
//! translated.
//!
//! Slice scope (T-A21, design `docs/design/converter-a2/DD.md` §4 S1): full
//! basic-text read — OPC (Content_Types + relationships), body blocks
//! (paragraph + table), paragraph props (pStyle/jc/numPr), run props
//! (b/i/u/sz), numbering resolve, image relationships (r:embed → part, no
//! image decoding), styles.xml basedOn resolve, headers/footers. Write is S2,
//! the adapter is S4 — out of scope here.

mod document;
mod opc;
mod styles;

use std::collections::BTreeMap;

use serde::Serialize;

/// A parsed document — the S1 read model.
#[derive(Debug, Serialize)]
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
}

/// One block-level element of the body/cell content stream.
#[derive(Debug, Serialize)]
pub enum Block {
    Paragraph(Paragraph),
    Table(Table),
}

/// One `w:p` (§17.3.1.22): its direct paragraph properties and its runs.
#[derive(Debug, Default, Serialize)]
pub struct Paragraph {
    /// `w:pStyle w:val` (§17.3.1.27) — the referenced paragraph style.
    pub style: Option<String>,
    /// `w:jc w:val` (§17.3.1.13) — paragraph alignment ("left", "center",
    /// "right", "both", …).
    pub align: Option<String>,
    /// `w:numPr` (§17.3.1.19) — the numbering definition this paragraph
    /// references (raw values; resolved formats live in `Document.numbering`).
    pub numbering: Option<NumRef>,
    pub runs: Vec<Run>,
}

/// A `w:numPr` reference: `w:numId w:val` (§17.9.19) + `w:ilvl w:val`
/// (§17.9.3).
#[derive(Debug, Serialize)]
pub struct NumRef {
    pub num_id: u32,
    pub ilvl: u32,
}

/// One `w:r` (§17.3.2.25). Text props are the direct `w:rPr` (§17.3.2.27)
/// values only — style-chain resolution lives on `StyleDef`.
#[derive(Debug, Default, Serialize)]
pub struct Run {
    /// Concatenated `w:t` content (§17.3.3.31); empty when the run carries
    /// non-text content (an image, a break).
    pub text: String,
    /// `w:b` (§17.3.2.1) with OnOff semantics (§17.3.3 val true/1/on, or
    /// absent = on; false/0/off = off).
    pub bold: bool,
    /// `w:i` (§17.3.2.16), same OnOff semantics.
    pub italic: bool,
    /// `w:u w:val` (§17.3.2.40) — underline style; spec default is "single".
    pub underline: Option<String>,
    /// `w:sz w:val` (§17.3.2.38) — font size in half-points.
    pub size: Option<u32>,
    /// `w:drawing` (§17.3.3.9) images, resolved to relationship targets.
    pub images: Vec<ImageRef>,
}

/// An embedded image reference: `a:blip r:embed` resolved through the part's
/// relationships to its OPC part name, with the `wp:extent` (§20.4.2.7)
/// size in EMU. S1 reads relationships only — image bytes are not touched.
#[derive(Debug, Serialize)]
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

/// One `w:tbl` (§17.4.38) — rows of cells. Row/cell properties (tblPr,
/// tblGrid, tcPr) are not modelled in S1.
#[derive(Debug, Default, Serialize)]
pub struct Table {
    /// `w:tr` rows (§17.4.79) in order; each row's `w:tc` cells (§17.4.66)
    /// in order.
    pub rows: Vec<Vec<TableCell>>,
}

/// One `w:tc` (§17.4.66) — a cell holds block-level content (paragraphs and
/// nested tables, per the CT_Tc content model).
#[derive(Debug, Default, Serialize)]
pub struct TableCell {
    pub blocks: Vec<Block>,
}

/// A header or footer part: its OPC part name plus its block content. The
/// part's own relationships are used for its content (e.g. header images).
#[derive(Debug, Serialize)]
pub struct HeaderFooter {
    /// OPC part name (e.g. `word/header1.xml`).
    pub part: String,
    pub blocks: Vec<Block>,
}

/// A style after basedOn-chain resolution (§17.7.4.17 `w:style`,
/// §17.7.4.3 `w:basedOn`, §17.7.4.9 `w:name`). `None` = no value declared
/// anywhere on the chain; a declared value always wins over the parent's.
#[derive(Debug, Default, Serialize)]
pub struct StyleDef {
    pub name: Option<String>,
    /// From the style's `w:pPr/w:jc` (§17.7.8.2-style pPr, jc §17.3.1.13).
    pub align: Option<String>,
    /// From the style's `w:rPr/w:b` (§17.7.9.1-style rPr, b §17.3.2.1).
    pub bold: Option<bool>,
    /// From the style's `w:rPr/w:i` (i §17.3.2.16).
    pub italic: Option<bool>,
    /// From the style's `w:rPr/w:u` (u §17.3.2.40).
    pub underline: Option<String>,
    /// From the style's `w:rPr/w:sz` (sz §17.3.2.38), half-points.
    pub size: Option<u32>,
}

/// One resolved numbering level: `w:numFmt w:val` (§17.9.18) and
/// `w:lvlText w:val` (§17.9.12) — the format string with `%1..%9` placeholders.
#[derive(Debug, Clone, Serialize)]
pub struct NumLvl {
    pub num_fmt: String,
    pub lvl_text: String,
}

/// Parse failure — malformed zip, missing part, malformed XML, or an OPC
/// violation. Never a panic: parser input is a trust boundary (user files).
#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

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
    let styles = match find_target(&rels, opc::RT_STYLES, &dir)? {
        Some(part) => styles::parse_styles_xml(&utf8(opc::part_bytes(zip_bytes, &part)?, &part)?)?,
        None => BTreeMap::new(),
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

    let body = document::parse_body(
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
