//! Slice tests (T-A21, DD §7): fixture chuẩn (tiếng Việt gộp) + corpus đầy
//! đủ mọi construct S1 + edge XML + construct semantics + fuzz. 5 test,
//! không framework.

use std::io::Write as _;

use nanooffice_ooxml::{Block, Document, Paragraph, TableCell};

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/nanooffice-fixture.docx"
);
const FULL_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/nanooffice-full-fixture.docx"
);

/// Fixture chuẩn (sinh bằng `harness/fixtures/make_docx.py` — spec-derived,
/// cùng file render GREEN trong engine): 5 paragraph, style, bold, tiếng Việt
/// có dấu, tab literal, smart quotes — text phải exact, không panic.
#[test]
fn fixture_parses_to_exact_model() {
    let doc = nanooffice_ooxml::parse_docx_file(FIXTURE).unwrap();
    assert_eq!(doc.body.len(), 5);

    let p = para(&doc, 0);
    assert_eq!(p.style.as_deref(), Some("Heading1"));
    assert_eq!(p.runs.len(), 1);
    assert_eq!(p.runs[0].text, "NanoOffice PH-0 Fixture");
    assert!(!p.runs[0].bold);

    let p = para(&doc, 2);
    assert_eq!(p.style, None);
    assert_eq!(p.runs.len(), 1);
    assert!(p.runs[0].bold, "whole-sentence bold run");
    assert_eq!(
        p.runs[0].text,
        "Bold run check: this whole sentence should be bold."
    );

    let p = para(&doc, 3);
    assert_eq!(
        p.runs[0].text,
        "Đoạn tiếng Việt có dấu: ẽnghệ ộ lại là là — kiểm tra font fallback."
    );

    let p = para(&doc, 4);
    assert_eq!(
        p.runs[0].text,
        "0123456789 — tab\there — quotes “smart” and 'plain'."
    );
}

