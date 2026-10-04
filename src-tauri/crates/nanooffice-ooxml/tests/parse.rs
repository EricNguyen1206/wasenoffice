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
const V1_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/nanooffice-v1-fixture.docx"
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
    assert_eq!(p.runs[0].bold, None);

    let p = para(&doc, 2);
    assert_eq!(p.style, None);
    assert_eq!(p.runs.len(), 1);
    assert_eq!(p.runs[0].bold, Some(true), "whole-sentence bold run");
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

    // run-level bold=false overrides the style chain (three-state: explicit
    // off is Some(false), distinguishable from not-set None)
    let p = para(&doc, 1);
    assert_eq!(p.style.as_deref(), Some("Heading1"));
    assert_eq!(p.runs[0].bold, Some(false), "run override w:b w:val=false");

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
    assert_eq!(runs[0].bold, Some(true));
    assert_eq!(runs[0].italic, None);
    assert_eq!(runs[1].text, "nghiêng ");
    assert_eq!(runs[1].italic, Some(true));
    assert_eq!(runs[1].bold, None);
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
    assert_eq!(
        runs[0].bold,
        Some(false),
        "explicit w:b w:val=false is three-state"
    );
    assert_eq!(runs[0].italic, None);
    assert_eq!(runs[1].text, "italic");
    assert_eq!(runs[1].italic, Some(true));
    assert_eq!(runs[1].bold, None);
    assert_eq!(runs[2].text, "bold only");
    assert_eq!(runs[2].bold, Some(true));
    assert_eq!(runs[2].italic, Some(false), "w:val=0 must turn w:i off");

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
    assert_eq!(p.runs[0].bold, None, "pPr's rPr styles the mark, not runs");

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

/// `mc:AlternateContent` run content (bắt được qua parity harness T-A23: x2t
/// round-trip bọc `w:drawing` trong `mc:Choice` + VML `mc:Fallback` — trước
/// fix image bị mất). Branch được chọn phải surface image + text như run
/// content thường; Fallback bị bỏ qua đúng quy tắc MCE.
#[test]
fn alternate_content_choice_surfaces_image() {
    // cấu trúc mirror output thật của x2t (ElementContent như engine ghi):
    // inline xmlns khai báo tại chỗ + wp:docPr/wp:cNvGraphicFramePr/pic:nvPr
    // + Choice Requires="wps" + Fallback VML (w:pict).
    let xml = concat!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:body>"#,
        r#"<w:p><w:r><w:t xml:space="preserve">trước ảnh </w:t>"#,
        r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006">"#,
        r#"<mc:Choice Requires="wps">"#,
        r#"<w:drawing><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" distT="0" distB="0" distL="0" distR="0">"#,
        r#"<wp:extent cx="9525" cy="19050"/><wp:docPr id="1" name=""/><wp:cNvGraphicFramePr/>"#,
        r#"<a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">"#,
        r#"<a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
        r#"<pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">"#,
        r#"<pic:nvPicPr><pic:cNvPr id="1" name="pixel"/><pic:cNvPicPr/><pic:nvPr/></pic:nvPicPr>"#,
        r#"<pic:blipFill><a:blip r:embed="rIdImg"></a:blip><a:stretch><a:fillRect/></a:stretch></pic:blipFill>"#,
        r#"<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="9525" cy="19050"/></a:xfrm>"#,
        r#"<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>"#,
        r#"</a:graphicData></a:graphic></wp:inline></w:drawing>"#,
        r#"</mc:Choice>"#,
        r#"<mc:Fallback><w:pict><v:shape xmlns:v="urn:schemas-microsoft-com:vml" style="width:1pt;height:2pt"><v:imagedata r:id="rIdGhost"/></v:shape></w:pict></mc:Fallback>"#,
        r#"</mc:AlternateContent>"#,
        r#"<w:t xml:space="preserve"> sau ảnh</w:t></w:r></w:p>"#,
        r#"</w:body></w:document>"#,
    );
    let rels = r#"<Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>"#;
    let doc = nanooffice_ooxml::parse_docx(&minimal_package_with(&xml, rels, &[])).unwrap();

    let p = para(&doc, 0);
    assert_eq!(p.runs.len(), 1, "AlternateContent KHÔNG tách run");
    assert_eq!(p.runs[0].text, "trước ảnh  sau ảnh");
    let img = &p.runs[0].images[0];
    assert_eq!(
        img.rel_id, "rIdImg",
        "Choice thắng — Fallback rIdGhost bị bỏ"
    );
    assert_eq!(img.part, "word/media/image1.png");
    assert_eq!((img.cx_emu, img.cy_emu), (9525, 19050));

    // round-trip: writer ghi plain w:drawing, re-parse vẫn đủ image
    let out = nanooffice_ooxml::write_docx(&doc, None).unwrap();
    let reparsed = nanooffice_ooxml::parse_docx(&out).unwrap();
    assert_eq!(doc, reparsed, "round-trip bất biến qua AlternateContent");
}

