---
type: queue
---
# docs/TODO-archive.md — kho ✅ của các phase đã đóng (QĐ7)

> Khi đóng phase, toàn bộ dòng ✅ của section đó được cắt từ `docs/TODO.md` sang đây
> (giữ nguyên text + commit hash). Chỉ đọc — khảo cổ git history khi cần; không tái dùng ID `T-n`.

## PH-0 — Spike editor + render DOCX (go/no-go) — đóng 27.09.2026, verdict GO WITH CAVEAT

> Caveat của verdict (criterion b — scroll/edit keyboard): chờ user xác nhận thủ công ~30s,
> bằng chứng và lý do ở `docs/diagnostics.md` entry 27.09 (T-003). Baseline release đo ở PH-4.

- [x] ✅ T-004 · Engine boot WKWebView không CEF · 22.09
- [x] ✅ T-005 · Bridge first wave 15+ method DONE-PH0 · 22.09
- [x] ✅ T-006 · LoadJS bootstrap contract + wasm locateFile fix · 22.09
- [x] ✅ T-001 · Chọn phương án phá blocker render · 27.09 · user ủy quyền → **(a) oracle-diff CDP** · triển khai trong T-002
- [x] ✅ T-002 · Triển khai (a) đến khi render qua vạch — **TC-D01-06 GREEN** (contentReady + viewer 2302×958 + 14 fetch `/fonts`, Hcc=15 khớp oracle app thật) · 27.09 · 14ee92c · diagnostics 27.09
- [x] ✅ T-003 · Verdict go/no-go PH-0 + baseline size/RAM/cold-start ghi diagnostics + kiểm tra scroll/edit cơ bản theo DoD handoff PH-0 · 27.09 · bd2bae8 · **GO WITH CAVEAT** — (a) gate 9 exit 0 · (b) UNTESTABLE-SAFELY khi đo (GUI user chiếm dụng live + `click at` bị assistive-access chặn) → user xác nhận thủ công 30s · (c) baseline debug: binary 29 MB, engines 313 MB, RAM idle ~203 MB / 1 doc ~561–579 MB, cold start 3.37–3.62 s · (d) không phá giả định WKWebView — diagnostics 27.09