/// Corpus đầy đủ (sinh bằng `harness/fixtures/make_docx_full.py`): mọi
/// construct S1 — style chain, run props, list/table/image, header/footer,
/// tiếng Việt — parse đúng spec, 100% surface.
#[test]
fn full_corpus_parses_every_construct() {
    let doc = nanooffice_ooxml::parse_docx_file(FULL_FIXTURE).unwrap();

    // body: 7 paragraphs + 1 table + 1 image paragraph + 1 closing paragraph
    assert_eq!(doc.body.len(), 10);

    // style chain: Heading1 basedOn Normal — child wins (b + sz 32 override)
    let h1 = &doc.styles["Heading1"];
    assert_eq!(h1.name.as_deref(), Some("heading 1"));
    assert_eq!(h1.bold, Some(true));
    assert_eq!(h1.size, Some(32));
    let normal = &doc.styles["Normal"];
    assert_eq!(normal.bold, None);
    assert_eq!(normal.size, Some(22));
    assert_eq!(doc.styles["EmphasisChar"].italic, Some(true));
    assert!(
        !doc.styles.contains_key("TableNormal"),
        "table style skipped"
    );

    let p = para(&doc, 0);
    assert_eq!(p.style.as_deref(), Some("Heading1"));
    assert_eq!(p.runs[0].text, "Giới thiệu NanoOffice");

    // run-level bold=false overrides the style chain (raw values kept)
    let p = para(&doc, 1);
    assert_eq!(p.style.as_deref(), Some("Heading1"));
    assert!(!p.runs[0].bold, "run override w:b w:val=false");

    // paragraph alignment
    let p = para(&doc, 2);
    assert_eq!(p.align.as_deref(), Some("center"));

    // run props: bold / italic / underline / size / plain
    let runs: &[&nanooffice_ooxml::Run] = &[
        &para(&doc, 3).runs[0],
        &para(&doc, 3).runs[1],
        &para(&doc, 3).runs[2],
        &para(&doc, 3).runs[3],
        &para(&doc, 3).runs[4],
    ];
    assert_eq!(runs[0].text, "bold ");
    assert!(runs[0].bold && !runs[0].italic);
    assert_eq!(runs[1].text, "nghiêng ");
    assert!(runs[1].italic && !runs[1].bold);
    assert_eq!(runs[2].text, "gạch chân ");
    assert_eq!(runs[2].underline.as_deref(), Some("single"));
    assert_eq!(runs[3].text, "cỡ 14pt ");
    assert_eq!(runs[3].size, Some(28));
    assert_eq!(runs[4].text, "thường");
    assert!(runs[4].underline.is_none() && runs[4].size.is_none());

    // lists: raw numPr references + resolved numbering dictionary
    let p = para(&doc, 4);
    let num = p.numbering.as_ref().expect("numPr on list paragraph");
    assert_eq!((num.num_id, num.ilvl), (1, 0));
    let sub = para(&doc, 5).numbering.as_ref().unwrap();
    assert_eq!((sub.num_id, sub.ilvl), (1, 1));
    assert_eq!(para(&doc, 6).numbering.as_ref().unwrap().num_id, 2);
    let lvl = &doc.numbering[&1][&0];
    assert_eq!(lvl.num_fmt, "decimal");
    assert_eq!(lvl.lvl_text, "%1.");
    assert_eq!(doc.numbering[&1][&1].num_fmt, "lowerLetter");
    assert_eq!(doc.numbering[&1][&1].lvl_text, "%2)");
    assert_eq!(doc.numbering[&2][&0].num_fmt, "bullet");
    assert_eq!(doc.numbering[&2][&0].lvl_text, "•");

    // table 2×2: cell text via nested paragraphs
    let cell = |r: usize, c: usize| cell_text(&doc, 7, r, c);
    assert_eq!(cell(0, 0), "Tên");
    assert_eq!(cell(0, 1), "Giá trị");
    assert_eq!(cell(1, 0), "RAM");
    assert_eq!(cell(1, 1), "561 MB");

    // image: relationship-only — r:embed resolved to its OPC part + EMU extent
    let img = &para(&doc, 8).runs[0].images[0];
    assert_eq!(img.rel_id, "rIdImg");
    assert_eq!(img.part, "word/media/image1.png");
    assert_eq!((img.cx_emu, img.cy_emu), (9525, 9525));

    // header/footer parts, content parsed with the container parser
    assert_eq!(doc.headers.len(), 1);
    assert_eq!(doc.headers[0].part, "word/header1.xml");
    assert_eq!(
        block_para(&doc.headers[0].blocks, 0).runs[0].text,
        "NanoOffice — văn bản mẫu"
    );
    assert_eq!(doc.footers.len(), 1);
    assert_eq!(
        block_para(&doc.footers[0].blocks, 0).runs[0].text,
        "Trang đầu"
    );

    // Vietnamese diacritics intact to the end of the body
    let p = para(&doc, 9);
    assert_eq!(
        p.runs[0].text,
        "Đoạn kết thúc — ẽnghệ ộ lại là là, kiểm tra font fallback."
    );

    // CLI JSON face: the whole model serializes (consumer of every field)
    let json = serde_json::to_string(&doc).unwrap();
    assert!(json.contains("word/media/image1.png"));
    assert!(json.contains("\"numbering\""));
}