/// V1 render constructs (DD V01 §3.2): rFonts/rStyle/color/spacing/ind/
/// tblGrid/pgSz/docDefaults/w:default/3-state/br+tab — PARSE raw + RESOLVED
/// values đúng thứ bậc §17.7.2 (direct → rStyle → pStyle/default-para-style →
/// docDefaults), và round-trip bất biến qua writer đối xứng.
#[test]
fn v1_render_constructs_parse_resolve_and_round_trip() {
    let styles = concat!(
        r#"<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
        r#"<w:docDefaults>"#,
        r#"<w:rPrDefault><w:rPr><w:rFonts w:ascii="Carlito" w:hAnsi="Carlito" w:eastAsia="Noto" w:cs="Mitra"/><w:color w:val="444444"/><w:sz w:val="20"/></w:rPr></w:rPrDefault>"#,
        r#"<w:pPrDefault><w:pPr><w:jc w:val="left"/></w:pPr></w:pPrDefault>"#,
        r#"</w:docDefaults>"#,
        r#"<w:style w:type="paragraph" w:styleId="Normal" w:default="1"><w:name w:val="Normal"/>"#,
        r#"<w:pPr><w:ind w:left="720"/></w:pPr><w:rPr><w:sz w:val="22"/></w:rPr></w:style>"#,
        r#"<w:style w:type="paragraph" w:styleId="Head"><w:name w:val="head"/><w:basedOn w:val="Normal"/>"#,
        r#"<w:rPr><w:b/><w:color w:val="0000FF"/><w:spacing w:val="10"/></w:rPr></w:style>"#,
        r#"<w:style w:type="character" w:styleId="Emph"><w:name w:val="emph"/>"#,
        r#"<w:rPr><w:i/><w:color w:val="00FF00"/></w:rPr></w:style>"#,
        r#"</w:styles>"#,
    );
    let doc_rels = r#"<Relationship Id="rIdS" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#;
    let xml = concat!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
        // p0: no pStyle → the w:default paragraph style backs it
        r#"<w:p><w:r><w:t>plain</w:t></w:r></w:p>"#,
        // p1: direct color/spacing beat rStyle; rStyle beats pStyle
        r#"<w:p><w:pPr><w:pStyle w:val="Head"/></w:pPr>"#,
        r#"<w:r><w:rPr><w:rStyle w:val="Emph"/><w:color w:val="FF0000"/><w:spacing w:val="-5"/></w:rPr><w:t>direct</w:t></w:r>"#,
        r#"<w:r><w:rPr><w:rStyle w:val="Emph"/></w:rPr><w:t>rs</w:t></w:r></w:p>"#,
        // p2: explicit-off overrides the style chain + direct ind overrides the style's
        r#"<w:p><w:pPr><w:pStyle w:val="Head"/><w:ind w:left="100" w:right="200" w:firstLine="50"/></w:pPr>"#,
        r#"<w:r><w:rPr><w:b w:val="false"/><w:i w:val="0"/></w:rPr><w:t>off</w:t></w:r></w:p>"#,
        // p3: run rFonts — ascii wins over hAnsi/eastAsia/cs
        r#"<w:p><w:r><w:rPr><w:rFonts w:ascii="Asana" w:hAnsi="Carlito" w:cs="Mitra"/></w:rPr><w:t>font</w:t></w:r></w:p>"#,
        // p4: br/tab (self-closing AND Start+End serializations) + page/column breaks
        r#"<w:p><w:r><w:t>a</w:t><w:tab/><w:t>b</w:t><w:br/><w:t>c</w:t><w:br w:type="page"/><w:t>d</w:t><w:br w:type="column"/><w:t>e</w:t></w:r></w:p>"#,
        r#"<w:p><w:r><w:t>x</w:t><w:tab></w:tab><w:br></w:br><w:t>y</w:t></w:r></w:p>"#,
        // p5: w:u val="none" — explicit no underline collapses to None when resolved
        r#"<w:p><w:r><w:rPr><w:u w:val="none"/></w:rPr><w:t>nu</w:t></w:r></w:p>"#,
        // tblGrid: widths + one auto column
        r#"<w:tbl><w:tblGrid><w:gridCol w:w="4000"/><w:gridCol/><w:gridCol w:w="2000"/></w:tblGrid>"#,
        r#"<w:tr><w:tc><w:p><w:r><w:t>c</w:t></w:r></w:p></w:tc><w:tc/></w:tr></w:tbl>"#,
        // final section page size
        r#"<w:sectPr><w:pgSz w:w="11906" w:h="16838" w:orient="portrait"/></w:sectPr>"#,
        r#"</w:body></w:document>"#,
    );
    let doc = nanooffice_ooxml::parse_docx(&minimal_package_with(
        xml,
        doc_rels,
        &[("word/styles.xml", styles.to_string())],
    ))
    .unwrap();

    // ---- raw parse ----
    assert_eq!(doc.page.as_ref().unwrap().w, Some(11906));
    assert_eq!(doc.page.as_ref().unwrap().h, Some(16838));
    assert_eq!(
        doc.page.as_ref().unwrap().orient.as_deref(),
        Some("portrait")
    );
    let dd = doc.doc_defaults.as_ref().unwrap();
    assert_eq!(dd.size, Some(20));
    assert_eq!(dd.color.as_deref(), Some("444444"));
    assert_eq!(dd.align.as_deref(), Some("left"));
    assert_eq!(
        dd.rfonts.as_ref().unwrap().ascii.as_deref(),
        Some("Carlito")
    );
    assert!(doc.styles["Normal"].default, "w:default=\"1\" parsed");
    assert_eq!(doc.styles["Normal"].ind.as_ref().unwrap().left, Some(720));
    assert_eq!(doc.styles["Head"].spacing, Some(10));
    assert_eq!(doc.styles["Head"].color.as_deref(), Some("0000FF"));
    assert_eq!(doc.styles["Emph"].italic, Some(true));

    let p = para(&doc, 4);
    assert_eq!(
        p.runs[0].text, "a\tb\nc\u{c}d\u{b}e",
        "tab + br + page + column"
    );
    assert_eq!(
        para(&doc, 5).runs[0].text,
        "x\t\ny",
        "Start+End serialization"
    );
    let tbl = match &doc.body[7] {
        Block::Table(t) => t,
        other => panic!("expected table, got {other:?}"),
    };
    assert_eq!(
        tbl.grid,
        vec![Some(4000), None, Some(2000)],
        "tblGrid widths + auto column"
    );

    // ---- resolved (§17.7.2: direct → rStyle → pStyle → docDefaults) ----
    let r = nanooffice_ooxml::resolve(&doc);
    let rp = |i: usize| match &r.body[i] {
        nanooffice_ooxml::RBlock::Paragraph(p) => p,
        other => panic!("body[{i}] not a paragraph: {other:?}"),
    };

    // p0: w:default paragraph style + docDefaults fill everything in
    let run = &rp(0).runs[0];
    assert_eq!(run.size, Some(22), "size from the default paragraph style");
    assert_eq!(
        run.font.as_deref(),
        Some("Carlito"),
        "font from docDefaults"
    );
    assert_eq!(
        run.color.as_deref(),
        Some("444444"),
        "color from docDefaults"
    );
    assert!(!run.bold && !run.italic);
    assert_eq!(
        rp(0).ind.as_ref().unwrap().left,
        Some(720),
        "ind from the default paragraph style"
    );
    assert_eq!(
        rp(0).align.as_deref(),
        Some("left"),
        "align from docDefaults pPr"
    );

    // p1 run 0: direct beats rStyle beats pStyle; chain fills size via Head→Normal
    let run = &rp(1).runs[0];
    assert_eq!(run.color.as_deref(), Some("FF0000"), "direct color wins");
    assert_eq!(run.spacing, Some(-5), "direct spacing wins (condensed)");
    assert!(run.italic, "italic from the rStyle");
    assert!(run.bold, "bold from the paragraph style");
    assert_eq!(
        run.size,
        Some(22),
        "size through the Head→Normal basedOn chain"
    );
    // p1 run 1: rStyle color (00FF00) must beat the paragraph style color (0000FF)
    let run = &rp(1).runs[1];
    assert_eq!(
        run.color.as_deref(),
        Some("00FF00"),
        "rStyle wins over pStyle"
    );
    assert!(run.italic);

    // p2: explicit-off beats the style chain; direct ind beats the style's
    let run = &rp(2).runs[0];
    assert!(!run.bold, "w:b w:val=false overrides the bold style");
    assert!(!run.italic, "w:i w:val=0 overrides");
    assert_eq!(
        run.color.as_deref(),
        Some("0000FF"),
        "color still from pStyle"
    );
    assert_eq!(
        rp(2).ind.as_ref().unwrap(),
        &nanooffice_ooxml::Ind {
            left: Some(100),
            right: Some(200),
            first_line: Some(50),
            hanging: None
        },
        "direct w:ind wins over the style's"
    );

    // p3: ascii family first
    let run = &rp(3).runs[0];
    assert_eq!(
        run.font.as_deref(),
        Some("Asana"),
        "ascii wins over hAnsi/cs"
    );

    // p6: w:u val="none" collapses to None
    assert_eq!(
        rp(6).runs[0].underline,
        None,
        "underline none → nothing to draw"
    );

    // ---- round-trip through the symmetric writer ----
    let out = nanooffice_ooxml::write_docx(&doc, None).unwrap();
    let reparsed = nanooffice_ooxml::parse_docx(&out).unwrap();
    assert_eq!(doc, reparsed, "round-trip bất biến qua mọi construct V1");
}

