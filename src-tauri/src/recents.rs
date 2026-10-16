//! Recents — danh sách tài liệu mở THÀNH CÔNG gần nhất (A01, DD §5).
//!
//! Rust là nguồn chân lý: store JSON ở app-data dir
//! (`WASEN_OFFICE_RECENTS_FILE` đè cho test/gate — verify không đụng recents
//! thật của user). Startpage chỉ render từ `recents_list` và gỡ bằng
//! `recents_remove`; việc ghi nhận xảy ra phía Rust tại mỗi open thành công
//! (`open_document` engine, `viewer_view` quick view, `wasen_light_view`/
//! `_reopen` Nano Light) — JS không bao giờ báo open. Store hỏng → reset rỗng,
//! không bao giờ chặn mở tài liệu hay làm crash startpage (DD §6).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::Manager;

/// Hard cap mới-nhất-trước (DD A01 §7) — entry cũ nhất rơi khỏi danh sách.
const MAX_ENTRIES: usize = 10;

/// Một dòng recents như lưu trên đĩa (`openedAt` = epoch ms lần mở gần nhất).
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecentEntry {
    pub path: String,
    pub name: String,
    /// Extension thường (không chấm, lowercase: docx/xlsx/pptx/pdf) — badge dòng.
    pub kind: String,
    pub opened_at: u64,
}

/// Một dòng như list cho startpage: trường đã lưu + probe tồn tại trên đĩa
/// TẠI THỜI ĐIỂM list (file có thể đã bị xoá từ lần mở trước — DD §6: dòng mờ).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentListed {
    #[serde(flatten)]
    pub entry: RecentEntry,
    pub exists: bool,
}

pub struct RecentsStore {
    file: PathBuf,
}

impl RecentsStore {
    /// Nhà sản phẩm: `<app-data>/recents.json`; `WASEN_OFFICE_RECENTS_FILE`
    /// đè (test/gate). app-data dir lỗi → fallback temp (store không critical).
    pub fn resolve(app: &tauri::AppHandle, override_path: Option<&str>) -> Self {
        if let Some(p) = override_path {
            return Self::at(PathBuf::from(p));
        }
        let file = app
            .path()
            .app_data_dir()
            .map(|dir| dir.join("recents.json"))
            .unwrap_or_else(|e| {
                log::warn!("recents: app data dir: {e} — falling back to temp");
                std::env::temp_dir().join("wasenoffice-recents.json")
            });
        Self::at(file)
    }

    pub fn at(file: PathBuf) -> Self {
        Self { file }
    }

