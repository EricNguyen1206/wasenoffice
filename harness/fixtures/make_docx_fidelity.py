#!/usr/bin/env python3
"""Generate the T-A31 fidelity corpus docx: hyperlink external + internal
anchor, a mid-document sectPr (pgSz+pgMar), and settings/theme/docProps
parts. parity-a2.sh picks it up via corpus-a2/*.docx (model INVARIANT +
passthrough byte-exactness); the crate tests parse the same file.
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

W = 'xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"'

CONTENT_TYPES = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
    '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
    '<Default Extension="xml" ContentType="application/xml"/>'
    '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
    f"{FIDELITY_CT_OVERRIDES}</Types>"
)
ROOT_RELS = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
    '<Relationship Id="rIdDoc" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>'
    f"{FIDELITY_ROOT_RELS_EXTRA}</Relationships>"
)
DOC_RELS = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
    '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>'
    f"{FIDELITY_DOC_RELS_EXTRA}</Relationships>"
)
STYLES = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    f'<w:styles {W}><w:docDefaults><w:rPrDefault><w:rPr>'
    '<w:rFonts w:ascii="Carlito" w:hAnsi="Carlito"/><w:sz w:val="22"/></w:rPr></w:rPrDefault></w:docDefaults>'
    '<w:style w:type="paragraph" w:default="1" w:styleId="Normal"><w:name w:val="Normal"/></w:style></w:styles>'
)
DOCUMENT = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    f"<w:document {W}><w:body>"
    '<w:p><w:r><w:t xml:space="preserve">Fidelity corpus header</w:t></w:r></w:p>'
    f"{FIDELITY_LINK_PARA}{FIDELITY_SECT_PARA}"
    '<w:p><w:r><w:t xml:space="preserve">after the section break</w:t></w:r></w:p>'
    '<w:sectPr><w:pgSz w:w="11906" w:h="16838"/>'
    '<w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134"/></w:sectPr>'
    "</w:body></w:document>"
)


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else "/tmp/wasenoffice-fidelity.docx"
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", CONTENT_TYPES)
        z.writestr("_rels/.rels", ROOT_RELS)
        z.writestr("word/_rels/document.xml.rels", DOC_RELS)
        z.writestr("word/styles.xml", STYLES)
        for name, xml in FIDELITY_PARTS.items():
            z.writestr(name, xml)
        z.writestr("word/document.xml", DOCUMENT)
    print(out)


if __name__ == "__main__":
    main()