/// Corpus V1 (sinh bằng `harness/fixtures/make_docx_v1.py`): construct
/// render-side DD V01 §3.2 trên file thật — resolved values đúng chuỗi
/// §17.7.2 (default-para-style / rStyle / docDefaults) + page + grid.
#[test]
fn v1_fixture_parses_resolved_values() {
    let doc = nanooffice_ooxml::parse_docx_file(V1_FIXTURE).unwrap();

    // docDefaults + default paragraph style surface raw
    let dd = doc.doc_defaults.as_ref().unwrap();
    assert_eq!(dd.size, Some(20));
    assert_eq!(
        dd.rfonts.as_ref().unwrap().ascii.as_deref(),
        Some("Carlito")
    );
    assert!(doc.styles["Normal"].default);
    assert_eq!(doc.page.as_ref().unwrap().w, Some(12240));

    let r = nanooffice_ooxml::resolve(&doc);
    let rp = |i: usize| match &r.body[i] {
        nanooffice_ooxml::RBlock::Paragraph(p) => p,
        other => panic!("body[{i}] not a paragraph: {other:?}"),
    };

    // p0: w:default style (ind 720, sz 22) + docDefaults (font, color 333333)
    let run = &rp(0).runs[0];
    assert_eq!(run.font.as_deref(), Some("Carlito"));
    assert_eq!(run.color.as_deref(), Some("333333"));
    assert_eq!(run.size, Some(22));
    assert_eq!(rp(0).ind.as_ref().unwrap().left, Some(720));

    // p1 run 0: direct color/spacing win over the rStyle
    let run = &rp(1).runs[0];
    assert_eq!(run.color.as_deref(), Some("FF0000"));
    assert_eq!(run.spacing, Some(-4));
    assert!(run.italic, "italic via the rStyle layer");
    assert_eq!(
        run.underline.as_deref(),
        Some("single"),
        "underline via rStyle"
    );
    assert!(run.bold, "bold via the Title2 style");
    // p1 run 1: rStyle color (00AA00) beats pStyle color (0000FF)
    assert_eq!(rp(1).runs[1].color.as_deref(), Some("00AA00"));

    // p2: explicit-off wins over Title2's bold
    let run = &rp(2).runs[0];
    assert!(!run.bold);
    assert!(!run.italic);

    // p3: run rFonts ascii wins
    assert_eq!(rp(3).runs[0].font.as_deref(), Some("Caladea"));

    // p4: break controls survive into the resolved text
    assert_eq!(
        rp(4).runs[0].text,
        "trước tab\tsau tab\nsau xuống dòng\u{c}sau ngắt trang\u{b}sau ngắt cột"
    );

    // p5: u val="none" collapses; direct ind wins over the style's
    assert_eq!(rp(5).runs[0].underline, None);
    let ind = rp(5).ind.as_ref().unwrap();
    assert_eq!(
        (ind.left, ind.right, ind.first_line),
        (Some(1134), Some(567), Some(227))
    );

    // table grid (resolved table keeps the row structure)
    let tbl = match &r.body[6] {
        nanooffice_ooxml::RBlock::Table(t) => t,
        other => panic!("expected table, got {other:?}"),
    };
    assert_eq!(tbl.rows.len(), 1);

    // round-trip: writer emits every V1 construct, model invariant
    let out = nanooffice_ooxml::write_docx(&doc, None).unwrap();
    assert_eq!(doc, nanooffice_ooxml::parse_docx(&out).unwrap());
}

