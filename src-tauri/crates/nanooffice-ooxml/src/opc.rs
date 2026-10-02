//! OPC package layer (ECMA-376 Part 2): a docx is a zip; a part is a named
//! zip entry. Slice scope: read one part by its well-known name.

use std::io::Read as _;

use crate::ParseError;

/// Read one part's bytes by its package path (e.g. `word/document.xml`).
pub fn part_bytes(zip_bytes: &[u8], name: &str) -> Result<Vec<u8>, ParseError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .map_err(|e| ParseError(format!("zip: {e}")))?;
    let mut part = archive
        .by_name(name)
        .map_err(|e| ParseError(format!("part {name}: {e}")))?;
    let mut out = Vec::with_capacity(part.size() as usize);
    part.read_to_end(&mut out)
        .map_err(|e| ParseError(format!("read part {name}: {e}")))?;
    Ok(out)
}
