#!/usr/bin/env python3
"""Generate the quick-view LAYOUT fixture (stdlib only) — T-V2 (A3/V2),
extended by T-V3 (A3/V3) with two body images.

Usage: make_docx_layout.py [output.docx]
  (default: /tmp/wasenoffice-quickview-layout.docx)

Built for the layout gate's assertions (harness/scripts/smoke-quick-view.sh):
uniform runs — every run inherits the same font + size from w:docDefaults
(Carlito, bundled by T-A12, 22 half-points = 11pt) — so every visual line
must advance by the SAME OS/2-derived line height, which the gate recomputes
from the metrics the viewer logs. Content: a short line, a long filler
paragraph (proves greedy wrap against the pgSz column), a tab paragraph
(default tab stops) and a medium paragraph. A4 pgSz at body level feeds the
wrap column. T-V3 appends two image paragraphs AFTER all text (multi-pixel
PNGs, saturated colors — never mistaken for the writer's 1×1 white
placeholder), each a distinct wp:extent so the gate's EMU→px assert cannot
pass on a swapped/constant size; text layout data stays image-free.
"""
import sys
import zipfile

from docxgen_common import (
    FIDELITY_CT_OVERRIDES,
    FIDELITY_DOC_RELS_EXTRA,
    FIDELITY_LINK_PARA,
    FIDELITY_PARTS,
    FIDELITY_ROOT_RELS_EXTRA,
    FIDELITY_SECT_PARA,
)
from docxgen_common import png
from make_docx_media import drawing

W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"'
R = 'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"'
ROOT = f"{W} {R}"

CONTENT_TYPES = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>
"""

ROOT_RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
"""

DOC_RELS = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
  <Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
  <Relationship Id="rIdImg2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image2.png"/>
</Relationships>
"""

STYLES = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles {W}>
  <w:docDefaults>
    <w:rPrDefault><w:rPr><w:rFonts w:ascii="Carlito" w:hAnsi="Carlito"/><w:sz w:val="22"/></w:rPr></w:rPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style>
</w:styles>
"""

# Saturated quads — the sampled rect of a REAL draw is >50% non-white; the
# writer's 1×1 white placeholder scaled up would ink ≈0.
IMG1 = png([[(0xFF, 0x00, 0x4C), (0x00, 0xB4, 0x2B)], [(0x41, 0x1E, 0xF0), (0xFF, 0xD5, 0x00)]])
IMG2 = png([[(0x00, 0x91, 0xAD), (0xE1, 0x71, 0x00), (0x5B, 0x10, 0x79)], [(0x00, 0x00, 0x00), (0x94, 0xE0, 0x38), (0xB9, 0x1E, 0x1E)]])
# wp:extent in EMUs: 952500×476250 → 100×50 px, 476250×285750 → 50×30 px
# (914400 EMU/in ÷ 96 dpi = 9525 EMU/px).
IMG1_EMU = (952500, 476250)
IMG2_EMU = (476250, 285750)

FILLER = (
    "the quick brown fox jumps over a lazy dog near the river bank every "
    "single morning before sunrise while the village slowly wakes up "
) * 8

MEDIUM = (
    "WasenOffice renders this paragraph through its own layout engine: "
    "greedy line breaking against the text column derived from the A4 "
    "page size, advances measured by the platform text stack, and line "
    "heights taken from the bundled font metrics."
)

TAB_PARA = (
    '<w:p><w:r><w:t xml:space="preserve">alpha</w:t></w:r>'
    '<w:r><w:tab/><w:t xml:space="preserve">beta</w:t></w:r>'
    '<w:r><w:tab/><w:t xml:space="preserve">gamma delta epsilon zeta</w:t></w:r></w:p>'
)


def p(text: str) -> str:
    return f'<w:p><w:r><w:t xml:space="preserve">{text}</w:t></w:r></w:p>'