/// Trust boundary: malformed input → `Err`, never panic — across OPC
/// structure, XML, styles resolution and relationships.
#[test]
fn malformed_input_is_an_error_not_a_panic() {
    // (a) garbage XML in the document part
    let doc = minimal_package("this is not xml at all <<<");
    assert!(nanooffice_ooxml::parse_docx(&doc).is_err());

    // (b) missing package root relationships
    let parts = vec![
        ("[Content_Types].xml", content_types_xml("")),
        ("word/document.xml", empty_document()),
    ];
    let err = nanooffice_ooxml::parse_docx(&zip_parts(&parts)).unwrap_err();
    assert!(err.0.contains("part _rels/.rels"), "unexpected: {err:?}");

    // (c) no officeDocument relationship in the root rels
    let parts = vec![
        ("[Content_Types].xml", content_types_xml("")),
        (
            "_rels/.rels",
            rels_xml(&[(
                "rId1",
                "http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles",
                "word/styles.xml",
            )]),
        ),
        ("word/document.xml", empty_document()),
    ];
    let err = nanooffice_ooxml::parse_docx(&zip_parts(&parts)).unwrap_err();
    assert!(
        err.0.contains("no officeDocument relationship"),
        "unexpected: {err:?}"
    );

    // (d) main part with a non-WordprocessingML content type — refuse
    let parts = vec![
        ("[Content_Types].xml", content_types_xml("")),
        ("_rels/.rels", root_rels_xml()),
        ("word/_rels/document.xml.rels", rels_xml(&[])),
        ("word/document.xml", empty_document()),
    ];
    let parts = parts
        .into_iter()
        .map(|(n, c)| {
            if n == "[Content_Types].xml" {
                (
                    n,
                    c.replace("wordprocessingml.document.main+xml", "sheet.main+xml"),
                )
            } else {
                (n, c)
            }
        })
        .collect::<Vec<_>>();
    let err = nanooffice_ooxml::parse_docx(&zip_parts(&parts)).unwrap_err();
    assert!(err.0.contains("refusing"), "unexpected: {err:?}");

    // (e) truncated zip
    let doc = minimal_package(&empty_document());
    let err = nanooffice_ooxml::parse_docx(&doc[..doc.len() / 2]).unwrap_err();
    assert!(err.0.contains("zip"), "unexpected: {err:?}");

    // (f) basedOn cycle in styles.xml
    let styles = concat!(
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:style w:type="paragraph" w:styleId="A"><w:basedOn w:val="B"/></w:style>"#,
        r#"<w:style w:type="paragraph" w:styleId="B"><w:basedOn w:val="A"/></w:style>"#,
        r#"</w:styles>"#,
    );
    let doc = minimal_package_with(
        &empty_document(),
        &style_image_rels(),
        &[("word/styles.xml", styles.to_string())],
    );
    let err = nanooffice_ooxml::parse_docx(&doc).unwrap_err();
    assert!(err.0.contains("cycle"), "unexpected: {err:?}");

    // (g) a:blip r:embed pointing at a relationship that does not exist
    let drawing = concat!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
        r#"<w:p><w:r><w:drawing><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing">"#,
        r#"<wp:extent cx="9525" cy="9525"/>"#,
        r#"<a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">"#,
        r#"<a:blip xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:embed="rIdGhost"/>"#,
        r#"</a:graphic></wp:inline></w:drawing></w:r></w:p>"#,
        r#"</w:body></w:document>"#,
    );
    let doc = minimal_package_with(&drawing, "", &[]);
    let err = nanooffice_ooxml::parse_docx(&doc).unwrap_err();
    assert!(err.0.contains("rIdGhost"), "unexpected: {err:?}");

    // (h) header part whose root is not w:hdr — refuse
    let doc_rels = format!(
        "{}<Relationship Id=\"rIdH\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/header\" Target=\"header1.xml\"/>",
        style_image_rels()
    );
    let doc = minimal_package_with(
        &empty_document(),
        &doc_rels,
        &[
            (
                "word/styles.xml",
                r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"/>"#.to_string(),
            ),
            ("word/header1.xml", empty_document()),
        ],
    );
    let err = nanooffice_ooxml::parse_docx(&doc).unwrap_err();
    assert!(err.0.contains("w:hdr"), "unexpected: {err:?}");

    // (i) officeDocument target points at a part that is not in the zip
    let parts = vec![
        ("[Content_Types].xml", content_types_xml("")),
        ("_rels/.rels", root_rels_xml()),
    ];
    let err = nanooffice_ooxml::parse_docx(&zip_parts(&parts)).unwrap_err();
    assert!(
        err.0.contains("part word/document.xml"),
        "unexpected: {err:?}"
    );
}

