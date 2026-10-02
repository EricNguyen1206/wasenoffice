//! File formats — the single registry (T-052, review #2 C2).
//!
//! One table answers "what is `.<ext>`" for the whole Rust shell: which
//! editor boots (doctype + boot params), how open-delivery runs (x2t convert
//! vs copy-original), whether save is wired today and with which x2t format
//! code, plus the per-document workdir layout ([`DocWorkdir`]). Before this
//! module those facts were tribal knowledge of three files (lib.rs delivery
//! branch, save.rs gate + params XML, main.ts boot params).
//!
//! `src/main.ts` `FILE_FORMATS` mirrors the doctype/boot-param side for the
//! boot page — Rust cannot generate TS, so a TEST cross-checks the two tables
//! (entry count + filetype/doctype/isForm per entry) and fails the build when
//! they drift. Behavioral evidence per row:
//! docs/reference.map.md § "Desktop open contract — MULTI-FORMAT" and
//! § "Desktop save contract".

use std::path::{Path, PathBuf};

/// How the open-delivery chain produces the bytes the engine consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenFlavor {
    /// x2t positional conversion → `Editor.bin` (OOXML sources).
    Convert,
    /// Copy-original flavor (PDF and other CROSSPLATFORM formats): the shell
    /// delivers the ORIGINAL bytes, no converter spawn.
    CopyOriginal,
}

/// Save-side status of a format (multi-format save = T-049).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveCode {
    /// Save is wired today through x2t with this `<m_nFormatTo>` value.
    Wired(i64),
    /// Code oracle-confirmed but save NOT wired yet — T-049 will flip this.
    KnownUnwired(i64),
    /// Code not identified — T-049 định danh (spec-ref or oracle).
    Unidentified,
}

impl SaveCode {
    /// The `<m_nFormatTo>` to use IF this format's save is wired today.
    pub fn wired_code(self) -> Option<i64> {
        match self {
            SaveCode::Wired(code) => Some(code),
            SaveCode::KnownUnwired(_) | SaveCode::Unidentified => None,
        }
    }
}

/// One row of the format registry. This is a declarative table: some fields
/// (doctype / extra_boot_params / inner_xml_path) are read by the main.ts
/// cross-check TEST today, with production consumers arriving as T-049 wires
/// multi-format save — dead-code analysis does not count test reads.
#[allow(dead_code)]
#[derive(Debug)]
pub struct FileFormat {
    /// Extension (lowercase) — the table key.
    pub ext: &'static str,
    /// Boot param `doctype`: picks the editor app on the shared boot page
    /// (api.js appMap — reference.map § multi-format).
    pub doctype: &'static str,
    /// Boot params beyond filetype/doctype (pdf: `isForm=false`, exactly the
    /// real shell's boot params — reference.map § multi-format).
    pub extra_boot_params: &'static [(&'static str, &'static str)],
    /// Open-delivery flavor.
    pub open: OpenFlavor,
    /// Save-side status.
    pub save: SaveCode,
    /// Inner OOXML part where saved text lands (marker checks in save gates;
    /// docx proven by gate 10 `unzip -p … word/document.xml | grep marker`).
    pub inner_xml_path: Option<&'static str>,
}

impl FileFormat {
    /// The `<m_nFormatTo>` for saving, or None when this format's save is not
    /// wired (today: everything but docx — multi-format save = T-049).
    pub fn wired_save_code(&self) -> Option<i64> {
        self.save.wired_code()
    }
}

