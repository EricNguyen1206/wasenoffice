//! x2t sidecar — the converter seam (T-A14, review #2 C1; absorbs T-043).
//!
//! Single home for every converter-process concern:
//! - binary resolution ([`x2t_path`]): env override → bundled walk-up →
//!   installed-app fallback (killed by clean-machine mode) — moved verbatim
//!   from bridge/save.rs (T-A10/T-A13); log lines are a frozen contract for
//!   gates 10/11 (grep targets) and stay byte-identical.
//! - the conversion interface [`convert`]: the 3 call shapes the shell uses
//!   today (open-time positional, save-time params XML, PDF passthrough).
//!   Spawn + output capture live HERE, once — call sites only pick a shape.
//!   Track A's Rust converter adapter lives here too ([`rust_canonical`],
//!   T-A24): docx → canonical via the crate, then x2t (kill-switchable).
//!
//! Process rules (harness/skills/x2t-sidecar/SKILL.md): one short-lived
//! process per conversion task, never a long-running daemon; max 1 concurrent
//! task per instance; stderr of the sidecar is the first failure evidence →
//! always captured and surfaced. No timeout yet — matches today's behavior
//! (blocking `.output()`); the day a hang is observed, the kill lands here,
//! at the single spawn site.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// x2t sidecar binary — resolution order (T-A10, Track A rung A1):
/// 1. env `NANO_OFFICE_X2T` (tests / custom builds) — honored only when the
///    file exists, so a bogus value falls through to the bundled converter
///    (the T-A10 counterfactual relies on that). The resolved path is logged:
///    gate 11 asserts 0 `/Applications/ONLYOFFICE` references in the whole
///    log, so an override wrongly pointed into the installed app is caught.
/// 2. bundled converter: `engines/converter/<target>/x2t` (created by
///    `harness/scripts/bundle-x2t.sh`), found by walking up from the exe dir
///    and the cwd — dev trees have the repo root 3 levels above target/debug
/// 3. installed ONLYOFFICE.app fallback — logged as WARN; DISABLED under
///    clean-machine mode (`NANO_OFFICE_NO_APP_FALLBACK`), which turns a
///    missing bundled converter into a hard error instead (T-A13, gate 11)
pub fn x2t_path() -> String {
    x2t_path_in(&discovery_roots())
}

/// Same resolution over explicit discovery roots — the resolution-order pin
/// for tests (tmp dirs + env, never the real /Applications tree).
fn x2t_path_in(roots: &[PathBuf]) -> String {
    if let Ok(p) = std::env::var("NANO_OFFICE_X2T") {
        if !p.is_empty() {
            if Path::new(&p).is_file() {
                log::info!("[x2t] using NANO_OFFICE_X2T override: {p}");
                return p;
            }
            log::warn!("x2t: NANO_OFFICE_X2T points to a missing file ({p}) — ignoring");
        }
    }
    if let Some(p) = find_bundled_x2t_in(roots) {
        return p;
    }
    if no_app_fallback() {
        log::error!(
            "x2t: clean-machine mode (NANO_OFFICE_NO_APP_FALLBACK set): bundled converter engines/converter/{}/x2t not found — app fallback disabled",
            target_dir_name()
        );
        return String::new();
    }
    log::warn!(
        "x2t: bundled converter engines/converter/{}/x2t not found — falling back to the installed ONLYOFFICE.app (A1 runtime dependency to remove)",
        target_dir_name()
    );
    "/Applications/ONLYOFFICE.app/Contents/Resources/converter/x2t".to_string()
}

/// Clean-machine mode (`NANO_OFFICE_NO_APP_FALLBACK`): set (to anything but
/// "0") the installed-app fallback is off and a missing bundled converter
/// fails loudly. Gate 11 boots the app with this always on; a dev opts out
/// explicitly to test the fallback path.
fn no_app_fallback() -> bool {
    match std::env::var("NANO_OFFICE_NO_APP_FALLBACK") {
        Ok(v) => v != "0",
        Err(_) => false,
    }
}