/// Construct semantics: pStyle qua pPr, OnOff `w:val` (false/0 tắt), `w:i`,
/// empty `<w:p/>`, entities trong `w:t`, jc/u/sz/numPr raw, pPr-rPr không
/// áp lên runs, empty cell.
#[test]
fn construct_semantics() {
    let xml = concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:body>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t xml:space="preserve">styled</w:t></w:r></w:p>"#,
        r#"<w:p>"#,
        r#"<w:r><w:rPr><w:b w:val="false"/></w:rPr><w:t>not bold</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:i/></w:rPr><w:t>italic</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:b/><w:i w:val="0"/></w:rPr><w:t>bold only</w:t></w:r>"#,
        r#"</w:p>"#,
        r#"<w:p/>"#,
        r#"<w:p><w:r><w:t>a &amp; b &lt;c&gt;</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:jc w:val="both"/><w:rPr><w:b/></w:rPr><w:numPr><w:ilvl w:val="3"/><w:numId w:val="7"/></w:numPr></w:pPr><w:r><w:t>props</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:rPr><w:u/><w:sz w:val="36"/></w:rPr><w:t>underline default + 18pt</w:t></w:r></w:p>"#,
        r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc><w:tc/></w:tr></w:tbl>"#,
        r#"</w:body></w:document>"#,
    );
    let doc = nanooffice_ooxml::parse_docx(&minimal_package(xml)).unwrap();
    assert_eq!(doc.body.len(), 7);

    let p = para(&doc, 0);
    assert_eq!(p.style.as_deref(), Some("Heading2"));
    assert_eq!(p.runs[0].text, "styled");

    let runs = &para(&doc, 1).runs;
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].text, "not bold");
    assert!(!runs[0].bold && !runs[0].italic);
    assert_eq!(runs[1].text, "italic");
    assert!(runs[1].italic && !runs[1].bold);
    assert_eq!(runs[2].text, "bold only");
    assert!(runs[2].bold && !runs[2].italic, "w:val=0 must turn w:i off");

    assert_eq!(para(&doc, 2).runs.len(), 0, "<w:p/> = empty paragraph");
    assert_eq!(para(&doc, 3).runs[0].text, "a & b <c>");

    // jc + numPr on the paragraph; the pPr's own rPr must NOT reach the runs;
    // numbering part absent → raw reference kept, empty dictionary
    let p = para(&doc, 4);
    assert_eq!(p.align.as_deref(), Some("both"));
    let num = p.numbering.as_ref().expect("numPr kept raw");
    assert_eq!((num.num_id, num.ilvl), (7, 3));
    assert!(
        doc.numbering.is_empty(),
        "no numbering part → empty dictionary"
    );
    assert!(!p.runs[0].bold, "pPr's rPr styles the mark, not runs");

    // w:u without w:val defaults to "single" (§17.3.2.40); w:sz half-points
    let r = &para(&doc, 5).runs[0];
    assert_eq!(r.underline.as_deref(), Some("single"));
    assert_eq!(r.size, Some(36));

    // table: one cell with a paragraph, one empty cell `<w:tc/>`
    let t = match &doc.body[6] {
        Block::Table(t) => t,
        other => panic!("expected table, got {other:?}"),
    };
    assert_eq!(t.rows.len(), 1);
    assert_eq!(t.rows[0].len(), 2);
    assert_eq!(cell_text(&doc, 6, 0, 0), "x");
    assert!(matches!(&t.rows[0][1], TableCell { blocks } if blocks.is_empty()));
}

/// Fuzz (mutation, seed cố định, không dep): XML mutations + zip mutations —
/// mọi trường hợp phải là `Ok` hoặc `Err`, KHÔNG panic.
#[test]
fn fuzz_mutations_never_panic() {
    let base_xml = concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:body>"#,
        r#"<w:p><w:pPr><w:pStyle w:val="H1"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Tiếng Việt &amp; “smart”</w:t></w:r></w:p>"#,
        r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        r#"</w:body></w:document>"#,
    );
    let base_zip = minimal_package(base_xml);
    assert!(
        nanooffice_ooxml::parse_docx(&base_zip).is_ok(),
        "precondition: unmutated base must parse"
    );

    let junk = [
        "<",
        ">",
        "&",
        "</w:p>",
        "<w:p",
        "&#xZZ;",
        "<x:y/>",
        "&amp",
        "\"",
        "\u{0}",
        "<w:b/>",
        "<w:document>",
        "</w:body>",
        "<?xml?>",
    ];
    let mut state: u64 = 42;
    let mut rand = move || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        state >> 33
    };

    for _ in 0..500u32 {
        let bytes = match rand() % 3 {
            // (1) XML mutation: truncate / splice junk / flip a byte
            0 => {
                let mut xml = base_xml.as_bytes().to_vec();
                let cut = (rand() as usize) % (xml.len() + 1);
                xml.truncate(cut);
                let at = (rand() as usize) % (xml.len() + 1);
                let j = junk[(rand() as usize) % junk.len()];
                xml.splice(at..at, j.bytes());
                minimal_package_with_bytes(&xml)
            }
            // (2) zip corruption: flip one byte anywhere in the container
            1 => {
                let mut zip = base_zip.clone();
                let at = (rand() as usize) % zip.len();
                zip[at] ^= 1 + (rand() as u8) % 255;
                zip
            }
            // (3) namespace/root mutation: bind w: to a wrong namespace
            _ => {
                let xml = base_xml
                    .replace(
                        "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
                        "http://example.invalid/ns",
                    )
                    .replace("<w:p>", "<x:p>")
                    .replace("</w:p>", "</x:p>");
                minimal_package(&xml)
            }
        };
        // Ok or Err are both fine — the contract is: never panic.
        let _ = nanooffice_ooxml::parse_docx(&bytes);
    }
}

