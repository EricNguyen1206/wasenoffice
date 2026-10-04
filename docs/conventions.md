---
type: convention
---
# docs/conventions.md — quy ước NanoOffice

## Kiến trúc mục tiêu (đã chốt theo feasibility study 21.09.2026)

Lộ trình A: **Tauri shell + engine gốc + x2t sidecar.**
(QĐ9 29.09.2026: "Lộ trình A" = kiến trúc shell ban đầu, giữ nguyên; **Track A** trong
`docs/TODO.md` = thang tự chủ mã nguồn — thay từng module engine bằng clean-room
(`harness/rules/clean-room.md`), shell/bridge không đổi. Hai khái niệm khác nhau.)

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

Skills import (generic, từ marketplace) nằm ở `.agents/skills/` — path do skills.sh CLI dictate (lock:
`skills-lock.json`; cập nhật: `npx skills update -p`). Quy trình RIÊNG của project nằm ở `harness/skills/`
(contract, đọc-by-path). Hai ngôi nhà khác seam — không gộp (xem `vision.md` Quyết định 5).
Chỉ dùng set tối thiểu — thêm skill mới phải qua acceptance giống dependency (YAGNI).

| Loại việc | Skill |
|---|---|
| Mọi việc viết/sửa code | `ponytail` — ladder YAGNI: cần tồn tại? → có sẵn trong codebase? → stdlib? → native platform? → dep đã cài? → 1 dòng? → mới viết tối thiểu |
| Brainstorm / đào sâu hướng giải quyết | `grilling` (user gọi `/grill-me`) — phỏng vấn design-tree theo vòng: mỗi round hỏi cả frontier câu hỏi khả dĩ + kèm đáp án đề xuất, sub-agent tự tra facts; xong khi frontier rỗng (không còn gì ngầm định) |
| Module/layer mới | `codebase-design` + đọc `harness/rules/mit-purity.md` trước khi viết |
| Trước khi tuyên bố xong | `verification-before-completion` — bằng chứng chạy thật trước khi claim (khớp rule gốc #5) |
| Trước khi commit | `code-review` trên diff; diff lớn/nghi over-engineering → thêm `ponytail-review` |
| Định kỳ (cuối mỗi phase) | Hai phần: **"gọn"** → `ponytail-audit` quét repo tìm bloat; **"không loạn"** → `/improve-codebase-architecture` soi hot-spot theo git history, đề xuất deepening (report HTML ở /tmp, không vào repo; user chọn cái nào mới đào). Đối chiếu quyết định cũ với `vision.md` (vai trò ADR) + `reference.map.md` (vai trò CONTEXT.md) — không re-litigate. Kết quả vào backlog phase kế; chỉ refactor khi có consumer thật |

Spawn subagent → prompt PHẢI nhúng/ trỏ tới `harness/rules/mit-purity.md` (subagent không tự đọc AGENTS.md).

## Vòng đời task — từ brainstorm đến đóng (6 sân khấu, 5 cổng)

0. **Backlog** — ý tưởng → 1 dòng trong `docs/TODO.md` (section đúng phase hoặc META).
1. **Quyết định** — nhỏ/lặp: `quick_decision_router`; có tầm: `/grill-me`. **G1:** QĐ ghi
   `vision.md` + entry `diagnostics.md` trước khi code.
2. **Spec** — task nhỏ: done-criteria 3–6 dòng ngay trong dòng TODO; task vừa/lớn/mở phase:
   `docs/handoff/<tên>.md` theo `docs/handoff/_TEMPLATE.md`. **G2:** done-criteria measurable
   tồn tại trước khi code.
3. **Code** — ponytail ladder trước mỗi file/dep mới · skill theo loại việc · subagent prompt
   nhúng `mit-purity.md` · upstream read-only · commit `phase(scope):` từng unit chạy được.
4. **Verify** — **G3:** `verify.sh` exit 0 (+ smoke bridge/protocol theo DD-TEST màn) +
   `code-review` diff (+ `ponytail-review` nếu lớn) · report kèm lệnh + output + exit code.
5. **Ghi & đóng** — **G4:** phát hiện về đúng nhà (reference.map / diagnostics / vision) ·
   **G5:** cập nhật TODO (✅ + commit hash, sink đáy) + xoá handoff của task (nếu có — quy tắc
   `docs/handoff/README.md`) + AGENTS Progress snapshot khi xong phase.

Cuối phase: cặp review định kỳ (ponytail-audit + `/improve-codebase-architecture`) → kết quả
vào backlog phase kế + sinh handoff phase kế.

## Tài liệu thiết kế màn hình — DD + TEST (QĐ6)

- Mỗi màn hình có 1 thư mục `docs/design/[Mã]-[tên]/` gồm `DD.md` (thiết kế chi tiết) + `TEST.md`
  (kế hoạch kiểm thử). Mã màn `[Chữ][2 số]`, màn con `-n` (vd `D01`, `D01-1`) — bảng cấp chữ +
  quy tắc cấp mã: `docs/design/README.md` (registry). Template: `docs/design/_TEMPLATE-*.md`.
- **Đổi hành vi màn hình → cập nhật DD + TEST cùng commit với code.** Màn mới → đăng ký
  registry (cấp mã) trước khi viết spec.
- TEST là điều kiện sign-off màn hình: chỉ tính xong khi mọi case P0 GREEN (RED phải có lý do +
  câu hỏi mở trong DD §10).
- Định tuyến mọi loại tài liệu: `docs/README.md` (ngôi nhà duy nhất của chỉ mục).
- Ngôn ngữ tài liệu: tiếng Việt (public official — QĐ6).

## Chuẩn tài liệu OKF-thin (QĐ8)

- Mọi file `docs/**/*.md` phải có frontmatter YAML với **khóa bắt buộc duy nhất `type`** thuộc
  vocabulary cố định dưới đây — khóa định tuyến máy đọc được (chuẩn Open Knowledge Format v0.2
  của Google, áp dạng thin — `docs/vision.md` QĐ8).

| type | Nghĩa | Ví dụ |
|---|---|---|
| `decision` | quyết định đã chốt (ADR) | `vision.md` |
| `convention` | quy ước / quy tắc | `conventions.md` |
| `evidence` | bản đồ bằng chứng | `reference.map.md` |
| `log` | nhật ký theo ngày, mới trên cùng | `diagnostics.md` |
| `design` | DD màn hình | `design/D01-editor/DD.md` |
| `test` | test plan màn hình | `design/D01-editor/TEST.md` |
| `template` | khuôn để copy | `design/_TEMPLATE-DD.md` |
| `queue` | hàng đợi công việc | `TODO.md` |
| `handoff` | prompt bàn giao phiên | `handoff/_TEMPLATE.md` |
| `registry` | chỉ mục / danh mục | `docs/README.md`, `design/README.md` |
| `research` | research một lần, không duy trì | `techstack-analysis.md` |

- **Ràng buộc cho agent sinh doc mới:** thêm frontmatter `type` đúng vocabulary + đăng ký dòng
  index trong `docs/README.md` — không bước nào khác; gate `harness/scripts/check-docs.mjs`
  (chạy trong `verify.sh`) chặn: thiếu type, type lạ, link nội bộ gãy, file chưa index.
- Template khi copy: **đổi `type: template`** thành type thật (`design` / `test` / `handoff`).
- KHÔNG áp full OKF: không đổi tên `README.md`→`index.md` hay `diagnostics.md`→`log.md`, không
  thêm metadata đầy đủ / attestation — trigger nâng cấp ghi ở QĐ8.

## Chế độ vận hành: đơn nhiệm (chốt bởi user 21.09.2026)

- **KHÔNG dùng git worktree, không song song hóa task.** Mỗi phiên = 1 task, chạy thẳng đến xong.
  Ưu tiên: tốc độ hoàn thành 1 task + hạn chế sai sót (checklist + rule + code review), không ưu
  tiên khả năng đa nhiệm.
- Checklist bắt buộc mỗi task:
  - Trước khi làm: đọc `vision.md` + `conventions.md`; check `harness/skills/` + `.agents/skills/`
    có skill tương ứng không; **claim task trong `docs/TODO.md`** (🟨 + ngày).
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

### Handoff vào/ra giữa các phiên

- Viết / ghi tiếp / xoá theo vòng đời trong `docs/handoff/README.md` (1 task đang mở ≤ 1 handoff).
- Cuối phase (hoặc mở task vừa/lớn) → sinh `docs/handoff/<tên>.md` theo `docs/handoff/_TEMPLATE.md`.
- Phiên dở giữa chừng → **ghi tiếp vào handoff hiện có** (trạng thái, việc còn lại), không mở
  file mới — state không được sống duy nhất trong chat.
- Task/phase đóng → xoá handoff của task đó **trong cùng commit đóng việc**.

### Protocol đa-phiên (nhiều agent trên một cây làm việc)

- `git add` **scoped theo file**, tuyệt đối không `git add -A` / `git add .`.
- Trước khi verify/commit → chụp `git status`; thấy thay đổi lạ (không do mình) → inspect
  mtime/diff, **báo user**, không tự commit/revert giùm.
- Không chạy hai agent ghi cùng một lúc trên cùng thư mục làm việc.

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

- Chạy `./harness/scripts/dev-check.sh` TRƯỚC khi tuyên bố xong một slice — các check
  nhanh (~15 s, KHÔNG boot app): lệnh source từ `harness/scripts/gate-cmds.sh` — HẰNG SỐ
  1 nhà mà `verify.sh` cũng dùng; **không tự ghép lệnh check riêng** (04.10: fmt thiếu
  `--all` pass ảo ×2, `cargo check -p` một package sót initializer shell crate).
- Chạy `./harness/scripts/verify.sh` — gồm: cargo check · cargo fmt --check · cargo clippy
  (-D warnings) · pnpm build · license gate (`check-licenses.mjs`, allowlist trong script) ·
  doc gate OKF-thin (`check-docs.mjs` — QĐ8) · submodules clean · không file `upstream/`/`engines/`
  nào staged. Phán quan cuối — dev-check chỉ là pré-flight.
- Có thay đổi bridge/protocol → chạy smoke: mở app, load editor, log các method UNIMPL bị gọi
  (thủ tục chuẩn hóa: `docs/design/D01-editor/TEST.md`).

## Vận hành agent — kỷ luật từ tự-review 04.10 (T-055 #6 + T-V6)

- **Edit: `old_string` copy từ file thật** — tuyệt đối không gõ lại từ trí nhớ hay từ
  output đọc bị elide (04.10: 2 lần edit làm hỏng body hàm — `render.ts`,
  `opc.rs` — cùng một root cause).
- **Đổi model Rust → `cargo check` TOÀN workspace** (hoặc `dev-check.sh`) trước gate
  nào khác — initializer thiếu nằm ở crate KHÁC nơi đổi (04.10: `Paragraph.spacing`
  lộ E0063 ở `editor/mutations.rs` tại gate 15, không phải crate).
- **Commit sau `git add` danh sách tay → `git status --short` soi file lạ** — tree dùng
  chung giữa các phiên; chỉ stage hunks của mình (04.10: session song song để
  work-in-progress trong cùng tree).
- **`harness/fixtures/corpus-a2/NOTES.md` là spec đầu vào của implement** — đọc trước
  khi viết code, không chỉ sau khi parity fail (04.10: lowerLetter/lowerRoman ghi sẵn
  trong NOTES nhưng mark implement chỉ decimal → parity FAIL ×3 hard).
- **parity-view chạy không deadline** (đo 97 s → 2506 s tùy tranh chấp máy; mỗi corpus
  boot app riêng).

## Định nghĩa các phase (ngắn gọn)

- PH-0: Tauri trống load documenteditor + render DOCX trong WKWebView (checkpoint go/no-go).
- PH-1: CRUD file end-to-end (open/save/convert) qua x2t sidecar.
- PH-2: print/clipboard/fonts/tabs/file-association.
- PH-3: crypto, signature, cloud (scope-cut được).
- PH-4: đóng gói + updater.
