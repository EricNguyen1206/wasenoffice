//! NanoOffice OOXML core — Track A rung A2 (T-A20), spec-first docx read.
//!
//! Clean-room per `harness/rules/clean-room.md`: the only inputs are the open
//! spec (`spec-ref: ECMA-376 Part 1 WordprocessingML §17, Part 2 OPC`) and
//! black-box observation. No upstream code is read or translated.
//!
//! Slice scope (T-A20, design `docs/design/converter-a2/DD.md`): paragraphs +
//! runs + text + bold/italic + paragraph style, read-only. Writer, tables,
//! lists, images = slices S1+ (see DD §4). The smallest thing that proves the
//! spec-first pipeline; grows with the corpus, not with speculation.

mod document;
mod opc;

use serde::Serialize;

/// A parsed document — the minimal model of slice T-A20.
#[derive(Debug, Serialize)]
pub struct Document {
    pub paragraphs: Vec<Paragraph>,
}

/// One `w:p` (ECMA-376 §17.3.1.22): its style (`w:pStyle` `w:val`) and runs.
/// Paragraphs inside table cells surface here as ordinary paragraphs until
/// the table slice models `w:tbl` (DD §4 S1).
#[derive(Debug, Serialize)]
pub struct Paragraph {
    pub style: Option<String>,
    pub runs: Vec<Run>,
}

/// One `w:r` (§17.3.2.25) with the text of its `w:t` (§17.3.3.30) and the two
/// OnOff properties the slice models (§17.3.2.1 `w:b`, §17.3.2.13 `w:i`).
#[derive(Debug, Serialize)]
pub struct Run {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
}

/// Parse failure — malformed zip, missing part, or malformed XML. Never a
/// panic: parser input is a trust boundary (user files).
#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ParseError {}

/// Parse a docx from its zip bytes (OPC package, ECMA-376 Part 2).
pub fn parse_docx(zip_bytes: &[u8]) -> Result<Document, ParseError> {
    let xml = opc::part_bytes(zip_bytes, "word/document.xml")?;
    let xml =
        std::str::from_utf8(&xml).map_err(|e| ParseError(format!("utf-8 document.xml: {e}")))?;
    document::parse_document_xml(xml)
}

/// Parse a docx from a file path.
pub fn parse_docx_file(path: impl AsRef<std::path::Path>) -> Result<Document, ParseError> {
    let bytes = std::fs::read(path).map_err(|e| ParseError(format!("read file: {e}")))?;
    parse_docx(&bytes)
}
