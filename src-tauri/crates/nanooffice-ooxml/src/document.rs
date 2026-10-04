//! `word/document.xml` (+ `w:hdr`/`w:ftr` parts) → blocks (ECMA-376 Part 1,
//! WordprocessingML §17).
//!
//! Recursive descent: one parser function per construct, each citing its
//! spec clause. Elements/attributes are matched by their prefixed names and
//! the root must bind the `w:` prefix to the WordprocessingML namespace —
//! anything outside the S1 construct table is skipped (subtree, no state
//! carried) rather than guessed at, with ONE exception: `mc:AlternateContent`
//! run content is resolved through its first `mc:Choice` branch (Markup
//! Compatibility, see [`Parser::parse_alt_content`]); documents that bind
//! `w:` wrongly are refused (DD §6 "refuse-unknown instead of parse-wrong").

use quick_xml::events::{BytesStart, Event};

use crate::opc::{self, attr, Rel};
use crate::{
    xml_err, Block, Hyperlink, ImageRef, Ind, NumRef, Page, PageMar, Paragraph, ParseError, RFonts,
    Run, Section, Table, TableCell,
};

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// Relationship table of the part being parsed (for `r:embed` resolution).
type RelsList = Vec<(String, Rel)>;

struct Parser<'a> {
    reader: quick_xml::Reader<&'a [u8]>,
    rels: &'a RelsList,
    base_dir: &'a str,
    /// Page size of the body-level `w:sectPr` (§17.6.17) — set while parsing
    /// the main body stream, left alone for headers/footers.
    page: Option<Page>,
    /// Same rule for the final section's `w:pgMar` (§17.6.11, T-A31).
    page_mar: Option<PageMar>,
}

/// The main-part parse result: body blocks + the FINAL section's typed
/// minimal (§17.6.17 — pgSz + pgMar).
pub(crate) struct BodyParse {
    pub blocks: Vec<Block>,
    pub page: Option<Page>,
    pub page_mar: Option<PageMar>,
}