// ---------- helpers ----------

fn para(doc: &Document, i: usize) -> &Paragraph {
    match &doc.body[i] {
        Block::Paragraph(p) => p,
        other => panic!("body[{i}] is not a paragraph: {other:?}"),
    }
}

/// `blocks` là block stream của một header/footer part (cùng shape với body).
fn block_para(blocks: &[Block], i: usize) -> &Paragraph {
    match &blocks[i] {
        Block::Paragraph(p) => p,
        other => panic!("blocks[{i}] is not a paragraph: {other:?}"),
    }
}

fn cell_text(doc: &Document, block: usize, row: usize, col: usize) -> String {
    match &doc.body[block] {
        Block::Table(t) => t.rows[row][col]
            .blocks
            .iter()
            .map(|b| match b {
                Block::Paragraph(p) => p.runs.iter().map(|r| r.text.as_str()).collect::<String>(),
                Block::Table(_) => String::new(),
            })
            .collect(),
        other => panic!("body[{block}] is not a table: {other:?}"),
    }
}

const OFFICE_REL: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument";
const RELS_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const W_MAIN: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

fn empty_document() -> String {
    concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#,
    )
    .to_string()
}

fn content_types_xml(extra: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="{W_MAIN}"/>{extra}
</Types>"#
    )
}

fn root_rels_xml() -> String {
    rels_xml(&[("rIdDoc", OFFICE_REL, "word/document.xml")])
}

/// styles + image relationship ids the malformed tests share
fn style_image_rels() -> String {
    format!(
        r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>{}"#,
        r#"<Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>"#,
    )
    .trim_end()
    .to_string()
}

fn rels_xml(rels: &[(&str, &str, &str)]) -> String {
    let body = rels
        .iter()
        .map(|(id, ty, target)| {
            format!(r#"<Relationship Id="{id}" Type="{ty}" Target="{target}"/>"#)
        })
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="{RELS_NS}">{body}</Relationships>"#
    )
}

fn doc_rels_xml(extra: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="{RELS_NS}">{extra}</Relationships>"#
    )
}

fn minimal_package(document_xml: &str) -> Vec<u8> {
    minimal_package_with(document_xml, "", &[])
}

fn minimal_package_with(document_xml: &str, doc_rels: &str, extra: &[(&str, String)]) -> Vec<u8> {
    let mut parts: Vec<(String, Vec<u8>)> = vec![
        (
            "[Content_Types].xml".into(),
            content_types_xml("").into_bytes(),
        ),
        ("_rels/.rels".into(), root_rels_xml().into_bytes()),
        (
            "word/_rels/document.xml.rels".into(),
            doc_rels_xml(doc_rels).into_bytes(),
        ),
        ("word/document.xml".into(), document_xml.as_bytes().to_vec()),
    ];
    parts.extend(
        extra
            .iter()
            .map(|(n, c)| (n.to_string(), c.as_bytes().to_vec())),
    );
    zip_bytes(&parts)
}

fn minimal_package_with_bytes(document_xml: &[u8]) -> Vec<u8> {
    let parts = vec![
        (
            "[Content_Types].xml".to_string(),
            content_types_xml("").into_bytes(),
        ),
        ("_rels/.rels".to_string(), root_rels_xml().into_bytes()),
        (
            "word/_rels/document.xml.rels".to_string(),
            doc_rels_xml("").into_bytes(),
        ),
        ("word/document.xml".to_string(), document_xml.to_vec()),
    ];
    zip_bytes(&parts)
}

fn zip_parts(parts: &[(&str, String)]) -> Vec<u8> {
    let owned = parts
        .iter()
        .map(|(n, c)| (n.to_string(), c.as_bytes().to_vec()))
        .collect::<Vec<_>>();
    zip_bytes(&owned)
}

/// In-memory STORED zip — chỉ cho test.
fn zip_bytes(parts: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, content) in parts {
        w.start_file(name.as_str(), zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(content).unwrap();
    }
    w.finish().unwrap().into_inner()
}
