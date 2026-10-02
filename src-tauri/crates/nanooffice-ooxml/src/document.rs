//! `word/document.xml` (+ `w:hdr`/`w:ftr` parts) → blocks (ECMA-376 Part 1,
//! WordprocessingML §17).
//!
//! Recursive descent: one parser function per construct, each citing its
//! spec clause. Elements/attributes are matched by their prefixed names and
//! the root must bind the `w:` prefix to the WordprocessingML namespace —
//! anything outside the S1 construct table is skipped (subtree, no state
//! carried) rather than guessed at; documents that bind `w:` wrongly are
//! refused (DD §6 "refuse-unknown instead of parse-wrong").

use quick_xml::events::{BytesStart, Event};

use crate::opc::{self, attr, Rel};
use crate::{xml_err, Block, ImageRef, NumRef, Paragraph, ParseError, Run, Table, TableCell};

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// Relationship table of the part being parsed (for `r:embed` resolution).
type RelsList = Vec<(String, Rel)>;

struct Parser<'a> {
    reader: quick_xml::Reader<&'a [u8]>,
    rels: &'a RelsList,
    base_dir: &'a str,
}

/// Parse the main document part: check the `w:document` root (§17.2.3) and
/// return the `w:body` (§17.2.2) block stream.
pub fn parse_body(xml: &str, rels: &RelsList, base_dir: &str) -> Result<Vec<Block>, ParseError> {
    let mut parser = Parser {
        reader: quick_xml::Reader::from_str(xml),
        rels,
        base_dir,
    };
    let mut buf = Vec::new();
    let mut saw_document = false;
    loop {
        buf.clear();
        match parser.reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => match e.name().as_ref() {
                b"w:document" => {
                    saw_document = true;
                    if attr(&e, b"xmlns:w")?.as_deref() != Some(W_NS) {
                        return Err(ParseError(
                            "w: prefix is not bound to the WordprocessingML namespace — refusing to parse with wrong namespaces".into(),
                        ));
                    }
                }
                b"w:body" if saw_document => return parser.parse_blocks(None),
                _ => {}
            },
            // a root `w:document` may be self-closing (no body follows)
            Event::Empty(e) if e.name().as_ref() == b"w:document" => {
                saw_document = true;
                if attr(&e, b"xmlns:w")?.as_deref() != Some(W_NS) {
                    return Err(ParseError(
                        "w: prefix is not bound to the WordprocessingML namespace — refusing to parse with wrong namespaces".into(),
                    ));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if !saw_document {
        return Err(ParseError(
            "no w:document root — not a wordprocessingml document".into(),
        ));
    }
    Err(ParseError("no w:body in document".into()))
}

/// Parse a header (`w:hdr`, §17.10.4) or footer (`w:ftr`, §17.10.3) part;
/// the root element is checked, not assumed.
pub fn parse_container(
    xml: &str,
    rels: &RelsList,
    base_dir: &str,
    expected_root: &[u8],
) -> Result<Vec<Block>, ParseError> {
    let mut parser = Parser {
        reader: quick_xml::Reader::from_str(xml),
        rels,
        base_dir,
    };
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match parser.reader.read_event_into(&mut buf).map_err(xml_err)? {
            Event::Start(e) => {
                if e.name().as_ref() != expected_root {
                    return Err(ParseError(format!(
                        "expected root {} in header/footer part, found {:?}",
                        String::from_utf8_lossy(expected_root),
                        String::from_utf8_lossy(e.name().as_ref()),
                    )));
                }
                return parser.parse_blocks(None);
            }
            Event::Eof => {
                return Err(ParseError(format!(
                    "no {} root in header/footer part",
                    String::from_utf8_lossy(expected_root)
                )))
            }
            _ => {}
        }
    }
}

impl Parser<'_> {
    /// Block stream (body content or a table cell): `w:p` (§17.3.1.22) and
    /// `w:tbl` (§17.4.38) blocks until the stop element closes, or EOF for a
    /// part-level stream.
    fn parse_blocks(&mut self, stop: Option<&[u8]>) -> Result<Vec<Block>, ParseError> {
        let mut buf = Vec::new();
        let mut blocks = Vec::new();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) => match e.name().as_ref() {
                    b"w:p" => blocks.push(Block::Paragraph(self.parse_paragraph()?)),
                    b"w:tbl" => blocks.push(Block::Table(self.parse_table()?)),
                    _ => self.skip_element()?,
                },
                Event::Empty(e) => match e.name().as_ref() {
                    b"w:p" => blocks.push(Block::Paragraph(Paragraph::default())),
                    b"w:tbl" => blocks.push(Block::Table(Table::default())),
                    _ => {}
                },
                Event::End(e) => {
                    if stop == Some(e.name().as_ref()) {
                        return Ok(blocks);
                    }
                }
                Event::Eof => match stop {
                    None => return Ok(blocks),
                    Some(s) => {
                        return Err(ParseError(format!(
                            "unexpected end of XML inside block stream (open {})",
                            String::from_utf8_lossy(s)
                        )))
                    }
                },
                _ => {}
            }
        }
    }

    /// `w:p` (§17.3.1.22): direct `w:pPr` (§17.3.1.25) then run content.
    fn parse_paragraph(&mut self) -> Result<Paragraph, ParseError> {
        let mut buf = Vec::new();
        let mut para = Paragraph::default();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) => match e.name().as_ref() {
                    b"w:pPr" => self.parse_ppr(&mut para)?,
                    b"w:r" => para.runs.push(self.parse_run()?),
                    _ => self.skip_element()?,
                },
                // leaf children of w:p other than pPr/r carry no run text
                Event::Empty(_) => {}
                Event::End(e) if e.name().as_ref() == b"w:p" => return Ok(para),
                Event::Eof => return Err(ParseError("unexpected end of XML inside w:p".into())),
                _ => {}
            }
        }
    }

    /// `w:pPr` (§17.3.1.25) — S1 models pStyle (§17.3.1.27), jc (§17.3.1.13)
    /// and numPr (§17.3.1.19). The pPr's own `w:rPr` (§17.3.1.29) styles the
    /// paragraph MARK, not runs — skipped wholesale.
    fn parse_ppr(&mut self, para: &mut Paragraph) -> Result<(), ParseError> {
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) => match e.name().as_ref() {
                    b"w:pStyle" => para.style = attr(&e, b"w:val")?,
                    b"w:jc" => para.align = attr(&e, b"w:val")?,
                    b"w:numPr" => para.numbering = Some(self.parse_numpr()?),
                    _ => self.skip_element()?,
                },
                Event::Empty(e) => match e.name().as_ref() {
                    b"w:pStyle" => para.style = attr(&e, b"w:val")?,
                    b"w:jc" => para.align = attr(&e, b"w:val")?,
                    _ => {} // e.g. <w:numPr/> — carries no reference to model
                },
                Event::End(e) if e.name().as_ref() == b"w:pPr" => return Ok(()),
                Event::Eof => return Err(ParseError("unexpected end of XML inside w:pPr".into())),
                _ => {}
            }
        }
    }

    /// `w:numPr` (§17.3.1.19): numId (§17.9.19) + ilvl (§17.9.3); both
    /// required — a partial reference is refused rather than defaulted.
    fn parse_numpr(&mut self) -> Result<NumRef, ParseError> {
        let mut buf = Vec::new();
        let mut num_id = None;
        let mut ilvl = None;
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                    b"w:numId" => num_id = Some(attr_u32(&e, b"w:val")?),
                    b"w:ilvl" => ilvl = Some(attr_u32(&e, b"w:val")?),
                    _ => {}
                },
                Event::End(e) if e.name().as_ref() == b"w:numPr" => {
                    return Ok(NumRef {
                        num_id: num_id
                            .ok_or_else(|| ParseError("w:numPr missing w:numId".into()))?,
                        ilvl: ilvl.ok_or_else(|| ParseError("w:numPr missing w:ilvl".into()))?,
                    })
                }
                Event::Eof => {
                    return Err(ParseError("unexpected end of XML inside w:numPr".into()))
                }
                _ => {}
            }
        }
    }

    /// `w:r` (§17.3.2.25): run properties + text/drawing content.
    fn parse_run(&mut self) -> Result<Run, ParseError> {
        let mut buf = Vec::new();
        let mut run = Run::default();
        let mut in_t = false;
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) => match e.name().as_ref() {
                    b"w:rPr" => self.parse_rpr(&mut run)?,
                    b"w:t" => in_t = true, // §17.3.3.31
                    b"w:drawing" => run.images.push(self.parse_drawing()?), // §17.3.3.9
                    _ => self.skip_element()?,
                },
                Event::Empty(_) => {} // <w:t/>, w:br/, w:tab/ — nothing to model
                Event::Text(t) if in_t => {
                    run.text.push_str(&t.unescape().map_err(xml_err)?);
                }
                // CDATA inside w:t is skipped (Word never emits it; S1 scope)
                Event::CData(_) => {}
                Event::End(e) => match e.name().as_ref() {
                    b"w:t" => in_t = false,
                    b"w:r" => return Ok(run),
                    _ => {}
                },
                Event::Eof => return Err(ParseError("unexpected end of XML inside w:r".into())),
                _ => {}
            }
        }
    }

    /// `w:rPr` (§17.3.2.27) — S1 models b/i/u/sz; everything else skipped.
    fn parse_rpr(&mut self, run: &mut Run) -> Result<(), ParseError> {
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                    b"w:b" => run.bold = onoff(&e)?,
                    b"w:i" => run.italic = onoff(&e)?,
                    b"w:u" => {
                        // val default "single" (§17.3.2.40 @val)
                        run.underline =
                            Some(attr(&e, b"w:val")?.unwrap_or_else(|| "single".to_string()));
                    }
                    b"w:sz" => run.size = Some(attr_u32(&e, b"w:val")?), // half-points
                    _ => {}
                },
                Event::End(e) if e.name().as_ref() == b"w:rPr" => return Ok(()),
                Event::Eof => return Err(ParseError("unexpected end of XML inside w:rPr".into())),
                _ => {}
            }
        }
    }

    /// `w:drawing` (§17.3.3.9) → `ImageRef`: `wp:extent` (§20.4.2.7, inside
    /// the `wp:inline` §20.4.2.8 wrapper) + `a:blip r:embed` resolved through
    /// the part's relationships. S1 reads the relationship only.
    fn parse_drawing(&mut self) -> Result<ImageRef, ParseError> {
        let mut buf = Vec::new();
        let mut extent: Option<(i64, i64)> = None;
        let mut embed: Option<String> = None;
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(_) => {} // graphic/pic wrappers hold nothing S1 needs
                Event::Empty(e) => match e.name().as_ref() {
                    b"wp:extent" => {
                        let cx = attr_i64(&e, b"cx")?;
                        let cy = attr_i64(&e, b"cy")?;
                        extent = Some((cx, cy));
                    }
                    b"a:blip" => embed = attr(&e, b"r:embed")?,
                    _ => {}
                },
                Event::End(e) if e.name().as_ref() == b"w:drawing" => {
                    let (cx_emu, cy_emu) =
                        extent.ok_or_else(|| ParseError("w:drawing missing wp:extent".into()))?;
                    let rel_id = embed
                        .ok_or_else(|| ParseError("w:drawing missing a:blip r:embed".into()))?;
                    let rel = self
                        .rels
                        .iter()
                        .find(|(id, _)| *id == rel_id)
                        .map(|(_, r)| r)
                        .ok_or_else(|| {
                            ParseError(format!("image relationship {rel_id} not found"))
                        })?;
                    if rel.rel_type != opc::RT_IMAGE {
                        return Err(ParseError(format!(
                            "relationship {rel_id} is not an image relationship — refusing"
                        )));
                    }
                    if rel.external {
                        return Err(ParseError(format!(
                            "image relationship {rel_id} is external — S1 reads embedded images only"
                        )));
                    }
                    let part = opc::resolve_target(self.base_dir, &rel.target)?;
                    return Ok(ImageRef {
                        rel_id,
                        part,
                        cx_emu,
                        cy_emu,
                    });
                }
                Event::Eof => {
                    return Err(ParseError("unexpected end of XML inside w:drawing".into()))
                }
                _ => {}
            }
        }
    }

    /// `w:tbl` (§17.4.38): `w:tr` rows (§17.4.79); tblPr/tblGrid skipped.
    fn parse_table(&mut self) -> Result<Table, ParseError> {
        let mut buf = Vec::new();
        let mut rows = Vec::new();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) => match e.name().as_ref() {
                    b"w:tr" => rows.push(self.parse_row()?),
                    _ => self.skip_element()?,
                },
                Event::Empty(_) => {}
                Event::End(e) if e.name().as_ref() == b"w:tbl" => return Ok(Table { rows }),
                Event::Eof => return Err(ParseError("unexpected end of XML inside w:tbl".into())),
                _ => {}
            }
        }
    }

    /// `w:tr` (§17.4.79): `w:tc` cells (§17.4.66); trPr skipped.
    fn parse_row(&mut self) -> Result<Vec<TableCell>, ParseError> {
        let mut buf = Vec::new();
        let mut cells = Vec::new();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) => match e.name().as_ref() {
                    b"w:tc" => cells.push(self.parse_cell()?),
                    _ => self.skip_element()?,
                },
                Event::Empty(e) if e.name().as_ref() == b"w:tc" => {
                    cells.push(TableCell::default());
                }
                Event::End(e) if e.name().as_ref() == b"w:tr" => return Ok(cells),
                Event::Eof => return Err(ParseError("unexpected end of XML inside w:tr".into())),
                _ => {}
            }
        }
    }

    /// `w:tc` (§17.4.66): block-level content (paragraphs + nested tables
    /// per the CT_Tc content model); tcPr skipped.
    fn parse_cell(&mut self) -> Result<TableCell, ParseError> {
        Ok(TableCell {
            blocks: self.parse_blocks(Some(b"w:tc"))?,
        })
    }

    /// Skip the subtree rooted at a `Start` event just consumed. Depth
    /// counting saturates so unmatched end tags can never underflow.
    fn skip_element(&mut self) -> Result<(), ParseError> {
        let mut buf = Vec::new();
        let mut depth = 1usize;
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(_) => depth += 1,
                Event::End(_) => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Ok(());
                    }
                }
                Event::Eof => {
                    return Err(ParseError("unexpected end of XML while skipping".into()))
                }
                _ => {}
            }
        }
    }
}