    fn load(&self) -> Vec<RecentEntry> {
        let raw = match std::fs::read_to_string(&self.file) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
            Err(e) => {
                log::warn!(
                    "recents: read {}: {e} — starting empty",
                    self.file.display()
                );
                return Vec::new();
            }
        };
        match serde_json::from_str(&raw) {
            Ok(entries) => entries,
            Err(e) => {
                log::warn!(
                    "recents: store {} corrupt ({e}) — resetting to empty",
                    self.file.display()
                );
                Vec::new()
            }
        }
    }

    fn save(&self, entries: &[RecentEntry]) {
        if let Some(dir) = self.file.parent() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                log::warn!("recents: mkdir {}: {e}", dir.display());
                return;
            }
        }
        match serde_json::to_string_pretty(entries) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&self.file, json) {
                    log::warn!("recents: write {}: {e}", self.file.display());
                }
            }
            Err(e) => log::warn!("recents: serialize: {e}"),
        }
    }

    /// Upsert theo path — mới nhất lên đầu, cap `MAX_ENTRIES`. Best-effort:
    /// lỗi chỉ log, không bao giờ làm fail open (DD §6).
    pub fn record(&self, path: &Path) {
        let path_str = path.to_string_lossy().into_owned();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path_str.clone());
        let kind = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let opened_at = now_ms();
        let mut entries: Vec<RecentEntry> = self
            .load()
            .into_iter()
            .filter(|e| e.path != path_str)
            .collect();
        entries.insert(
            0,
            RecentEntry {
                path: path_str,
                name,
                kind,
                opened_at,
            },
        );
        entries.truncate(MAX_ENTRIES);
        self.save(&entries);
    }

    /// Dòng mới-nhất-trước + cờ tồn tại đĩa tươi tại lúc list.
    pub fn list(&self) -> Vec<RecentListed> {
        self.load()
            .into_iter()
            .map(|entry| RecentListed {
                exists: Path::new(&entry.path).is_file(),
                entry,
            })
            .collect()
    }

    /// Gỡ một entry theo path; true khi nó có tồn tại.
    pub fn remove(&self, path: &str) -> bool {
        let mut entries: Vec<RecentEntry> = self.load();
        let before = entries.len();
        entries.retain(|e| e.path != path);
        let removed = entries.len() < before;
        if removed {
            self.save(&entries);
        }
        removed
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Dòng recents cho startpage (A01) — kèm cờ `exists` tính tươi.
#[tauri::command]
pub fn recents_list(store: tauri::State<'_, std::sync::Arc<RecentsStore>>) -> Vec<RecentListed> {
    store.list()
}

/// Gỡ một dòng (nút × trên row / gợi ý sau "không tìm thấy").
#[tauri::command]
pub fn recents_remove(store: tauri::State<'_, std::sync::Arc<RecentsStore>>, path: String) -> bool {
    store.remove(&path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Store ở temp dir riêng từng test — fake paths không đụng đĩa, trừ
    /// test existence tạo file thật.
    fn temp_store(tag: &str) -> (RecentsStore, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "wasenoffice-recents-test-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        (RecentsStore::at(dir.join("recents.json")), dir)
    }

    fn entry(path: &str) -> RecentEntry {
        RecentEntry {
            path: path.into(),
            name: "a.docx".into(),
            kind: "docx".into(),
            opened_at: 1,
        }
    }

    #[test]
    fn test_record_upsert_moves_front_and_caps() {
        let (store, dir) = temp_store("upsert");
        store.record(Path::new("/docs/a.docx"));
        store.record(Path::new("/docs/b.docx"));
        store.record(Path::new("/docs/a.docx"));
        let listed = store.list();
        assert_eq!(listed.len(), 2);
        assert_eq!(
            listed[0].entry.path, "/docs/a.docx",
            "re-open moves to front"
        );
        assert_eq!(listed[1].entry.path, "/docs/b.docx");

        for i in 0..(MAX_ENTRIES as u32 + 2) {
            store.record(Path::new(&format!("/docs/fill-{i}.docx")));
        }
        assert_eq!(store.list().len(), MAX_ENTRIES, "hard cap drops oldest");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_list_flags_missing_files() {
        let (store, dir) = temp_store("exists");
        let real = dir.join("real.docx");
        std::fs::write(&real, b"x").unwrap();
        store.record(&real);
        store.record(Path::new("/docs/ghost.docx"));
        let listed = store.list();
        assert_eq!(listed.len(), 2);
        // newest first: ghost (recorded last) leads, the real file follows
        assert!(!listed[0].exists, "missing file listed with exists=false");
        assert!(listed[1].exists, "real file listed with exists=true");
        assert_eq!(listed[1].entry.kind, "docx");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_remove_removes_only_target() {
        let (store, dir) = temp_store("remove");
        store.record(Path::new("/docs/a.docx"));
        store.record(Path::new("/docs/b.docx"));
        assert!(store.remove("/docs/a.docx"));
        assert!(!store.remove("/docs/a.docx"), "second remove is a no-op");
        assert!(!store.remove("/docs/never.docx"));
        let listed = store.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].entry.path, "/docs/b.docx");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_corrupt_store_resets_empty() {
        let (store, dir) = temp_store("corrupt");
        std::fs::write(&store.file, "{not json").unwrap();
        assert!(store.list().is_empty(), "corrupt store reads as empty");
        store.record(Path::new("/docs/a.docx"));
        assert_eq!(store.list().len(), 1, "record after reset works");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_store_persists_across_instances() {
        let (store, dir) = temp_store("persist");
        store.record(Path::new("/docs/a.docx"));
        let reopened = RecentsStore::at(dir.join("recents.json"));
        let listed = reopened.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].entry.name, "a.docx");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn test_entry_serde_is_camel_case() {
        let json = serde_json::to_string(&entry("/docs/a.docx")).unwrap();
        assert!(
            json.contains("openedAt"),
            "wire format is camelCase (T-059 gotcha)"
        );
        let back: RecentEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(back.path, "/docs/a.docx");
    }
}