// ── T-A31 (DD L01 §5.6): hyperlink + sectPr + passthrough + detect ────────

const FIDELITY_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/nanooffice-fidelity-fixture.docx"
);

/// Fidelity corpus: hyperlink groups (external `r:id` + internal `w:anchor`)
/// flatten onto their runs with the relationship resolved; the mid-document
/// `w:pPr/w:sectPr` and the final one keep pgSz+pgMar; the file carries
/// nothing `detect_save_losses` would warn about.
#[test]
fn fidelity_corpus_parses_hyperlinks_and_sections() {
    let doc = nanooffice_ooxml::parse_docx_file(FIDELITY_FIXTURE).unwrap();
    let link_para = para(&doc, 1);
    assert_eq!(
        link_para.runs.len(),
        4,
        "outside + link + middle + anchor runs"
    );
    let link = link_para.runs[1].link.as_ref().unwrap();
    assert_eq!(link.rel_id.as_deref(), Some("rIdLink"));
    assert_eq!(
        link.target.as_deref(),
        Some("https://example.com/nanooffice-fidelity")
    );
    assert!(link.external);
    assert_eq!(link_para.runs[1].text, "NanoOffice fidelity link");
    let anchor = link_para.runs[3].link.as_ref().unwrap();
    assert_eq!(anchor.anchor.as_deref(), Some("sect-anchor"));
    assert!(anchor.rel_id.is_none() && anchor.target.is_none());
    assert!(
        link_para.runs[0].link.is_none(),
        "runs outside the group stay link-free"
    );

    let sect = para(&doc, 2).sect.as_ref().unwrap();
    assert_eq!(sect.page.as_ref().unwrap().w, Some(11906));
    let mar = sect.mar.as_ref().unwrap();
    assert_eq!(mar.top, Some(1134));
    assert_eq!(mar.header, Some(720));
    assert_eq!(mar.gutter, Some(0));
    assert!(
        para(&doc, 3).sect.is_none(),
        "sections only end AT their paragraph"
    );
    let fin = doc.page_mar.as_ref().unwrap();
    assert_eq!(fin.top, Some(1134));
    assert_eq!(doc.page.as_ref().unwrap().w, Some(11906));

    assert!(
        nanooffice_ooxml::detect_save_losses(&std::fs::read(FIDELITY_FIXTURE).unwrap()).is_empty(),
        "the fidelity fixture must not trigger false warnings"
    );
}