/// The format registry — the Rust half of the "formats 1 nhà" contract
/// (T-052); the TS half is `src/main.ts` FILE_FORMATS, cross-checked by test.
pub static FORMATS: &[FileFormat] = &[
    FileFormat {
        ext: "docx",
        doctype: "word",
        extra_boot_params: &[],
        open: OpenFlavor::Convert,
        // provenance: AVS_OFFICESTUDIO_FILE_DOCUMENT_DOCX = 65 = 0x0041 —
        // save contract step 3, reference.map § "Desktop save contract"
        // (oracle-confirmed 28.09, T-010 round-trip GREEN).
        save: SaveCode::Wired(65),
        // provenance: gate 10/11 marker check reads this part (smoke-save.sh).
        inner_xml_path: Some("word/document.xml"),
    },
    FileFormat {
        ext: "xlsx",
        doctype: "cell",
        extra_boot_params: &[],
        open: OpenFlavor::Convert,
        // provenance: 257 = XLSX — oracle-confirmed 29.09 (`<m_nFormatTo>` 257
        // → exit 0, `file` = "Microsoft Excel 2007+"; reference.map §
        // multi-format). Save itself NOT wired — T-049.
        save: SaveCode::KnownUnwired(257),
        // provenance TBD (T-049) — marker part for the xlsx save gate.
        inner_xml_path: None,
    },
    FileFormat {
        ext: "pptx",
        doctype: "slide",
        extra_boot_params: &[],
        open: OpenFlavor::Convert,
        // provenance: PPTX code chưa định danh (514/515/516/512/1025/8193/
        // 4097/258 đều fail — reference.map § multi-format). 513 = EXPORT PDF
        // từ editor binary bất kỳ, KHÔNG PHẢI PPTX — đừng tái sử dụng cho
        // pptx (bẫy đã ghi cho T-049). Định danh = T-049.
        save: SaveCode::Unidentified,
        // provenance TBD (T-049) — marker part for the pptx save gate.
        inner_xml_path: None,
    },
    FileFormat {
        ext: "pdf",
        doctype: "pdf",
        extra_boot_params: &[("isForm", "false")],
        open: OpenFlavor::CopyOriginal,
        // provenance: no-conversion flavor — save PDF là pipeline khác
        // (`localSaveToDrawingFormat`, OQ-F01-1); 513 chỉ là code EXPORT PDF
        // (xem pptx ở trên) — không áp dụng cho save pdf gốc.
        save: SaveCode::Unidentified,
        // raw PDF delivered as-is — no inner XML part exists.
        inner_xml_path: None,
    },
];

/// Registry lookup by lowercase extension.
pub fn by_ext(ext: &str) -> Option<&'static FileFormat> {
    FORMATS.iter().find(|f| f.ext == ext)
}

/// `<m_nCsvTxtEncoding>` value 46.
/// provenance TBD (T-049) — value observed: present in the save params XML
/// that round-tripped GREEN (T-010, 28.09) and untouched since.
pub const CSV_TXT_ENCODING: &str = "46";
/// `<m_nCsvDelimiter>` value 4.
/// provenance TBD (T-049) — value observed: same T-010 params XML as above.
pub const CSV_DELIMITER: &str = "4";
/// `<m_nDoctParams>` value 1.
/// provenance TBD (T-049) — value observed: same T-010 params XML as above.
pub const DOCT_PARAMS: &str = "1";

/// File name of the open-time converted binary (x2t positional output;
/// the engine's offline loader looks for exactly this name in the folder).
pub const EDITOR_BIN_NAME: &str = "Editor.bin";

/// Per-document x2t work directory — the ONE owner of the on-disk layout the
/// open-delivery chain (lib.rs) and the save chain (bridge/save.rs) share,
/// exactly like the real shell's recovery dir. The `folder` string format
/// (`<tmp>/nanooffice-doc-<id>`) is a contract consumed by
/// src/shim/delivery.js — frozen.
#[derive(Debug, Clone)]
pub struct DocWorkdir {
    root: PathBuf,
}

impl DocWorkdir {
    pub fn new(id: u32) -> Self {
        Self {
            root: std::env::temp_dir().join(format!("nanooffice-doc-{id}")),
        }
    }

    /// Workdir root — the delivery `folder` value.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Open-time converted binary (sidecar `OpenBinary` dst; save-time
    /// `<m_sFileFrom>`).
    pub fn editor_bin(&self) -> PathBuf {
        self.root.join(EDITOR_BIN_NAME)
    }

    /// Engine edit-deltas journal dir (x2t `<m_bFromChanges>` reads here).
    pub fn changes_dir(&self) -> PathBuf {
        self.root.join("changes")
    }

    /// `changes0.json` — byte-format owned by bridge::save (do not touch).
    pub fn changes_file(&self) -> PathBuf {
        self.changes_dir().join("changes0.json")
    }

    /// Save-time `<TaskQueueDataConvert>` params XML (sidecar
    /// `SaveFromChanges` input).
    pub fn params_xml(&self) -> PathBuf {
        self.root.join("params_to.xml")
    }