/// `engines/converter/<target>` dir name — composed from std::env::consts.
/// Single home for the naming rule: `harness/scripts/bundle-x2t.sh` derives
/// the same name from `rustc -vV` host at bundle time — keep the two in sync.
fn target_dir_name() -> String {
    let arch = std::env::consts::ARCH;
    match std::env::consts::OS {
        "macos" => format!("{arch}-apple-darwin"),
        "linux" => format!("{arch}-unknown-linux-gnu"),
        "windows" => format!("{arch}-pc-windows-msvc"),
        os => format!("{arch}-{os}"),
    }
}

/// Walk up the given roots looking for `engines/converter/<target>/x2t`
/// (real roots come from [`discovery_roots`]: exe dir ancestors first, then
/// cwd ancestors — the order the shell has always used). Returns the path
/// and logs it — gate 10 evidence that the save used the bundled converter
/// (smoke-save.sh keeps the full log).
fn find_bundled_x2t_in(roots: &[PathBuf]) -> Option<String> {
    let rel = format!("engines/converter/{}/x2t", target_dir_name());
    for root in roots {
        let cand = root.join(&rel);
        if cand.is_file() {
            log::info!("[x2t] using bundled converter: {}", cand.display());
            return Some(cand.to_string_lossy().into_owned());
        }
    }
    None
}

/// Real walk-up roots: ancestors of the exe dir (skipping the exe itself),
/// then ancestors of the cwd.
fn discovery_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        roots.extend(exe.ancestors().skip(1).map(Path::to_path_buf));
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.extend(cwd.ancestors().map(Path::to_path_buf));
    }
    roots
}

/// The conversion request shapes the shell uses today (T-A14). Each shape
/// carries its own success criterion — they are deliberately NOT uniform,
/// mirroring what x2t actually does per mode.
pub enum ConvertRequest<'a> {
    /// Open-time binary build: `x2t <src> <dst>` (positional 2-arg).
    /// Success = dst EXISTS after the run — deliberately not the exit status:
    /// x2t is routinely non-zero while still writing a valid output
    /// (observed, T-A10); the open chain has only ever checked dst presence.
    OpenBinary { src: &'a Path, dst: &'a Path },
    /// Save-time from-changes conversion: `x2t <params_xml>` (1-arg params
    /// file). Success = exit 0 AND output exists non-empty — a non-zero exit
    /// with output present means a PARTIAL conversion (observed 28.09:
    /// missing font dir → changes dropped, file still written) which must
    /// never be swapped over the user's original.
    SaveFromChanges {
        params_xml: &'a Path,
        output: &'a Path,
    },
    /// No-conversion flavor (PDF and other CROSSPLATFORM formats): copy the
    /// original, never spawn a converter.
    Passthrough { src: &'a Path, dst: &'a Path },
}

/// A completed conversion.
#[derive(Debug)]
pub struct ConvertResult {
    /// Wall time of the process run (or the copy, for [`ConvertRequest::Passthrough`]).
    pub elapsed: Duration,
}

/// A failed conversion. `note` is the fully-formed message the call site
/// reported historically — kept verbatim (error surfaces + log greps depend
/// on these strings).
#[derive(Debug)]
pub struct ConvertError {
    pub note: String,
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.note)
    }
}

/// Run one conversion through the seam. Spawn + output capture happen here,
/// once, for every call shape.
pub fn convert(req: &ConvertRequest<'_>) -> Result<ConvertResult, ConvertError> {
    match req {
        ConvertRequest::Passthrough { src, dst } => {
            let started = Instant::now();
            std::fs::copy(src, dst).map_err(|e| ConvertError {
                note: format!("copy original into workdir: {e}"),
            })?;
            Ok(ConvertResult {
                elapsed: started.elapsed(),
            })
        }
        ConvertRequest::OpenBinary { src, dst } => {
            // A2 adapter (T-A24): docx goes through the crate first — x2t
            // converts the canonical copy, everything else is untouched.
            let canonical = rust_canonical(src, dst);
            let src = canonical.as_deref().unwrap_or(src);
            let run = spawn_x2t(&[src, dst])?;
            if !dst.is_file() {
                return Err(ConvertError {
                    note: format!(
                        "x2t produced no Editor.bin (exit {:?}, stderr: {})",
                        run.exit,
                        run.stderr.chars().take(200).collect::<String>()
                    ),
                });
            }
            Ok(ConvertResult {
                elapsed: run.elapsed,
            })
        }
        ConvertRequest::SaveFromChanges { params_xml, output } => {
            let run = spawn_x2t(&[params_xml])?;
            let complete = run.exit == Some(0)
                && output.is_file()
                && std::fs::metadata(output).is_ok_and(|m| m.len() > 0);
            if !complete {
                let detail = if run.stderr.trim().is_empty() {
                    &run.stdout
                } else {
                    &run.stderr
                };
                return Err(ConvertError {
                    note: format!(
                        "x2t failed (exit {:?}, output: {})",
                        run.exit,
                        detail.chars().take(200).collect::<String>()
                    ),
                });
            }
            Ok(ConvertResult {
                elapsed: run.elapsed,
            })
        }
    }
}