/// Detect (refuse-warn, DD §5.6d + §5.7): tracked changes / fields / footnotes /
/// tblPr/tcPr table styling / sectPr children beyond pgSz+pgMar are flagged; a
/// pgSz/pgMar-only sectPr — mid-document or final — and `<w:tblPrChange>`/
/// `<w:tblPrEx>` lookalikes stay silent.
#[test]
fn detect_save_losses_flags_and_spares() {
    let clean = minimal_package(&empty_document());
    assert!(nanooffice_ooxml::detect_save_losses(&clean).is_empty());

    let dirty = concat!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
        r#"<w:p><w:ins w:id="1" w:author="A"><w:r><w:t>tracked</w:t></w:r></w:ins></w:p>"#,
        r#"<w:p><w:fldChar w:fldCharType="begin"/></w:p>"#,
        r#"<w:p><w:footnoteReference w:id="2"/></w:p>"#,
        r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tr><w:tc><w:tcPr><w:tcW w:w="4780" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        r#"<w:p><w:pPr><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:cols w:num="2"/></w:sectPr></w:pPr></w:p>"#,
        r#"</w:body></w:document>"#,
    );
    let losses = nanooffice_ooxml::detect_save_losses(&minimal_package(dirty));
    assert!(
        losses.contains(&"tracked changes".to_string()),
        "{losses:?}"
    );
    assert!(losses.contains(&"fields".to_string()), "{losses:?}");
    assert!(losses.contains(&"footnotes".to_string()), "{losses:?}");
    assert!(losses.contains(&"table styling".to_string()), "{losses:?}");
    assert!(
        losses.contains(&"section properties beyond page size/margins".to_string()),
        "{losses:?}"
    );

    let plain_sect = concat!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
        r#"<w:p><w:r><w:t>a</w:t></w:r></w:p>"#,
        r#"<w:p><w:pPr><w:sectPr><w:pgSz w:w="11906" w:h="16838"/>"#,
        r#"<w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134"/></w:sectPr></w:pPr></w:p>"#,
        r#"</w:body></w:document>"#,
    );
    assert!(nanooffice_ooxml::detect_save_losses(&minimal_package(plain_sect)).is_empty());
    // <w:insideH>/<w:delText> must NOT trip the tracked-change scan
    let lookalikes = concat!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
        r#"<w:tbl><w:tr><w:tc><w:p><w:r><w:delText>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        r#"</w:body></w:document>"#,
    );
    assert!(nanooffice_ooxml::detect_save_losses(&minimal_package(lookalikes)).is_empty());
    // <w:tblPrChange>/<w:tblPrEx> must NOT trip the table-styling scan
    // (exact tag boundary — T-A32, DD L01 §5.7)
    let tblprop_lookalikes = concat!(
        r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>"#,
        r#"<w:tbl><w:tr><w:tc><w:tcPrChange><w:tcPr/></w:tcPrChange><w:p><w:r><w:t>x</w:t></w:r></w:p></w:tc></w:tr></w:tbl>"#,
        r#"</w:body></w:document>"#,
    );
    assert!(nanooffice_ooxml::detect_save_losses(&minimal_package(tblprop_lookalikes)).is_empty());
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