/// Parse the main document part: check the `w:document` root (§17.2.3) and
/// return the `w:body` (§17.2.2) block stream plus the final section's page
/// size, if declared.
pub fn parse_body(xml: &str, rels: &RelsList, base_dir: &str) -> Result<BodyParse, ParseError> {
    let mut parser = Parser {
        reader: quick_xml::Reader::from_str(xml),
        rels,
        base_dir,
        page: None,
        page_mar: None,
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
                b"w:body" if saw_document => {
                    return Ok(BodyParse {
                        blocks: parser.parse_blocks(None)?,
                        page: parser.page,
                        page_mar: parser.page_mar,
                    });
                }
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
        page: None,
        page_mar: None,
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
                    // the body-level final section (§17.6.17) — only pgSz/pgMar
                    // are modelled; mid-document sections live inside w:pPr
                    b"w:sectPr" => {
                        let (page, mar) = self.parse_sectpr()?;
                        self.page = page;
                        self.page_mar = mar;
                    }
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

    /// `w:p` (§17.3.1.22): direct `w:pPr` (§17.3.1.25) then run content —
    /// including `w:hyperlink` groups (§17.16.22, T-A31).
    fn parse_paragraph(&mut self) -> Result<Paragraph, ParseError> {
        let mut buf = Vec::new();
        let mut para = Paragraph::default();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) => match e.name().as_ref() {
                    b"w:pPr" => self.parse_ppr(&mut para)?,
                    b"w:r" => para.runs.push(self.parse_run()?),
                    b"w:hyperlink" => {
                        let rel_id = attr(&e, b"r:id")?;
                        let anchor = attr(&e, b"w:anchor")?;
                        self.parse_hyperlink(&mut para, rel_id, anchor)?;
                    }
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

    /// `w:hyperlink` (§17.16.22) content: every run gets the group's link
    /// (§17.16.22 may nest hyperlinks — the innermost group wins). A group
    /// with NEITHER `r:id` NOR `w:anchor` is refused, not guessed.
    fn parse_hyperlink(
        &mut self,
        para: &mut Paragraph,
        rel_id: Option<String>,
        anchor: Option<String>,
    ) -> Result<(), ParseError> {
        let link = self.resolve_link(rel_id, anchor)?;
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) => match e.name().as_ref() {
                    b"w:r" => {
                        let mut run = self.parse_run()?;
                        run.link = Some(link.clone());
                        para.runs.push(run);
                    }
                    b"w:hyperlink" => {
                        let rel = attr(&e, b"r:id")?;
                        let anc = attr(&e, b"w:anchor")?;
                        self.parse_hyperlink(para, rel, anc)?;
                    }
                    _ => self.skip_element()?,
                },
                Event::End(e) if e.name().as_ref() == b"w:hyperlink" => return Ok(()),
                Event::Eof => {
                    return Err(ParseError(
                        "unexpected end of XML inside w:hyperlink".into(),
                    ))
                }
                _ => {}
            }
        }
    }

    /// Build the model link from a group's `r:id`/`w:anchor`: the relationship
    /// must exist and be a hyperlink rel; external targets stay verbatim URIs,
    /// internal ones resolve against the part's directory.
    fn resolve_link(
        &self,
        rel_id: Option<String>,
        anchor: Option<String>,
    ) -> Result<Hyperlink, ParseError> {
        if let Some(rel_id) = rel_id {
            let rel = self
                .rels
                .iter()
                .find(|(id, _)| *id == rel_id)
                .map(|(_, r)| r)
                .ok_or_else(|| ParseError(format!("hyperlink relationship {rel_id} not found")))?;
            if rel.rel_type != opc::RT_HYPERLINK {
                return Err(ParseError(format!(
                    "relationship {rel_id} is not a hyperlink relationship — refusing"
                )));
            }
            let target = if rel.external {
                rel.target.clone()
            } else {
                opc::resolve_target(self.base_dir, &rel.target)?
            };
            Ok(Hyperlink {
                rel_id: Some(rel_id),
                anchor,
                target: Some(target),
                external: rel.external,
            })
        } else if let Some(anchor) = anchor {
            Ok(Hyperlink {
                rel_id: None,
                anchor: Some(anchor),
                target: None,
                external: false,
            })
        } else {
            Err(ParseError(
                "w:hyperlink without r:id or w:anchor — refusing".into(),
            ))
        }
    }

    /// `w:pPr` (§17.3.1.25) — S1 models pStyle (§17.3.1.27), jc (§17.3.1.13),
    /// numPr (§17.3.1.19) and ind (§17.3.1.12). The pPr's own `w:rPr`
    /// (§17.3.1.29) styles the paragraph MARK, not runs — skipped wholesale.
    fn parse_ppr(&mut self, para: &mut Paragraph) -> Result<(), ParseError> {
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) => match e.name().as_ref() {
                    b"w:pStyle" => para.style = attr(&e, b"w:val")?,
                    b"w:jc" => para.align = attr(&e, b"w:val")?,
                    b"w:numPr" => para.numbering = Some(self.parse_numpr()?),
                    b"w:ind" => para.ind = Some(parse_ind(&e)?),
                    // mid-document section mark (§17.6.18 inside w:pPr) —
                    // typed-minimal (pgSz + pgMar, T-A31)
                    b"w:sectPr" => {
                        let (page, mar) = self.parse_sectpr()?;
                        para.sect = Some(Box::new(Section { page, mar }));
                    }
                    _ => self.skip_element()?,
                },
                Event::Empty(e) => match e.name().as_ref() {
                    b"w:pStyle" => para.style = attr(&e, b"w:val")?,
                    b"w:jc" => para.align = attr(&e, b"w:val")?,
                    b"w:ind" => para.ind = Some(parse_ind(&e)?),
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
                    b"mc:AlternateContent" => self.parse_alt_content(&mut run)?,
                    // x2t mixes serializations (a:blip lesson, T-A23) — the
                    // break elements can arrive Start+End as well as empty;
                    // neither legally has content, so Start is consumed as a
                    // subtree after reading its attributes
                    b"w:tab" => {
                        run.text.push('\t'); // §17.3.3.34
                        self.skip_element()?;
                    }
                    b"w:br" => {
                        run.text.push(br_char(&e)?); // §17.3.3.3
                        self.skip_element()?;
                    }
                    _ => self.skip_element()?,
                },
                Event::Empty(e) => match e.name().as_ref() {
                    b"w:tab" => run.text.push('\t'),
                    b"w:br" => run.text.push(br_char(&e)?),
                    // <w:t/>, other leaves — nothing to model
                    _ => {}
                },
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

    /// `w:rPr` (§17.3.2.27) — S1 models rStyle (§17.5.2.3), rFonts
    /// (§17.3.2.26), b/i/u/sz, color (§17.3.2.6), spacing (§17.3.2.33);
    /// everything else skipped.
    fn parse_rpr(&mut self, run: &mut Run) -> Result<(), ParseError> {
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                    b"w:b" => run.bold = Some(onoff(&e)?),
                    b"w:i" => run.italic = Some(onoff(&e)?),
                    b"w:u" => {
                        // val default "single" (§17.3.2.40 @val)
                        run.underline =
                            Some(attr(&e, b"w:val")?.unwrap_or_else(|| "single".to_string()));
                    }
                    b"w:sz" => run.size = Some(attr_u32(&e, b"w:val")?), // half-points
                    b"w:rStyle" => run.rstyle = attr(&e, b"w:val")?,
                    b"w:rFonts" => run.rfonts = Some(parse_rfonts(&e)?),
                    b"w:color" => run.color = attr(&e, b"w:val")?,
                    b"w:spacing" => run.spacing = Some(attr_i32(&e, b"w:val")?), // twips
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
                // wp:extent and a:blip can arrive as Start+End or self-closing
                // Empty — x2t (and Word) emit `<a:blip r:embed=…></a:blip>`
                // (caught by the T-A23 parity harness on a real x2t round-trip)
                Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
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

    /// `mc:AlternateContent` (namespace
    /// `http://schemas.openxmlformats.org/markup-compatibility/2006`): one or
    /// more `mc:Choice` branches plus an optional `mc:Fallback` — the same
    /// content in alternative serializations. Spec rule (spec-ref: ECMA-376
    /// Markup Compatibility, AlternateContent element — ECMA-376 Part 5 1st
    /// ed., "Markup Compatibility Attributes and Elements"; Part 3 in
    /// ISO/IEC 29500 editions): the selected branch "is processed as though
    /// it replaces the entire AlternateContent element", and unselected
    /// branches raise no error. This reader understands run content
    /// (w:rPr/w:t/w:drawing), so the FIRST `mc:Choice` is selected — that is
    /// where both Word and x2t put the DrawingML representation; the VML
    /// `w:pict` in `mc:Fallback` is not modelled. Prefix is matched as the
    /// literal `mc:` (the one Word/x2t emit); a part binding another prefix
    /// to the compatibility namespace is out of S3 scope.
    fn parse_alt_content(&mut self, run: &mut Run) -> Result<(), ParseError> {
        const OUTER: &[u8] = b"mc:AlternateContent";
        const CHOICE: &[u8] = b"mc:Choice";
        let mut buf = Vec::new();
        let mut chosen = false; // a branch has already been selected
        let mut in_choice = false;
        let mut in_t = false;
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) => match e.name().as_ref() {
                    CHOICE if chosen => self.skip_element()?, // later branches lose
                    CHOICE => in_choice = true,
                    n if in_choice && n == b"w:rPr" => self.parse_rpr(run)?,
                    n if in_choice && n == b"w:t" => in_t = true, // §17.3.3.31
                    n if in_choice && n == b"w:drawing" => run.images.push(self.parse_drawing()?),
                    n if in_choice && n == b"w:tab" => {
                        run.text.push('\t');
                        self.skip_element()?;
                    }
                    n if in_choice && n == b"w:br" => {
                        run.text.push(br_char(&e)?);
                        self.skip_element()?;
                    }
                    _ => self.skip_element()?, // Fallback, unknown Choice content
                },
                // leaf content inside the selected branch carries no model data
                Event::Empty(e) => match e.name().as_ref() {
                    b"w:tab" if in_choice => run.text.push('\t'),
                    b"w:br" if in_choice => run.text.push(br_char(&e)?),
                    _ => {}
                },
                Event::Text(t) if in_t => {
                    run.text.push_str(&t.unescape().map_err(xml_err)?);
                }
                Event::End(e) => match e.name().as_ref() {
                    OUTER => return Ok(()),
                    CHOICE => {
                        in_choice = false;
                        chosen = true;
                    }
                    b"w:t" => in_t = false,
                    _ => {}
                },
                Event::Eof => {
                    return Err(ParseError(
                        "unexpected end of XML inside mc:AlternateContent".into(),
                    ))
                }
                _ => {}
            }
        }
    }

    /// `w:tbl` (§17.4.38): `w:tblGrid` (§17.4.4) columns then `w:tr` rows
    /// (§17.4.79); tblPr skipped.
    fn parse_table(&mut self) -> Result<Table, ParseError> {
        let mut buf = Vec::new();
        let mut grid = Vec::new();
        let mut rows = Vec::new();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) => match e.name().as_ref() {
                    b"w:tr" => rows.push(self.parse_row()?),
                    b"w:tblGrid" => grid = self.parse_grid()?,
                    _ => self.skip_element()?,
                },
                Event::Empty(e) if e.name().as_ref() == b"w:tblGrid" => {} // no columns declared
                Event::End(e) if e.name().as_ref() == b"w:tbl" => return Ok(Table { grid, rows }),
                Event::Eof => return Err(ParseError("unexpected end of XML inside w:tbl".into())),
                _ => {}
            }
        }
    }

    /// `w:tblGrid` (§17.4.4): `w:gridCol` (§17.4.17) widths in order —
    /// `w:w` in twips; a grid column without `w:w` is an auto column (None).
    fn parse_grid(&mut self) -> Result<Vec<Option<u32>>, ParseError> {
        let mut buf = Vec::new();
        let mut cols = Vec::new();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) | Event::Empty(e) => {
                    if e.name().as_ref() == b"w:gridCol" {
                        // absent w:w = auto column; a present-but-invalid one
                        // is refused, not guessed
                        let w = match attr(&e, b"w:w")? {
                            Some(v) => Some(
                                v.parse()
                                    .map_err(|_| ParseError(format!("w:w={v:?} is not a u32")))?,
                            ),
                            None => None,
                        };
                        cols.push(w);
                    }
                }
                Event::End(e) if e.name().as_ref() == b"w:tblGrid" => return Ok(cols),
                Event::Eof => {
                    return Err(ParseError("unexpected end of XML inside w:tblGrid".into()))
                }
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

    /// `w:sectPr` (§17.6.17/§17.6.18): only `w:pgSz` (§17.6.13) + `w:pgMar`
    /// (§17.6.11) are modelled; everything else is skipped (and warned by
    /// `detect_save_losses`).
    fn parse_sectpr(&mut self) -> Result<(Option<Page>, Option<PageMar>), ParseError> {
        let mut buf = Vec::new();
        let mut page = Page::default();
        let mut mar = PageMar::default();
        loop {
            buf.clear();
            match self.reader.read_event_into(&mut buf).map_err(xml_err)? {
                Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                    b"w:pgSz" => {
                        page.w = attr_u32_opt(&e, b"w:w")?;
                        page.h = attr_u32_opt(&e, b"w:h")?;
                        page.orient = attr(&e, b"w:orient")?;
                    }
                    b"w:pgMar" => {
                        mar.top = attr_i64_opt(&e, b"w:top")?;
                        mar.right = attr_i64_opt(&e, b"w:right")?;
                        mar.bottom = attr_i64_opt(&e, b"w:bottom")?;
                        mar.left = attr_i64_opt(&e, b"w:left")?;
                        mar.header = attr_i64_opt(&e, b"w:header")?;
                        mar.footer = attr_i64_opt(&e, b"w:footer")?;
                        mar.gutter = attr_i64_opt(&e, b"w:gutter")?;
                    }
                    _ => {}
                },
                Event::End(e) if e.name().as_ref() == b"w:sectPr" => {
                    return Ok((Some(page), Some(mar)));
                }
                Event::Eof => {
                    return Err(ParseError("unexpected end of XML inside w:sectPr".into()))
                }
                _ => {}
            }
        }
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

