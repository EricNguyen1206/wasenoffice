//! Safe disk storage & atomic file replacement seam (T-062, Candidate 2).
//!
//! Enforces disk safety invariants across all document-saving pipelines:
//! - Generates temporary sibling files in the SAME directory (same filesystem mount/volume).
//! - Rejects zero-byte/empty outputs to prevent file corruption.
//! - Executes atomic `std::fs::rename` over the original target path.
//! - Cleans up temporary artifacts on failure so no orphans remain on disk.

use std::path::{Path, PathBuf};

/// Generate a sibling temporary file path next to `target` (same volume).
pub fn sibling_tmp_path(target: &Path, tmp_tag: &str) -> PathBuf {
    let ext = target.extension().and_then(|e| e.to_str()).unwrap_or("tmp");
    target.with_extension(format!("{tmp_tag}.{ext}"))
}

/// Atomically replace `target` with data written by `writer(tmp_path)`.
///
/// Invariants:
/// 1. `tmp_path` is created next to `target`.
/// 2. If `writer` fails, `tmp_path` is removed and the error is returned.
/// 3. If `writer` succeeds, verifies `tmp_path` exists and has non-zero size.
/// 4. Renames `tmp_path` over `target` atomically; cleans up `tmp_path` if rename fails.
pub fn atomic_save<F>(target: &Path, tmp_tag: &str, writer: F) -> Result<u64, String>
where
    F: FnOnce(&Path) -> Result<(), String>,
{
    let tmp = sibling_tmp_path(target, tmp_tag);
    let _ = std::fs::remove_file(&tmp);

    if let Err(e) = writer(&tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }

    atomic_swap(&tmp, target)
}

/// Atomically write in-memory `bytes` over `target` via a temporary sibling.
pub fn atomic_write_bytes(target: &Path, bytes: &[u8], tmp_tag: &str) -> Result<u64, String> {
    if bytes.is_empty() {
        return Err(format!(
            "atomic_write_bytes {}: refusing to write 0 bytes",
            target.display()
        ));
    }
    atomic_save(target, tmp_tag, |tmp| {
        std::fs::write(tmp, bytes).map_err(|e| format!("write tmp {}: {e}", tmp.display()))
    })
}

/// Atomically swap an existing temporary file `tmp` over `target`.
///
/// Verifies `tmp` is non-empty before renaming. Cleans up `tmp` on failure.
pub fn atomic_swap(tmp: &Path, target: &Path) -> Result<u64, String> {
    let meta = match std::fs::metadata(tmp) {
        Ok(m) if m.len() > 0 => m,
        Ok(_) => {
            let _ = std::fs::remove_file(tmp);
            return Err(format!("atomic_swap: tmp file {} is empty", tmp.display()));
        }
        Err(e) => {
            let _ = std::fs::remove_file(tmp);
            return Err(format!(
                "atomic_swap: missing tmp file {}: {e}",
                tmp.display()
            ));
        }
    };

    if let Err(e) = std::fs::rename(tmp, target) {
        let _ = std::fs::remove_file(tmp);
        return Err(format!(
            "atomic_swap: rename {} over {}: {e}",
            tmp.display(),
            target.display()
        ));
    }

    Ok(meta.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sibling_tmp_path() {
        let target = Path::new("/var/docs/report.docx");
        let tmp = sibling_tmp_path(target, "wasenoffice-save");
        assert_eq!(tmp, Path::new("/var/docs/report.wasenoffice-save.docx"));
    }

    #[test]
    fn test_atomic_write_bytes_success() {
        let dir = std::env::temp_dir().join("wasenoffice-test-storage-1");
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("doc.docx");

        let bytes = b"hello wasenoffice storage";
        let len = atomic_write_bytes(&target, bytes, "test-tmp").unwrap();
        assert_eq!(len, bytes.len() as u64);

        let read_back = std::fs::read(&target).unwrap();
        assert_eq!(read_back, bytes);

        // tmp sibling is removed by rename
        let tmp = sibling_tmp_path(&target, "test-tmp");
        assert!(!tmp.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_write_bytes_empty_refused() {
        let dir = std::env::temp_dir().join("wasenoffice-test-storage-2");
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("doc.docx");

        let res = atomic_write_bytes(&target, b"", "test-tmp");
        assert!(res.is_err());
        assert!(!target.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_swap_success() {
        let dir = std::env::temp_dir().join("wasenoffice-test-storage-3");
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("final.docx");
        let tmp = sibling_tmp_path(&target, "tmp-test");

        std::fs::write(&tmp, b"sample content").unwrap();
        let len = atomic_swap(&tmp, &target).unwrap();
        assert_eq!(len, 14);

        assert!(target.exists());
        assert!(!tmp.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_atomic_swap_empty_cleans_up() {
        let dir = std::env::temp_dir().join("wasenoffice-test-storage-4");
        let _ = std::fs::create_dir_all(&dir);
        let target = dir.join("final.docx");
        let tmp = sibling_tmp_path(&target, "tmp-test");

        std::fs::write(&tmp, b"").unwrap();
        let res = atomic_swap(&tmp, &target);
        assert!(res.is_err());
        assert!(!tmp.exists(), "empty tmp file must be cleaned up");
        assert!(!target.exists());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
