#!/usr/bin/env python3
"""Generate the full-corpus DOCX fixture (stdlib only) for T-A21 (A2/S1).

Usage: make_docx_full.py [output.docx]
  (default: src-tauri/crates/wasenoffice-ooxml/tests/fixtures/wasenoffice-full-fixture.docx)

Covers every S1 construct, spec-derived from ECMA-376 / ISO 29500 (no upstream
code involved): paragraph props (pStyle/jc/numPr), run props (b/i/u/sz),
table 2x2, numbering (decimal + lowerLetter + bullet), embedded 1x1 PNG via
wp:/a:/pic: with relationship, styles.xml basedOn chain (Heading1 -> Normal,
run bold=false override), header + footer, Vietnamese diacritics.
"""
import sys
import zipfile

from docxgen_common import png_1x1

W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"'
R = 'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"'
WP = 'xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing"'
A = 'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"'
PIC = 'xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"'
ROOT = f"{W} {R} {WP} {A} {PIC}"


CONTENT_TYPES = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Default Extension="png" ContentType="image/png"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
  <Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/>
  <Override PartName="/word/header1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml"/>
  <Override PartName="/word/footer1.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml"/>
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
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/>
  <Relationship Id="rIdImg" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>
  <Relationship Id="rIdHead" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/header" Target="header1.xml"/>
  <Relationship Id="rIdFoot" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer" Target="footer1.xml"/>
</Relationships>
"""

STYLES = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles {W}>
  <w:style w:type="paragraph" w:default="1" w:styleId="Normal">
    <w:name w:val="Normal"/>
    <w:qFormat/>
    <w:rPr><w:sz w:val="22"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="32"/></w:rPr>
  </w:style>
  <w:style w:type="table" w:styleId="TableNormal">
    <w:name w:val="Normal Table"/>
  </w:style>
  <w:style w:type="character" w:styleId="EmphasisChar">
    <w:name w:val="Emphasis Char"/>
    <w:rPr><w:i/></w:rPr>
  </w:style>
</w:styles>
"""

NUMBERING = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:numbering {W}>
  <w:abstractNum w:abstractNumId="0">
    <w:multiLevelType w:val="hybridMultilevel"/>
    <w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/></w:lvl>
    <w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%2)"/></w:lvl>
  </w:abstractNum>
  <w:abstractNum w:abstractNumId="1">
    <w:multiLevelType w:val="hybridMultilevel"/>
    <w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/></w:lvl>
  </w:abstractNum>
  <w:num w:numId="1"><w:abstractNumId w:val="0"/></w:num>
  <w:num w:numId="2"><w:abstractNumId w:val="1"/></w:num>
