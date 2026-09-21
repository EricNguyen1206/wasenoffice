//! `nanooffice-file://` custom protocol.
//!
//! Serves local files to the editing engine without exposing real disk paths:
//! the engine only ever sees `nanooffice-file://<fileId>/...` URLs.
//! Must support HTTP Range requests (engine fetches large embedded media partially).
//! See harness/skills/open-docx/SKILL.md for the open/save flow.
//!
//! TODO(ph0): register via tauri::Builder::register_uri_scheme_protocol,
//! fileId → absolute path map kept in window/session state (never sent to JS).