/// `w:br` (§17.3.3.3) → its control character: no `w:type` or "text" (the
/// default) = line break `\n`; "page" = `\u{c}`; "column" = `\u{b}`. All
/// three are illegal inside XML text content, so a `w:t` can never collide
/// with them. Other type values are refused (ST_BrType §17.18.49 is a closed
/// enum — parsing an unknown one as a line break would corrupt the model).
fn br_char(e: &BytesStart<'_>) -> Result<char, ParseError> {
    Ok(match attr(e, b"w:type")?.as_deref() {
        None | Some("text") => '\n',
        Some("page") => '\u{c}',
        Some("column") => '\u{b}',
        Some(other) => {
            return Err(ParseError(format!(
                "w:br w:type={other:?} is not a ST_BrType value"
            )))
        }
    })
}

/// `w:ind` (§17.3.1.12) attributes, twips; each is optional as written.
pub(crate) fn parse_ind(e: &BytesStart<'_>) -> Result<Ind, ParseError> {
    let signed = |v: Option<String>| -> Result<Option<i64>, ParseError> {
        v.map(|s| {
            s.parse::<i64>()
                .map_err(|_| ParseError(format!("w:ind value {s:?} is not an integer")))
        })
        .transpose()
    };
    Ok(Ind {
        left: signed(attr(e, b"w:left")?)?,
        right: signed(attr(e, b"w:right")?)?,
        first_line: signed(attr(e, b"w:firstLine")?)?,
        hanging: signed(attr(e, b"w:hanging")?)?,
    })
}

