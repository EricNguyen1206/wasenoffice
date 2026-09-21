# docs/conventions.md — quy ước NanoOffice

## Kiến trúc mục tiêu (đã chốt theo feasibility study 21.09.2026)

Lộ trình A: **Tauri shell + engine gốc + x2t sidecar.**

```
src-tauri/ (Rust — mã nguồn chính)
  ├─ bridge/        shim window.AscDesktopEditor: command enum + handlers
  ├─ protocol/      custom protocol nanooffice-file:// (Range support)
  ├─ sidecar/       quản lý x2t (spawn, timeout, queue)
  ├─ window/        cửa sổ, tab, state
  └─ main.rs
src/ (frontend Tauri — KHÔNG chứa engine)
  ├─ startpage/     start page thay loginpage upstream (JS riêng của mình)
  └─ shim/          inject bridge JS vào WebView trước khi engine load
engines/            artifact build sdkjs/web-apps + x2t binary (không edit tay)
upstream/           submodule chỉ đọc
```

## Quy ước code

- **Rust**: edition 2021+, `cargo fmt` + `cargo clippy` sạch trước commit; error handling dùng
  `thiserror` cho lib-layer, `anyhow` chỉ ở main; không `unwrap()` trong handler liên quan UI.
- **TypeScript/JS (src/)**: đặt tên shim khớp method bridge 1-1; mỗi shim module export đúng một
  nhóm chức năng (fileOps, print, clipboard...).
- **Tên method bridge**: giữ NGUYÊN VĂN tên upstream (`IsLocalFile`, `LocalFileSave`...) — không
  rename sang convention riêng, giúp đối chiếu call-sites và rebase đỡ đau.
- **Không** hardcode đường đĩa trong engine-facing code; mọi đường file đi qua fileId + custom protocol.

## Git

- Branch chính: `main`. Nhánh làm việc: `feat/<phase>-<mô tả>` (vd `feat/ph0-spike-editor-load`).
- Commit message: `phase(scope): mô tả` (vd `ph0(bridge): stub file-dialog methods`).
- upstream submodule cập nhật là commit riêng, không trộn với commit code.

## Workflow kỹ năng (skills) — bắt buộc theo loại việc

Skills project nằm ở `.agents/skills/` (lock: `skills-lock.json`; cập nhật: `npx skills update -p`).
Chỉ dùng set tối thiểu — thêm skill mới phải qua acceptance giống dependency (YAGNI).

| Loại việc | Skill |
|---|---|
| Mọi việc viết/sửa code | `ponytail` — ladder YAGNI: cần tồn tại? → có sẵn trong codebase? → stdlib? → native platform? → dep đã cài? → 1 dòng? → mới viết tối thiểu |
| Module/layer mới | `codebase-design` + đọc `harness/rules/mit-purity.md` trước khi viết |
| Trước khi tuyên bố xong | `verification-before-completion` — bằng chứng chạy thật trước khi claim (khớp rule gốc #5) |
| Trước khi commit | `code-review` trên diff; diff lớn/nghi over-engineering → thêm `ponytail-review` |
| Định kỳ (cuối mỗi phase) | `ponytail-audit` — quét cả repo tìm bloat, kết quả vào backlog phase kế |

Spawn subagent → prompt PHẢI nhúng/ trỏ tới `harness/rules/mit-purity.md` (subagent không tự đọc AGENTS.md).

## Chế độ vận hành: đơn nhiệm (chốt bởi user 21.09.2026)

- **KHÔNG dùng git worktree, không song song hóa task.** Mỗi phiên = 1 task, chạy thẳng đến xong.
  Ưu tiên: tốc độ hoàn thành 1 task + hạn chế sai sót (checklist + rule + code review), không ưu
  tiên khả năng đa nhiệm.
- Checklist bắt buộc mỗi task:
  - Trước khi làm: đọc `vision.md` + `conventions.md`; check `harness/skills/` + `.agents/skills/`
    có skill tương ứng không.
  - Trong khi làm: ladder YAGNI (skill `ponytail`) trước mỗi file/dependency mới; không mở front
    mới khi front cũ chưa xong; subagent prompt phải nhúng `mit-purity.md`.
  - Trước khi báo xong: `verify.sh` exit 0 → `code-review` trên diff (thêm `ponytail-review` nếu
    diff lớn) → bằng chứng + lệnh kiểm chứng + exit code trong báo cáo → phát hiện mới về upstream
    ghi `reference.map.md` / quyết định mới ghi `vision.md` / sự cố ghi `diagnostics.md`.
  - **System-1 accounting** (bắt buộc khi task có gọi `quick_decision_router`): đếm từ các
    response `routing` trong phiên, báo đúng 1 dòng cuối report:
    `System-1: X calls · Y auto · Z escalated → main-agent · tiết kiệm ~Y×300 tokens (ước tính
    bảo thủ theo quy ước; System-1 chạy local = 0 token cloud)`.
    Không gọi lần nào → ghi `System-1: 0 calls`. KHÔNG thổi số: escalated không tính là tiết kiệm.

## Công cụ cục bộ hỗ trợ agent

- **System-1 decisions** — entry point cho agent là **`quick_decision_router`**
  (`harness/system1/`, `localhost:11437`): `POST /evaluate` (state + questions
  choice/score/noul, payload 100% tiếng Anh). Engine phía sau: SemIf direct logit readout
  (`harness/semif_engine/`, MiniCPM5-2B trên llama-server, ~95ms/3-câu warm, 0 output token).
  Cascade: `confidence < 0.85` (config `SYSTEM1_CONF_THRESHOLD`) → `escalated=true` → Main Agent
  tự reason câu đó. Lựa chọn SemIf thay Laya đã đo bằng benchmark 22.09.2026 — bảng số liệu
  trong `harness/system1/README.md`. Start `harness/system1/start.sh`, smoke test `smoke_test.sh`.
  **CẤM** dùng cho: kiến trúc, license, review/viết code.
- `quick-decide.sh` — free-form Q&A qua model local (ít dùng; ưu tiên decision engine vì có
  constraint + confidence).

## Kiểm chứng mỗi vòng

- Chạy `./harness/scripts/verify.sh` — gồm: cargo check · cargo fmt --check · cargo clippy
  (-D warnings) · pnpm build · license gate (`check-licenses.mjs`, allowlist trong script) ·
  submodules clean · không file `upstream/`/`engines/` nào staged.
- Có thay đổi bridge/protocol → chạy smoke: mở app, load editor, log các method UNIMPL bị gọi.

## Định nghĩa các phase (ngắn gọn)

- PH-0: Tauri trống load documenteditor + render DOCX trong WKWebView (checkpoint go/no-go).
- PH-1: CRUD file end-to-end (open/save/convert) qua x2t sidecar.
- PH-2: print/clipboard/fonts/tabs/file-association.
- PH-3: crypto, signature, cloud (scope-cut được).
- PH-4: đóng gói + updater.