# T-A30 gate-15 flag (`table` argv): one 1×2 table appended before the
# sectPr — the default output stays byte-identical (gate 12 + parity share
# this generator without the flag).
TABLE = (
    '<w:tbl><w:tr>'
    '<w:tc><w:p><w:r><w:t xml:space="preserve">Alpha cell</w:t></w:r></w:p></w:tc>'
    '<w:tc><w:p><w:r><w:t xml:space="preserve">Beta cell</w:t></w:r></w:p></w:tc>'
    '</w:tr></w:tbl>'
)

# T-A32 gate-15 flag (`tblstyle` argv): same table WITH tblPr/tcPr styling —
# fires the `table styling` warn (`detect_save_losses`, DD L01 §5.7). Default
# output stays byte-identical.
TABLE_STYLED = (
    '<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr><w:tr>'
    '<w:tc><w:tcPr><w:tcW w:w="4780" w:type="dxa"/></w:tcPr>'
    '<w:p><w:r><w:t xml:space="preserve">Alpha cell</w:t></w:r></w:p></w:tc>'
    '<w:tc><w:tcPr><w:tcW w:w="4780" w:type="dxa"/></w:tcPr>'
    '<w:p><w:r><w:t xml:space="preserve">Beta cell</w:t></w:r></w:p></w:tc>'
    '</w:tr></w:tbl>'
)


def document_xml(with_table: bool = False, with_fidelity: bool = False,
                 with_tracked: bool = False, with_tblstyle: bool = False):
    title = '<w:r><w:t xml:space="preserve">Quick view layout fixture</w:t></w:r>'
    if with_tracked:
        title = f'<w:ins w:id="1" w:date="2026-10-04T00:00:00Z" w:author="WasenOffice">{title}</w:ins>'
    body = [f"<w:p>{title}</w:p>", p(FILLER), TAB_PARA]
    if with_fidelity:
        body += [FIDELITY_LINK_PARA, FIDELITY_SECT_PARA]
    body += [
        p(MEDIUM),
        # images AFTER all text: the text-line baselines stay contiguous, so
        # the gate's per-delta assert keeps holding and the first image's top
        # is exactly last-baseline + last-line-height
        f'<w:p>{drawing("rIdImg1", *IMG1_EMU, 1)}</w:p>',
        f'<w:p>{drawing("rIdImg2", *IMG2_EMU, 2)}</w:p>',
    ]
    if with_tblstyle:
        body.append(TABLE_STYLED)
    elif with_table:
        body.append(TABLE)
    body += [
        '<w:sectPr><w:pgSz w:w="11906" w:h="16838"/>'
        '<w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134"/></w:sectPr>',
    ]
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:document {ROOT}><w:body>{"".join(body)}</w:body></w:document>'
    )


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "/tmp/wasenoffice-quickview-layout.docx"
    flags = set(sys.argv[2:])
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        content_types, root_rels, doc_rels = CONTENT_TYPES, ROOT_RELS, DOC_RELS
        if "fidelity" in flags:
            content_types = content_types.replace("</Types>", f"  {FIDELITY_CT_OVERRIDES}\n</Types>")
            root_rels = root_rels.replace("</Relationships>", f"  {FIDELITY_ROOT_RELS_EXTRA}\n</Relationships>")
            doc_rels = doc_rels.replace("</Relationships>", f"  {FIDELITY_DOC_RELS_EXTRA}\n</Relationships>")
        z.writestr("[Content_Types].xml", content_types)
        z.writestr("_rels/.rels", root_rels)
        z.writestr("word/_rels/document.xml.rels", doc_rels)
        z.writestr("word/styles.xml", STYLES)
        z.writestr("word/media/image1.png", IMG1)
        z.writestr("word/media/image2.png", IMG2)
        if "fidelity" in flags:
            for name, xml in FIDELITY_PARTS.items():
                z.writestr(name, xml)
        z.writestr("word/document.xml", document_xml("table" in flags, "fidelity" in flags, "tracked" in flags, "tblstyle" in flags))
    print(out)


if __name__ == "__main__":
    main()