/// `w:val` of an OnOff element (`<w:b/>`, `<w:b w:val="…"/>`): absent or
/// true/1/on = on; false/0/off = off (§17.3.3 val ST_OnOff).
pub(crate) fn onoff(e: &BytesStart<'_>) -> Result<bool, ParseError> {
    Ok(match attr(e, b"w:val")?.as_deref() {
        None | Some("true") | Some("1") | Some("on") => true,
        Some("false") | Some("0") | Some("off") => false,
        Some(other) => return Err(ParseError(format!("w:val={other:?} is not an OnOff value"))),
    })
}

fn attr_u32(e: &BytesStart<'_>, key: &[u8]) -> Result<u32, ParseError> {
    let v = attr(e, key)?.ok_or_else(|| {
        ParseError(format!(
            "missing w:val on {}",
            String::from_utf8_lossy(e.name().as_ref())
        ))
    })?;
    v.parse()
        .map_err(|_| ParseError(format!("w:val={v:?} is not a u32")))
}

fn attr_i64(e: &BytesStart<'_>, key: &[u8]) -> Result<i64, ParseError> {
    let v = attr(e, key)?.ok_or_else(|| {
        ParseError(format!(
            "missing {0} on {1}",
            String::from_utf8_lossy(key),
            String::from_utf8_lossy(e.name().as_ref())
        ))
    })?;
    v.parse().map_err(|_| {
        ParseError(format!(
            "{0}={v:?} is not an integer",
            String::from_utf8_lossy(key)
        ))
    })
}