    /// Engine-visible delivered name for a flavor: `Editor.bin` for converted
    /// formats, the ORIGINAL file's own name for copy-original ones (the pdf
    /// loader falls back to `folder + "/" + file_name(src)` when Editor.bin
    /// is absent — reference.map § multi-format).
    pub fn delivered_name(&self, flavor: OpenFlavor, original: &Path) -> Result<String, String> {
        match flavor {
            OpenFlavor::Convert => Ok(EDITOR_BIN_NAME.to_string()),
            OpenFlavor::CopyOriginal => original
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .ok_or_else(|| "document path has no file name".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_the_four_formats() {
        for ext in ["docx", "xlsx", "pptx", "pdf"] {
            assert!(by_ext(ext).is_some(), "missing {ext}");
        }
        assert!(by_ext("txt").is_none());
        assert_eq!(FORMATS.len(), 4);
    }

    /// Today only docx save is wired — xlsx 257 is KNOWN but unwired (T-049).
    /// Pins the save gate semantics (xlsx/pptx saves still rejected).
    #[test]
    fn only_docx_save_is_wired() {
        assert_eq!(by_ext("docx").unwrap().wired_save_code(), Some(65));
        assert_eq!(by_ext("xlsx").unwrap().wired_save_code(), None);
        assert_eq!(by_ext("pptx").unwrap().wired_save_code(), None);
        assert_eq!(by_ext("pdf").unwrap().wired_save_code(), None);
    }

    /// The workdir layout contract (delivery.js consumes `folder`; save.rs
    /// consumes the inner paths).
    #[test]
    fn workdir_layout_is_pinned() {
        let work = DocWorkdir::new(1234);
        assert_eq!(
            work.root().file_name().unwrap().to_string_lossy(),
            "nanooffice-doc-1234"
        );
        assert_eq!(work.editor_bin(), work.root().join("Editor.bin"));
        assert_eq!(
            work.changes_file(),
            work.root().join("changes").join("changes0.json")
        );
        assert_eq!(work.params_xml(), work.root().join("params_to.xml"));
        // delivered names per flavor
        let orig = Path::new("/tmp/some/f.pdf");
        assert_eq!(
            work.delivered_name(OpenFlavor::CopyOriginal, orig).unwrap(),
            "f.pdf"
        );
        assert_eq!(
            work.delivered_name(OpenFlavor::Convert, orig).unwrap(),
            "Editor.bin"
        );
        // pathological: no file name → the historical error string
        assert_eq!(
            work.delivered_name(OpenFlavor::CopyOriginal, Path::new("/")),
            Err("document path has no file name".to_string())
        );
    }

    /// Minimal FILE_FORMATS row parser for src/main.ts (lines shaped like
    /// `  docx: { filetype: 'docx', doctype: 'word' },`).
    fn parse_main_ts_formats(src: &str) -> Vec<(String, String, String, Option<String>)> {
        let extract = |hay: &str, key: &str| -> Option<String> {
            let i = hay.find(key)? + key.len();
            let rest = &hay[i..];
            let end = rest.find('\'')?;
            Some(rest[..end].to_string())
        };
        let mut rows = Vec::new();
        for line in src.lines() {
            let Some((key, rest)) = line.split_once(':') else {
                continue;
            };
            if !rest.contains("filetype: '") {
                continue;
            }
            rows.push((
                key.trim().to_string(),
                extract(rest, "filetype: '").unwrap_or_default(),
                extract(rest, "doctype: '").unwrap_or_default(),
                extract(rest, "isForm: '"),
            ));
        }
        rows
    }

    /// Cross-check the two format tables (T-052): main.ts FILE_FORMATS ↔
    /// formats::FORMATS — same entries, filetype/doctype/isForm matching per
    /// entry. Chosen over a verify.sh script because it is the fewest moving
    /// parts (no extra gate, runs with `cargo test`). The file is read at
    /// RUNTIME, not include_str! — include_str! from outside the crate root
    /// does not reliably trigger rebuilds when main.ts changes (known trap),
    /// and a stale embed would silently pass this check.
    #[test]
    fn main_ts_file_formats_match_rust_table() {
        let main_ts = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/main.ts");
        let src = std::fs::read_to_string(&main_ts)
            .unwrap_or_else(|e| panic!("read {}: {e}", main_ts.display()));
        let ts = parse_main_ts_formats(&src);
        assert_eq!(
            ts.len(),
            FORMATS.len(),
            "main.ts FILE_FORMATS and formats::FORMATS entry counts differ: {ts:?}"
        );
        for fmt in FORMATS {
            let Some((_, filetype, doctype, is_form)) =
                ts.iter().find(|(ext, ..)| ext.as_str() == fmt.ext)
            else {
                panic!("main.ts FILE_FORMATS has no entry for .{}", fmt.ext);
            };
            assert_eq!(filetype.as_str(), fmt.ext, ".{} filetype", fmt.ext);
            assert_eq!(doctype.as_str(), fmt.doctype, ".{} doctype", fmt.ext);
            let want = fmt
                .extra_boot_params
                .iter()
                .find(|(k, _)| *k == "isForm")
                .map(|(_, v)| v.to_string());
            assert_eq!(is_form.as_deref(), want.as_deref(), ".{} isForm", fmt.ext);
        }
    }
}