/// `w:rFonts` (§17.3.2.26) literal family attributes; theme attributes
/// (asciiTheme…) are not modelled.
pub(crate) fn parse_rfonts(e: &BytesStart<'_>) -> Result<RFonts, ParseError> {
    Ok(RFonts {
        ascii: attr(e, b"w:ascii")?,
        hansi: attr(e, b"w:hAnsi")?,
        eastasia: attr(e, b"w:eastAsia")?,
        cs: attr(e, b"w:cs")?,
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

fn attr_u32_opt(e: &BytesStart<'_>, key: &[u8]) -> Result<Option<u32>, ParseError> {
    match attr(e, key)? {
        Some(v) => v.parse().map(Some).map_err(|_| {
            ParseError(format!(
                "{0}={v:?} is not a u32",
                String::from_utf8_lossy(key)
            ))
        }),
        None => Ok(None),
    }
}

fn attr_i64_opt(e: &BytesStart<'_>, key: &[u8]) -> Result<Option<i64>, ParseError> {
    match attr(e, key)? {
        Some(v) => v.parse().map(Some).map_err(|_| {
            ParseError(format!(
                "{0}={v:?} is not an integer",
                String::from_utf8_lossy(key)
            ))
        }),
        None => Ok(None),
    }
}

/// Signed twips (`w:spacing` §17.3.2.33).
pub(crate) fn attr_i32(e: &BytesStart<'_>, key: &[u8]) -> Result<i32, ParseError> {
    let v = attr(e, key)?.ok_or_else(|| {
        ParseError(format!(
            "missing w:val on {}",
            String::from_utf8_lossy(e.name().as_ref())
        ))
    })?;
    v.parse()
        .map_err(|_| ParseError(format!("w:val={v:?} is not an i32")))
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