/// A2 adapter (T-A24) — the Rust OOXML crate's slot in the OPEN path: parse
/// the user `.docx` and write a canonical one, then x2t converts THAT into
/// Editor.bin (Editor.bin itself stays x2t's until A4 — DD K01 §2 P2). A file
/// the crate refuses must still open exactly as before → any parse/write
/// error only WARNs and falls back to the original bytes. Kill switch
/// `NANO_OFFICE_NO_RUST_CONVERTER` (set ≠ "0", like `NO_APP_FALLBACK`)
/// restores the direct path — counterfactual + rollback.
fn rust_canonical(src: &Path, dst: &Path) -> Option<PathBuf> {
    let is_docx = src
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("docx"));
    if !is_docx {
        return None; // crate scope: docx today (xlsx/pptx = S5)
    }
    if matches!(std::env::var("NANO_OFFICE_NO_RUST_CONVERTER"), Ok(v) if v != "0") {
        log::info!(
            "converter: rust-canonical OFF (NANO_OFFICE_NO_RUST_CONVERTER set) — x2t direct"
        );
        return None;
    }
    let canonical = dst.with_file_name("canonical.docx");
    let started = Instant::now();
    let outcome = std::fs::read(src)
        .map_err(|e| format!("read: {e}"))
        .and_then(|bytes| {
            nanooffice_ooxml::parse_docx(&bytes)
                .map_err(|e| e.to_string())
                .and_then(|doc| {
                    // source bytes go back in: media parts must survive the
                    // canonical re-serialize byte-exact (T-A26)
                    nanooffice_ooxml::write_docx_file(&doc, Some(&bytes), &canonical)
                        .map_err(|e| e.to_string())
                })
        });
    match outcome {
        Ok(()) => {
            log::info!(
                "converter: rust-canonical → x2t ({} → {}, normalize {:?})",
                src.display(),
                canonical.display(),
                started.elapsed()
            );
            Some(canonical)
        }
        Err(e) => {
            log::warn!("converter: rust-canonical failed ({e}) — x2t takes the original file");
            None
        }
    }
}

struct RunOutput {
    exit: Option<i32>,
    stdout: String,
    stderr: String,
    elapsed: Duration,
}

