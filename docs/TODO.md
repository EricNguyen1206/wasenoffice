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

▶ Đang làm: (chưa có — T-003 verdict go/no-go PH-0 là việc kế tiếp)

| Phase | Tổng | ✅ | 🟨 | ⏸ | ⬜ |
|---|---|---|---|---|---|
| PH-0  |  6  | 5  | 0  | 0  | 1  |
| PH-1  |  2  | 0  | 0  | 0  | 2  |
| PH-4  |  2  | 0  | 0  | 0  | 2  |
| META  |  8  | 5  | 0  | 0  | 3  |

## PH-0 — Spike editor + render DOCX (go/no-go)

- [ ] ⬜ T-003 · Verdict go/no-go PH-0 + baseline size/RAM/cold-start ghi diagnostics · ← T-002
- [x] ✅ T-004 · Engine boot WKWebView không CEF · 22.09
- [x] ✅ T-005 · Bridge first wave 15+ method DONE-PH0 · 22.09
- [x] ✅ T-006 · LoadJS bootstrap contract + wasm locateFile fix · 22.09
- [x] ✅ T-001 · Chọn phương án phá blocker render · 27.09 · user ủy quyền → **(a) oracle-diff CDP** · triển khai trong T-002
- [x] ✅ T-002 · Triển khai (a) đến khi render qua vạch — **TC-D01-06 GREEN** (contentReady + viewer 2302×958 + 14 fetch `/fonts`, Hcc=15 khớp oracle app thật) · 27.09 · 14ee92c · diagnostics 27.09

## PH-1 — Vòng đời file (open/save/convert qua x2t sidecar)

- [ ] ⬜ T-010 · Open/save/convert end-to-end qua x2t sidecar · ← T-003
- [ ] ⬜ T-011 · Round-trip test tự động thêm vào verify.sh · ← T-010

## PH-4 — Đóng gói + updater (trigger: PH-2 xong)

- [ ] ⬜ T-040 · Trim engines payload 313 MB → ≤100 MB budget (per-app-type build, strip loginpage/cloud) · vision QĐ2
- [ ] ⬜ T-041 · Quyết bundle vs download-first cho engine AGPL (OQ — vision Câu hỏi mở) · ← T-040

## META — harness / docs / skills

- [ ] ⬜ T-020 · Add 2 skill theo audit 26.09: `systematic-debugging` (superpowers) + `handoff` (mattpocock) qua cổng acceptance · **chờ user duyệt**
- [ ] ⬜ T-021 · CI + gates số liệu size/RAM/startup · trigger: PH-4
- [ ] ⬜ T-034 · Bật GitNexus index `upstream/sdkjs` phục vụ reverse font pipeline (trigger đã chạm: bridge first wave xong) · ← T-001 · skill: gitnexus-cli
- [x] ✅ T-022 · Migration OKF-thin: frontmatter `type` toàn docs + gate check-docs.mjs (QĐ8) · 27.09
- [x] ✅ T-030 · QĐ5 hai ngôi nhà harness/.agents + QĐ6 hệ DD/TEST mã hóa màn hình + CONTRIBUTING + docs routing · 26–27.09 · bbb99b1
- [x] ✅ T-031 · QĐ7 TODO queue + template handoff + vòng đời task + protocol đa-phiên · 27.09
- [x] ✅ T-032 · Chốt stack giữ Rust+Tauri (re-validate) · 26.09 · ea24231 · research: `techstack-analysis.md` (untracked — user: research không commit)
- [x] ✅ T-033 · Quyết định 5 "hai ngôi nhà" ghi vision · 26.09
