---
type: queue
---
# docs/TODO.md — hàng đợi công việc theo phase

> Nhà **duy nhất** của task vận hành (routing: `docs/README.md`). 1 task = 1 dòng pointer —
> chi tiết sống ở nhà của nó (handoff / DD-TEST / diagnostics) qua link.
>
> **Quy tắc:** thứ tự trong section = priority (trên = làm trước) · phụ thuộc ghi `← T-xxx` ·
> toàn file tối đa 1 dòng 🟨 (đơn nhiệm) — không đụng 🟨 của session khác · trạng thái: ⬜ todo ·
> 🟨 claimed (kèm ngày) · ⏸ blocked (kèm chờ ai) · ✅ done (kèm commit) · 🚫 dropped (lý do) ·
> ID `T-n` duy nhất không tái dùng · ✅ sink xuống đáy section · dashboard + dòng ▶ cập nhật khi
> đổi trạng thái · đóng phase → cắt ✅ sang `docs/TODO-archive.md` · section **META** dành cho
> việc harness/docs/skills không thuộc phase nào.

## Tình hình nhanh

▶ Đang làm: 🟨 T-A10 (30.09) — A1: Bundle x2t per-platform vào `engines/converter/` (T-050 đóng 30.09: probe lặp, pptx ×3 GREEN — chi tiết diagnostics 30.09; handoff `docs/handoff/ta10.md`)

| Phase | Tổng | ✅ | 🟨 | ⏸ | ⬜ |
|---|---|---|---|---|---|
| PH-0 — ĐÃ ĐÓNG 27.09  |  6  | 6  | 0  | 0  | 0  |
| PH-1  |  8  | 5  | 0  | 0  | 3  |
| PH-4  |  2  | 0  | 0  | 0  | 0  |
| Track A (QĐ9) |  4  | 0  | 1  | 0  | 3  |
| META  | 11  | 6  | 0  | 0  | 5  |

## PH-0 — Spike editor + render DOCX (go/no-go) — ĐÃ ĐÓNG 27.09 · verdict GO WITH CAVEAT

> Toàn bộ ✅ của phase cắt sang [`docs/TODO-archive.md`](TODO-archive.md). Caveat: criterion (b)
> scroll/edit keyboard chờ user xác nhận thủ công ~30s — bằng chứng ở `docs/diagnostics.md` 27.09
> (T-003). Baseline release đo ở PH-4.

## PH-1 — Vòng đời file (open/save/convert qua x2t sidecar)

- [ ] ⬜ T-049 · Save multi-format (XLSX/PPTX) qua x2t from-changes: định danh format code PPTX (spec-ref `AVS_OFFICESTUDIO_FILE_*` — core chưa clone, hoặc oracle tiếp) · XLSX = **257 đã oracle-confirmed** (29.09) · 513 = export PDF (đã obs, không phải PPTX) · PPTY→PPTX cần `<m_sFontDir>`+`<m_sAllFontsPath>` (exit 80 nếu thiếu) · không truyền m_nFormatTo thì extension output quyết định (PK hợp lệ) · save PDF = pipeline khác (`localSaveToDrawingFormat`) cần điều tra riêng (OQ-F01-1) · bằng chứng: reference.map § multi-format · ← T-048
- [ ] ⬜ T-046 · Module config `env.rs` gom `NANO_OFFICE_*` + 2 quy tắc khám phá artifact (review C5 — Speculative) · trigger: T-010 chạm sidecar config · diagnostics 27.09
- [ ] ⬜ T-047 · Điều tra dialog lỗi "An error has occurred…" thỉnh thoảng hiện sau khi open thành công (emitter `iLa` ở viewer api riêng) · ← T-010 · diagnostics 27.09 (T-002)
- [x] ✅ T-045 · Refactor open-delivery chain thành module có tên (ready predicate + timeout thành hằng tên) + probe sau `NANO_OFFICE_PROBE` (review C3) · 28.09 · 0d8f7a6
- [x] ✅ T-010 · Open/save/convert end-to-end qua x2t sidecar — **SAVE DOCX round-trip GREEN** (mở → `asc_AddText`+`asc_Save` probe → file gốc 1962→25613 B chứa text trong `word/document.xml` → reopen render GREEN; verify.sh exit 0) · 28.09 · a9e69d7 · contract: reference.map § Desktop save contract, diagnostics 28.09
- [x] ✅ T-011 · Round-trip test tự động trong verify.sh — gate 10 `smoke-save.sh` edit→save→disk (fixture fresh + marker random `NANO_OFFICE_SAVE_PROBE`; TC-D01-11 GREEN: standalone ×2 exit 0 + counterfactual probe-tắt exit 1 đúng kỳ vọng) · 28.09 · 7ba9150 · diagnostics 28.09
- [x] ✅ T-048 · Wire entry XLSX/PPTX/PDF — **mở được cả 4 định dạng** (map `FILE_FORMATS` 1 nhà trong main.ts: xlsx→cell/pptx→slide/pdf→pdf+isForm=false; PDF không qua x2t — deliver file gốc, oracle CDP app thật; shim font-loader generalise 3 build) · smoke gates xlsx/pptx/pdf trong verify.sh exit 0 + negative proven + gates docx/save regression GREEN · màn E01/P01/F01 đăng ký + DD/TEST · 29.09 · 7725690 · diagnostics 29.09
- [x] ✅ T-050 · Fix gate pptx flaky — probe one-shot 6/8/15s → **interval lặp 2s ×60 (dừng sớm khi canvas height>0)**, chuỗi log contract gate 9 giữ nguyên verbatim · chứng minh pptx ×3 liên tiếp PASSED exit 0 + docx regression GREEN · 30.09 · eec769d · diagnostics 30.09 ("fix landed")