/// The single spawn site of the shell. stderr first, stdout as fallback —
/// both always captured (sidecar rules, harness/skills/x2t-sidecar).
fn spawn_x2t(args: &[&Path]) -> Result<RunOutput, ConvertError> {
    let started = Instant::now();
    let out = std::process::Command::new(x2t_path())
        .args(args)
        .output()
        .map_err(|e| ConvertError {
            note: format!("spawn x2t: {e}"),
        })?;
    Ok(RunOutput {
        exit: out.status.code(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        elapsed: started.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nanooffice-test-sidecar-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The bundled layout is exactly `engines/converter/<target>/x2t` under
    /// one of the walk-up roots.
    #[test]
    fn bundled_discovery_layout_is_pinned() {
        let dir = tmp("layout");
        assert_eq!(find_bundled_x2t_in(&[dir.clone()]), None);
        let conv = dir.join("engines/converter").join(target_dir_name());
        std::fs::create_dir_all(&conv).unwrap();
        std::fs::write(conv.join("x2t"), b"").unwrap();
        let got = find_bundled_x2t_in(&[dir]).unwrap();
        assert_eq!(got, conv.join("x2t").to_string_lossy().into_owned());
    }

    /// Resolution order through `x2t_path_in` (tmp roots + env, never the
    /// real /Applications tree): env-if-exists → bundled → strict-error /
    /// app-fallback. All env-dependent scenarios live in ONE test fn —
    /// std::env is process-global and cargo runs tests in parallel threads.
    #[test]
    fn resolution_order_env_bundled_strict() {
        let dir = tmp("resolution");
        let bundle_root = dir.join("repo");
        let conv = bundle_root
            .join("engines/converter")
            .join(target_dir_name());
        std::fs::create_dir_all(&conv).unwrap();
        let bundled = conv.join("x2t");
        std::fs::write(&bundled, b"").unwrap();
        let empty_root = dir.join("empty");
        std::fs::create_dir_all(&empty_root).unwrap();
        let env_file = dir.join("override-x2t");
        std::fs::write(&env_file, b"").unwrap();

        let saved_override = std::env::var("NANO_OFFICE_X2T").ok();
        let saved_strict = std::env::var("NANO_OFFICE_NO_APP_FALLBACK").ok();
        std::env::remove_var("NANO_OFFICE_X2T");
        std::env::remove_var("NANO_OFFICE_NO_APP_FALLBACK");

        // 1. env override pointing at an EXISTING file wins over bundled
        std::env::set_var("NANO_OFFICE_X2T", &env_file);
        assert_eq!(
            x2t_path_in(&[bundle_root.clone()]),
            env_file.to_string_lossy().into_owned()
        );

        // 2. bogus env (missing file) is ignored → falls through to bundled
        std::env::set_var(
            "NANO_OFFICE_X2T",
            dir.join("missing-x2t").to_string_lossy().into_owned(),
        );
        assert_eq!(
            x2t_path_in(&[bundle_root.clone()]),
            bundled.to_string_lossy().into_owned()
        );

        // 3. no env, no bundled, strict mode → hard error (empty path)
        std::env::remove_var("NANO_OFFICE_X2T");
        std::env::set_var("NANO_OFFICE_NO_APP_FALLBACK", "1");
        assert_eq!(x2t_path_in(&[empty_root.clone()]), "");

        // 4. no env, no bundled, no strict → installed-app fallback constant
        std::env::remove_var("NANO_OFFICE_NO_APP_FALLBACK");
        assert_eq!(
            x2t_path_in(&[empty_root.clone()]),
            "/Applications/ONLYOFFICE.app/Contents/Resources/converter/x2t"
        );

        // restore whatever the environment had before the test
        match saved_override {
            Some(v) => std::env::set_var("NANO_OFFICE_X2T", v),
            None => std::env::remove_var("NANO_OFFICE_X2T"),
        }
        match saved_strict {
            Some(v) => std::env::set_var("NANO_OFFICE_NO_APP_FALLBACK", v),
            None => std::env::remove_var("NANO_OFFICE_NO_APP_FALLBACK"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The 3 request shapes through the interface, against a fake converter
    /// resolved via the env override (branch 1 — deterministic regardless of
    /// whether engines/ exists on this machine).
    #[cfg(unix)]
    #[test]
    fn convert_shapes_through_the_seam() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tmp("iface");
        // fake x2t: 1-arg = params-XML mode (write payload to <m_sFileTo>);
        // 2-arg = positional mode (copy src → dst)
        let fake = dir.join("fake-x2t.sh");
        std::fs::write(
            &fake,
            concat!(
                "#!/bin/sh\n",
                "if [ \"$#\" -eq 1 ]; then\n",
                "  to=$(sed -n 's/.*<m_sFileTo>\\([^<]*\\)<\\/m_sFileTo>.*/\\1/p' \"$1\")\n",
                "  printf 'saved-by-fake' > \"$to\"\n",
                "  exit 0\n",
                "fi\n",
                "cp \"$1\" \"$2\"\n",
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let failing = dir.join("failing-x2t.sh");
        std::fs::write(&failing, "#!/bin/sh\necho boom >&2\nexit 3\n").unwrap();
        std::fs::set_permissions(&failing, std::fs::Permissions::from_mode(0o755)).unwrap();

        let saved_override = std::env::var("NANO_OFFICE_X2T").ok();
        std::env::set_var("NANO_OFFICE_X2T", &fake);

        // (a) OpenBinary: success = dst exists (fake copies)
        let src = dir.join("in.docy");
        std::fs::write(&src, b"DOCY;v10").unwrap();
        let dst = dir.join("Editor.bin");
        convert(&ConvertRequest::OpenBinary {
            src: &src,
            dst: &dst,
        })
        .unwrap();
        assert_eq!(std::fs::read(&dst).unwrap(), b"DOCY;v10");

        // (b) SaveFromChanges: success = exit 0 + non-empty output
        let params = dir.join("params_to.xml");
        let out = dir.join("out.docx");
        std::fs::write(
            &params,
            format!(
                "<TaskQueueDataConvert><m_sFileTo>{}</m_sFileTo></TaskQueueDataConvert>",
                out.display()
            ),
        )
        .unwrap();
        convert(&ConvertRequest::SaveFromChanges {
            params_xml: &params,
            output: &out,
        })
        .unwrap();
        assert!(out.metadata().is_ok_and(|m| m.len() > 0));

        // (b') failing converter → the historical save-side error note
        std::env::set_var("NANO_OFFICE_X2T", &failing);
        let err = convert(&ConvertRequest::SaveFromChanges {
            params_xml: &params,
            output: &out,
        })
        .unwrap_err();
        assert!(err
            .note
            .starts_with("x2t failed (exit Some(3), output: boom"));

        // (c) Passthrough never spawns: copy succeeds even though the env
        // override points at the failing converter
        let copy_dst = dir.join("copy.pdf");
        convert(&ConvertRequest::Passthrough {
            src: &src,
            dst: &copy_dst,
        })
        .unwrap();
        assert_eq!(std::fs::read(&copy_dst).unwrap(), b"DOCY;v10");

        // (d) T-A24 adapter: a .docx source round-trips through the crate —
        // canonical.docx lands next to Editor.bin and is what x2t receives
        // (b' left the env on the failing converter — back to the fake one)
        std::env::set_var("NANO_OFFICE_X2T", &fake);
        let doc = nanooffice_ooxml::Document {
            body: vec![nanooffice_ooxml::Block::Paragraph(
                nanooffice_ooxml::Paragraph {
                    runs: vec![nanooffice_ooxml::Run {
                        text: "adapter".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            )],
            headers: vec![],
            footers: vec![],
            styles: Default::default(),
            numbering: Default::default(),
            doc_defaults: None,
            page: None,
            page_mar: None,
        };
        let docx = dir.join("t24.docx");
        nanooffice_ooxml::write_docx_file(&doc, None, &docx).unwrap();
        let dst2 = dir.join("Editor2.bin");
        let canonical = dir.join("canonical.docx");
        convert(&ConvertRequest::OpenBinary {
            src: &docx,
            dst: &dst2,
        })
        .unwrap();
        assert!(
            canonical.is_file(),
            "adapter must write canonical.docx next to Editor.bin"
        );
        assert_eq!(
            std::fs::read(&dst2).unwrap(),
            std::fs::read(&canonical).unwrap(),
            "x2t must receive the canonical copy, not the original"
        );

        // (e) crate refuses the file → fallback: the open still works, and
        // no stale canonical copy is left behind
        std::fs::remove_file(&canonical).unwrap();
        std::fs::write(&docx, b"not a zip").unwrap();
        convert(&ConvertRequest::OpenBinary {
            src: &docx,
            dst: &dst2,
        })
        .unwrap();
        assert!(dst2.is_file());
        assert!(
            !canonical.exists(),
            "fallback must not leave a canonical copy"
        );

        // (f) kill switch NANO_OFFICE_NO_RUST_CONVERTER → direct path, no
        // canonical copy even for a file the crate would accept
        nanooffice_ooxml::write_docx_file(&doc, None, &docx).unwrap();
        let saved_switch = std::env::var("NANO_OFFICE_NO_RUST_CONVERTER").ok();
        std::env::set_var("NANO_OFFICE_NO_RUST_CONVERTER", "1");
        convert(&ConvertRequest::OpenBinary {
            src: &docx,
            dst: &dst2,
        })
        .unwrap();
        assert!(
            !canonical.exists(),
            "kill switch must restore the direct path"
        );

        match saved_override {
            Some(v) => std::env::set_var("NANO_OFFICE_X2T", v),
            None => std::env::remove_var("NANO_OFFICE_X2T"),
        }
        match saved_switch {
            Some(v) => std::env::set_var("NANO_OFFICE_NO_RUST_CONVERTER", v),
            None => std::env::remove_var("NANO_OFFICE_NO_RUST_CONVERTER"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
