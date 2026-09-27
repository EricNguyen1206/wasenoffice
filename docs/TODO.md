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

▶ Kế tiếp: PH-1 — T-045 (refactor open-delivery chain, trigger mở PH-1) rồi T-010 (open/save/convert
qua x2t sidecar). Lưu ý caveat từ T-003: user xác nhận scroll/edit thủ công ~30s (diagnostics 27.09).

| Phase | Tổng | ✅ | 🟨 | ⏸ | ⬜ |
|---|---|---|---|---|---|
| PH-0 — ĐÃ ĐÓNG 27.09  |  6  | 6  | 0  | 0  | 0  |
| PH-1  |  5  | 0  | 0  | 0  | 5  |
| PH-4  |  2  | 0  | 0  | 0  | 2  |
| META  | 11  | 6  | 0  | 0  | 5  |

## PH-0 — Spike editor + render DOCX (go/no-go) — ĐÃ ĐÓNG 27.09 · verdict GO WITH CAVEAT

> Toàn bộ ✅ của phase cắt sang [`docs/TODO-archive.md`](TODO-archive.md). Caveat: criterion (b)
> scroll/edit keyboard chờ user xác nhận thủ công ~30s — bằng chứng ở `docs/diagnostics.md` 27.09
> (T-003). Baseline release đo ở PH-4.

## PH-1 — Vòng đời file (open/save/convert qua x2t sidecar)

- [ ] ⬜ T-045 · Refactor open-delivery chain thành module có tên (ready predicate + timeout thành hằng tên) + probe sau `NANO_OFFICE_PROBE` (review C3) · trigger: mở PH-1 · diagnostics 27.09 (review kiến trúc)
- [ ] ⬜ T-010 · Open/save/convert end-to-end qua x2t sidecar · ← T-003
- [ ] ⬜ T-011 · Round-trip test tự động thêm vào verify.sh · ← T-010
- [ ] ⬜ T-046 · Module config `env.rs` gom `NANO_OFFICE_*` + 2 quy tắc khám phá artifact (review C5 — Speculative) · trigger: T-010 chạm sidecar config · diagnostics 27.09
- [ ] ⬜ T-047 · Điều tra dialog lỗi "An error has occurred…" thỉnh thoảng hiện sau khi open thành công (emitter `iLa` ở viewer api riêng) · ← T-010 · diagnostics 27.09 (T-002)

## PH-4 — Đóng gói + updater (trigger: PH-2 xong)

- [ ] ⬜ T-040 · Trim engines payload 313 MB → ≤100 MB budget (per-app-type build, strip loginpage/cloud) · vision QĐ2
- [ ] ⬜ T-041 · Quyết bundle vs download-first cho engine AGPL (OQ — vision Câu hỏi mở) · ← T-040

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
