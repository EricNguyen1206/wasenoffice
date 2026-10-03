#!/usr/bin/env python3
"""Generate the V1-corpus DOCX fixture (stdlib only) for T-V1 (A3/V1).

Usage: make_docx_v1.py [output.docx]
  (default: src-tauri/crates/nanooffice-ooxml/tests/fixtures/nanooffice-v1-fixture.docx)

Covers the render-side constructs of DD V01 §3.2 the S1 corpus does not
exercise, spec-derived from ECMA-376 / ISO 29500 (no upstream code involved):
w:rFonts (§17.3.2.26), w:br/w:tab incl. page/column types (§17.3.3.3/
§17.3.3.34), w:rStyle (§17.5.2.3), w:color (§17.3.2.6), w:spacing
(§17.3.2.33), w:ind (§17.3.1.12), explicit-off b/i (three-state ST_OnOff),
w:u val="none", w:docDefaults (§17.7.5.1) and the w:default paragraph style
(§17.7.2.4). tblGrid/pgSz are already covered by make_docx_full.py.
"""
import sys
import zipfile

W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"'


CONTENT_TYPES = """<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
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
  <Relationship Id="rIdS" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>
"""

STYLES = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles {W}>
  <w:docDefaults>
    <w:rPrDefault><w:rPr>
      <w:rFonts w:ascii="Carlito" w:hAnsi="Carlito" w:eastAsia="Noto Sans" w:cs="Mitra"/>
      <w:color w:val="333333"/>
      <w:sz w:val="20"/>
    </w:rPr></w:rPrDefault>
    <w:pPrDefault><w:pPr><w:jc w:val="left"/></w:pPr></w:pPrDefault>
  </w:docDefaults>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:pPr><w:ind w:left="720"/></w:pPr>
    <w:rPr><w:sz w:val="22"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Title2">
    <w:name w:val="title 2"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:jc w:val="center"/></w:pPr>
    <w:rPr><w:b/><w:color w:val="0000FF"/><w:spacing w:val="10"/></w:rPr>
  </w:style>
  <w:style w:type="character" w:styleId="EmphChar">
    <w:name w:val="Emphasis Char"/>
    <w:rPr><w:i/><w:color w:val="00AA00"/><w:u w:val="single"/></w:rPr>
  </w:style>
</w:styles>
"""


def document_xml() -> str:
    body = [
        # default paragraph style backs this paragraph (no pStyle)
        '<w:p><w:r><w:t>Đoạn không pStyle — kế thừa style w:default.</w:t></w:r></w:p>',
        # Title2 chain (basedOn Normal) + rStyle layer: direct color/spacing win
        '<w:p><w:pPr><w:pStyle w:val="Title2"/></w:pPr>'
        '<w:r><w:rPr><w:rStyle w:val="EmphChar"/><w:color w:val="FF0000"/>'
        '<w:spacing w:val="-4"/></w:rPr><w:t xml:space="preserve">Trực tiếp thắng rStyle: đỏ, hẹp. </w:t></w:r>'
        '<w:r><w:rPr><w:rStyle w:val="EmphChar"/></w:rPr><w:t>rStyle thắng pStyle: xanh lá.</w:t></w:r></w:p>',
        # explicit-off overrides the style chain (three-state)
        '<w:p><w:pPr><w:pStyle w:val="Title2"/></w:pPr>'
        '<w:r><w:rPr><w:b w:val="false"/><w:i w:val="0"/></w:rPr>'
        '<w:t>Tắt tường minh b/i của style Title2.</w:t></w:r></w:p>',
        # run rFonts — ascii wins over hAnsi/cs
        '<w:p><w:r><w:rPr><w:rFonts w:ascii="Caladea" w:hAnsi="Carlito" w:cs="Mitra"/></w:rPr>'
        '<w:t xml:space="preserve">Font riêng Caladea — ẽnghệ ộ lại là là.</w:t></w:r></w:p>',
        # breaks: tab, line, page, column (self-closing serialization)
        '<w:p><w:r><w:t>trước tab</w:t><w:tab/><w:t>sau tab</w:t>'
        '<w:br/><w:t>sau xuống dòng</w:t>'
        '<w:br w:type="page"/><w:t>sau ngắt trang</w:t>'
        '<w:br w:type="column"/><w:t>sau ngắt cột</w:t></w:r></w:p>',
        # underline explicit none over the EmphChar underline + direct ind
        '<w:p><w:pPr><w:ind w:left="1134" w:right="567" w:firstLine="227"/></w:pPr>'
        '<w:r><w:rPr><w:rStyle w:val="EmphChar"/><w:u w:val="none"/></w:rPr>'
        '<w:t>Gạch chân tắt tường minh, đoạn thụt dòng.</w:t></w:r></w:p>',
        # tblGrid widths (full fixture also carries them; keep one here so the
        # V1 corpus is self-contained)
        '<w:tbl><w:tblGrid><w:gridCol w:w="6000"/><w:gridCol w:w="3000"/></w:tblGrid>'
        '<w:tr><w:tc><w:p><w:r><w:t>Khởi</w:t></w:r></w:p></w:tc>'
        '<w:tc><w:p><w:r><w:t>RAM viewer</w:t></w:r></w:p></w:tc></w:tr></w:tbl>',
        '<w:sectPr><w:pgSz w:w="12240" w:h="15840" w:orient="portrait"/>'
        '<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr>',
    ]
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:document {W}><w:body>{"".join(body)}</w:body></w:document>'
    )


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else (
        "src-tauri/crates/nanooffice-ooxml/tests/fixtures/nanooffice-v1-fixture.docx"
    )
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        for part, content in {
            "[Content_Types].xml": CONTENT_TYPES,
            "_rels/.rels": ROOT_RELS,
            "word/_rels/document.xml.rels": DOC_RELS,
            "word/styles.xml": STYLES,
            "word/document.xml": document_xml(),
        }.items():
            # pinned timestamp → regenerate-and-cmp detects generator↔corpus drift
            zi = zipfile.ZipInfo(part, date_time=(2026, 10, 3, 0, 0, 0))
            zi.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(zi, content)
    print(out)


if __name__ == "__main__":
    main()
