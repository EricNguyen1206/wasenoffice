#!/usr/bin/env python3
"""Shared helpers for the DOCX fixture generators (stdlib only).

T-056 (review #3 C5): png_1x1()+chunk() were copy-pasted across generators —
one home here. Namespace constants stay per-generator (each binds what it
uses); import from this module when a new generator needs the png helpers.
"""
import struct
import zlib


def chunk(tag: bytes, data: bytes) -> bytes:
    return struct.pack(">I", len(data)) + tag + data + struct.pack(
        ">I", zlib.crc32(tag + data)
    )


def png(rgb_rows: list[list[tuple[int, int, int]]]) -> bytes:
    """Truecolor PNG, one filter-0 scanline per row."""
    raw = b"".join(
        b"\x00" + b"".join(struct.pack("BBB", *px) for px in row) for row in rgb_rows
    )
    ihdr = struct.pack(">IIBBBBB", len(rgb_rows[0]), len(rgb_rows), 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b"")


def png_1x1(r: int, g: int, b: int) -> bytes:
    return png([[(r, g, b)]])


# T-A31 fidelity extras (hyperlink + mid sectPr + settings/theme/docProps) shared by the corpus generator and make_docx_layout.py's `fidelity` flag.

LINK_REL_ID = "rIdLink"
LINK_TARGET = "https://example.com/nanooffice-fidelity"
LINK_TEXT = "NanoOffice fidelity link"
ANCHOR_NAME = "sect-anchor"

_SETTINGS_XML = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
    '<w:defaultTabStop w:val="720"/></w:settings>'
)
_THEME_XML = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="NanoOffice">'
    '<a:themeElements>'
    '<a:clrScheme name="NanoOffice"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1>'
    '<a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="44546A"/></a:dk2>'
    '<a:lt2><a:srgbClr val="E7E6E6"/></a:lt2><a:accent1><a:srgbClr val="0563C1"/></a:accent1>'
    '<a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3>'
    '<a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="4472C4"/></a:accent5>'
    '<a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink>'
    '<a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme>'
    '<a:fontScheme name="NanoOffice"><a:majorFont><a:latin typeface="Carlito"/><a:ea typeface=""/>'
    '<a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Carlito"/><a:ea typeface=""/>'
    '<a:cs typeface=""/></a:minorFont></a:fontScheme>'
    '<a:fmtScheme name="NanoOffice">'
    '<a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>'
    '<a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>'
    '<a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>'
    '<a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln>'
    '<a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>'
    '<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle>'
    '<a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst>'
    '<a:outerShdw blurRad="57150" dist="19050" dir="5400000" algn="ctr" rotWithShape="0">'
    '<a:srgbClr val="000000"><a:alpha val="63000"/></a:srgbClr></a:outerShdw></a:effectLst></a:effectStyle></a:effectStyleLst>'
    '<a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill>'
    '<a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst>'
    '</a:fmtScheme></a:themeElements></a:theme>'
)
_CORE_XML = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" '
    'xmlns:dc="http://purl.org/dc/elements/1.1/">'
    '<dc:title>NanoOffice fidelity fixture</dc:title><dc:creator>NanoOffice</dc:creator></cp:coreProperties>'
)
_APP_XML = (
    '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
    '<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">'
    '<Application>NanoOffice</Application></Properties>'
)

FIDELITY_PARTS = {
    "word/settings.xml": _SETTINGS_XML,
    "word/theme/theme1.xml": _THEME_XML,
    "docProps/core.xml": _CORE_XML,
    "docProps/app.xml": _APP_XML,
}
FIDELITY_CT_OVERRIDES = (
    '<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>'
    '<Override PartName="/word/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>'
    '<Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>'
    '<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>'
)
FIDELITY_ROOT_RELS_EXTRA = (
    '<Relationship Id="rIdCore" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>'
    '<Relationship Id="rIdApp" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>'
)
FIDELITY_DOC_RELS_EXTRA = (
    f'<Relationship Id="{LINK_REL_ID}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink" Target="{LINK_TARGET}" TargetMode="External"/>'
    '<Relationship Id="rIdSettings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>'
    '<Relationship Id="rIdTheme" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/>'
)
FIDELITY_LINK_PARA = (
    '<w:p><w:r><w:t xml:space="preserve">Link paragraph: </w:t></w:r>'
    f'<w:hyperlink r:id="{LINK_REL_ID}"><w:r><w:rPr><w:color w:val="0563C1"/></w:rPr>'
    f'<w:t xml:space="preserve">{LINK_TEXT}</w:t></w:r></w:hyperlink>'
    '<w:r><w:t xml:space="preserve"> plus </w:t></w:r>'
    f'<w:hyperlink w:anchor="{ANCHOR_NAME}"><w:r><w:t xml:space="preserve">internal anchor</w:t></w:r></w:hyperlink></w:p>'
)
FIDELITY_SECT_PARA = (
    '<w:p><w:pPr><w:sectPr><w:pgSz w:w="11906" w:h="16838"/>'
    '<w:pgMar w:top="1134" w:right="1134" w:bottom="1134" w:left="1134" w:header="720" w:footer="720" w:gutter="0"/>'
    '</w:sectPr></w:pPr><w:r><w:t xml:space="preserve">Section one ends here</w:t></w:r></w:p>'
)