## PH-4 — Đóng gói + updater (trigger: PH-2 xong)

- [ ] 🚫 T-040 · ~~Trim engines payload 313 MB → ≤100 MB~~ gộp vào Track A **T-A12** theo QĐ9 (29.09) — nằm trong rung A1
- [ ] 🚫 T-041 · ~~Quyết bundle vs download-first~~ gộp vào **T-A10** (A1 buộc bundle x2t) + PH-4 packaging (câu hỏi download-first còn lại áp cho toàn engine) theo QĐ9

## Track A — Tự chủ mã nguồn 100% (QĐ9, chốt 29.09 — thang A1→A6)

> **Ghi nhớ D1 (user 29.09):** engine AGPL giữ vai trò reference + "chế độ nâng cao"
> **tới tận A6 — KHÔNG hard-cut**; phân phối giai đoạn này chịu nghĩa vụ AGPL component (QĐ1).
> Rung hiện tại: **A1 — tự chủ phân phối** (H1). Task Track A bắt buộc đọc
> `harness/rules/clean-room.md` trước khi làm; roadmap đầy đủ: `docs/vision.md` QĐ9.

- [ ] 🟨 T-A10 (30.09) · A1: Bundle x2t per-platform vào `engines/converter/` (rút tham chiếu runtime đầu tiên khỏi ONLYOFFICE.app — còn fonts/AllFonts) · ← T-048 · QĐ9 · handoff: `docs/handoff/ta10.md`
- [ ] ⬜ T-A11 · A1: Tự sinh `AllFonts.js` bằng Rust (enumerate font hệ thống; format theo oracle-obs — không đọc data dir của app thật) · ← T-A10
- [ ] ⬜ T-A12 · A1: Font subset theo locale + trim engines 313 MB → ≤100 MB (nuốt T-040) · ← T-A11
- [ ] ⬜ T-A13 · A1: **Gate 11 "sạch ONLYOFFICE"** — round-trip open→edit→save trên máy KHÔNG cài ONLYOFFICE.app (x2t bundled + AllFonts tự sinh + fonts subset) · ← T-A12 · đích rung A1 trong vision QĐ9

## META — harness / docs / skills

- [ ] ⬜ T-020 · Add 2 skill theo audit 26.09: `systematic-debugging` (superpowers) + `handoff` (mattpocock) qua cổng acceptance · **chờ user duyệt**
- [ ] ⬜ T-021 · CI + gates số liệu size/RAM/startup · trigger: PH-4
- [ ] ⬜ T-034 · Bật GitNexus index `upstream/sdkjs` phục vụ reverse font pipeline (trigger đã chạm: bridge first wave xong) · ← T-001 · skill: gitnexus-cli
- [ ] ⬜ T-043 · Xoá stub `sidecar/` + `window/` (fail deletion test) + sửa lib.rs doc-comment + conventions.md (review C4, ~5 phút) · diagnostics 27.09 (review kiến trúc)
- [ ] ⬜ T-044 · Registry 1 nhà cho bridge method surface (~130 method đang sửa tay 3 nơi/2 ngôn ngữ) + `globals.d.ts` sync/async đúng contract (review C2) · trigger: trước bridge wave 2 PH-1 · diagnostics 27.09
- [x] ✅ T-042 · Gate 9 smoke render behavioral trong verify.sh — TC-D01-06 chạy tự động, exit code (architecture review Candidate 1; negative test proven) · 27.09 · 82502c1
- [x] ✅ T-022 · Migration OKF-thin: frontmatter `type` toàn docs + gate check-docs.mjs (QĐ8) · 27.09
- [x] ✅ T-030 · QĐ5 hai ngôi nhà harness/.agents + QĐ6 hệ DD/TEST mã hóa màn hình + CONTRIBUTING + docs routing · 26–27.09 · bbb99b1
- [x] ✅ T-031 · QĐ7 TODO queue + template handoff + vòng đời task + protocol đa-phiên · 27.09
- [x] ✅ T-032 · Chốt stack giữ Rust+Tauri (re-validate) · 26.09 · ea24231 · research: `techstack-analysis.md` (untracked — user: research không commit)
- [x] ✅ T-033 · Quyết định 5 "hai ngôi nhà" ghi vision · 26.09