</w:numbering>
"""


def drawing() -> str:
    # wp:inline extent 9525 EMU = 1px @96dpi; a:blip r:embed -> relationship
    return (
        '<w:r><w:drawing>'
        '<wp:inline><wp:extent cx="9525" cy="9525"/>'
        f'<a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture">'
        f'<pic:pic><pic:nvPicPr><pic:cNvPr id="1" name="pixel"/><pic:cNvPicPr/></pic:nvPicPr>'
        '<pic:blipFill><a:blip r:embed="rIdImg"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>'
        '<pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="9525" cy="9525"/></a:xfrm>'
        '<a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr>'
        '</pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r>'
    )


def document_xml() -> str:
    body = [
        '<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr>'
        '<w:r><w:t xml:space="preserve">Giới thiệu WasenOffice</w:t></w:r></w:p>',
        # run-level bold=false overrides the Heading1 style chain
        '<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr>'
        '<w:r><w:rPr><w:b w:val="false"/></w:rPr>'
        '<w:t>Run override tắt bold của style Heading1.</w:t></w:r></w:p>',
        '<w:p><w:pPr><w:jc w:val="center"/></w:pPr>'
        '<w:r><w:t>Căn giữa.</w:t></w:r></w:p>',
        '<w:p>'
        '<w:r><w:rPr><w:b/></w:rPr><w:t xml:space="preserve">bold </w:t></w:r>'
        '<w:r><w:rPr><w:i/></w:rPr><w:t xml:space="preserve">nghiêng </w:t></w:r>'
        '<w:r><w:rPr><w:u w:val="single"/></w:rPr><w:t xml:space="preserve">gạch chân </w:t></w:r>'
        '<w:r><w:rPr><w:sz w:val="28"/></w:rPr><w:t xml:space="preserve">cỡ 14pt </w:t></w:r>'
        '<w:r><w:t xml:space="preserve">thường</w:t></w:r>'
        '</w:p>',
        '<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr>'
        '<w:r><w:t>Gạch đầu dòng thứ nhất</w:t></w:r></w:p>',
        '<w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr>'
        '<w:r><w:t>Mức con</w:t></w:r></w:p>',
        '<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="2"/></w:numPr></w:pPr>'
        '<w:r><w:t>Bullet tiếng Việt</w:t></w:r></w:p>',
        '<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr>'
        '<w:tblGrid><w:gridCol w:w="4366"/><w:gridCol w:w="4366"/></w:tblGrid>'
        '<w:tr><w:tc><w:tcPr><w:tcW w:w="4366" w:type="dxa"/></w:tcPr>'
        '<w:p><w:r><w:t>Tên</w:t></w:r></w:p></w:tc>'
        '<w:tc><w:tcPr><w:tcW w:w="4366" w:type="dxa"/></w:tcPr>'
        '<w:p><w:r><w:t>Giá trị</w:t></w:r></w:p></w:tc></w:tr>'
        '<w:tr><w:tc><w:tcPr><w:tcW w:w="4366" w:type="dxa"/></w:tcPr>'
        '<w:p><w:r><w:t>RAM</w:t></w:r></w:p></w:tc>'
        '<w:tc><w:tcPr><w:tcW w:w="4366" w:type="dxa"/></w:tcPr>'
        '<w:p><w:r><w:t>561 MB</w:t></w:r></w:p></w:tc></w:tr>'
        '</w:tbl>',
        f'<w:p>{drawing()}</w:p>',
        '<w:p><w:r><w:t xml:space="preserve">Đoạn kết thúc — ẽnghệ ộ lại là là, '
        'kiểm tra font fallback.</w:t></w:r></w:p>',
        '<w:sectPr><w:pgSz w:w="11906" w:h="16838"/>'
        '<w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134"/>'
        '<w:headerReference w:type="default" r:id="rIdHead"/>'
        '<w:footerReference w:type="default" r:id="rIdFoot"/>'
        '</w:sectPr>',
    ]
    return (
        '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
        f'<w:document {ROOT}><w:body>{"".join(body)}</w:body></w:document>'
    )


HEADER1 = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:hdr {W}><w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:t>WasenOffice — văn bản mẫu</w:t></w:r></w:p></w:hdr>
"""

FOOTER1 = f"""<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:ftr {W}><w:p><w:r><w:t>Trang đầu</w:t></w:r></w:p></w:ftr>
"""


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else (
        "src-tauri/crates/wasenoffice-ooxml/tests/fixtures/wasenoffice-full-fixture.docx"
    )
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", CONTENT_TYPES)
        z.writestr("_rels/.rels", ROOT_RELS)
        z.writestr("word/_rels/document.xml.rels", DOC_RELS)
        z.writestr("word/styles.xml", STYLES)
        z.writestr("word/numbering.xml", NUMBERING)
        z.writestr("word/header1.xml", HEADER1)
        z.writestr("word/footer1.xml", FOOTER1)
        z.writestr("word/media/image1.png", png_1x1(255, 0, 0))
        z.writestr("word/document.xml", document_xml())
    print(out)


if __name__ == "__main__":
    main()
