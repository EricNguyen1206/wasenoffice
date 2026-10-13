---
type: research
---
# docs/techstack-analysis.md — Bun vs Golang vs Rust cho lớp shell NanoOffice

> Phân tích độc lập theo handoff 22.09 (đã dọn theo vòng đời `docs/handoff/README.md` — bản gốc
> trong git history). Ngày: 22.09.2026.
> Máy đo: macOS arm64 (darwin 25.6.0). Toolchain đã có sẵn: cargo/rustc 1.93.1, go 1.23.4, bun 1.4.0, node 22.14.
> **Phương pháp evidence-discipline:** mọi nhận định gắn nhãn **observed** (đo trên máy này, lệnh ghi kèm) /
> **candidate** (nguồn ngoài, ghi link) / **unknown** (chưa đủ dữ kiện — nói thẳng).
> Phân tích này KHÔNG đổi kiến trúc — quyết định cuối là của user.

---

## 0. Tóm tắt điều hành

**Khuyến nghị: GIỮ Rust + Tauri cho lớp A/B, chốt ngay trước PH-1; chỉ cân nhắc Bun ở lớp C (tooling) khi có nhu cầu thật (YAGNI).**

Ba lý do một dòng:
1. Trọng số sản phẩm (lightweight + multi-platform) nghiêng về Tauri: binary release đo được **5.0 MB**, cold start **342 ms (warm)**, và là **ứng viên duy nhất có mobile stable** (iOS/Android từ Tauri 2.0, 10/2024).
2. Go+Wails đo được tương đương về RAM nhưng **mobile chưa ship** (Wails v3 mới đến beta, 8/2026); **Bun không có webview host nào ở mức production** — không thể là shell lớp A.
3. Cả 3 runtime đều permissive về license nên **MIT không phải lý do đổi**; chi phí đổi khỏi Rust hôm nay còn thấp (~1.1k LOC) nhưng lợi ích thu về ≈ 0 trên mọi số đo được.

---

## 1. Phạm vi — tách bạch 3 lớp (không gộp)

Theo handoff, phân tích chỉ chấm ứng viên cho đúng lớp tương ứng:

| Lớp | Nội dung | Ứng viên có ý nghĩa? |
|---|---|---|
| **A — Shell/WebView host** | cửa sổ, tab, `nanooffice-file://` (Range), file dialog, print, updater | Rust+Tauri / Go+Wails / (Bun: **không có path production** — xem §4.4) |
| **B — Sidecar/process layer** | spawn x2t (timeout, queue, stderr-as-evidence), sau này AI runtime local | cả 3 đều làm được; process mgmt + IPC là vấn đề đã giải ở mọi ngôn ngữ |
| **C — Dev tooling/runtime** | pnpm/vite scripts, harness scripts | Bun thay Node được **bất kể** shell là gì — không được lẫn vào kết luận lớp A |

Engine (sdkjs/web-apps, AGPL) và x2t (C++, AGPL) **không đổi** dù chọn gì — tách biên qua bridge + sidecar.
Frontend TS/Vite cũng không đổi. Bẫy đã ghi trong handoff: latency app do engine JS + webview quyết định,
shell chỉ là overhead — nên các số dưới đây đo đúng **shell overhead**, không phải benchmark tổng quát.

## 2. Trạng thái code hiện tại (nền của mục migration cost)

**observed** (đếm bằng `git ls-files | xargs wc -l`, 22.09.2026):

| Thành phần | LOC | Ghi chú |
|---|---|---|
| `src-tauri/src/**` (Rust shell) | **1 141** | protocol/mod.rs 528 · protocol/filesrv.rs 290 · lib.rs 227 · bridge/mod.rs 76 · sidecar 10 · window 4 · main.rs 6 |
| `src/main.ts` + `src/shim/globals.d.ts` | 168 | gọi `window.__TAURI__.core.invoke` ở ~7 điểm |
| `src/shim/desktopEditor.js` | 450 | relay bridge → invoke Rust; dính API `__TAURI__` |

=> Đúng như handoff dự đoán: đổi ngôn ngữ shell **ngay bây giờ** thì phần viết lại còn nhỏ (xem §7).
Sau PH-0/PH-1 con số này sẽ tăng — phân tích này time-sensitive.

## 3. Decision matrix 3 ứng viên × 6 tiêu chí

Rubric: mỗi tiêu chí chấm 1–5. MIT là **gate** (đạt/không đạt, không nhân trọng số — Bun có điều kiện, ghi ở §5).
Trọng số theo vision.md (lightweight + multi-platform là tiêu chí sản phẩm mũi nhọn):
lightweight 30% · multi-platform 25% · ecosystem webview-shell 20% · agent-velocity 15% · migration cost 10%
(với migration cost: **điểm cao = chi phí chuyển thấp**; "giữ Rust" = 0 chi phí).

| Tiêu chí (weight) | Rust + Tauri v2 | Go + Wails | Bun |
|---|---|---|---|
| MIT readiness (gate) | ĐẠT — runtime MIT/Apache-2.0 [candidate] | ĐẠT — Go BSD-3-Clause, Wails MIT [candidate] | ĐẠT CÓ ĐIỀU KIỆN — Bun MIT nhưng binary compiled nhúng JSC/WebKit LGPL-2 [candidate] |
| Lightweight 30% | **5** — binary 5.0 MB, RSS tổng ~181 MB (observed) | **4** — .app 7.9 MB, RSS tổng ~198–218 MB (observed) | **2** — không có webview; runtime đơn lẻ 12 MB RSS nhưng không comparable (observed) |
| Multi-platform + mobile 25% | **5** — 5 platform, mobile stable từ 2.0 (10/2024) [candidate] | **3** — desktop OK; mobile thuộc v3, v3 mới beta [candidate] | **1** — chỉ macOS/Linux/Windows, không mobile [candidate] |
| Ecosystem webview-shell 20% | **5** — Tauri 2 stable, release hoạt động liên tục (v3.0.0-alpha.2 ngày 21/09/2026) [candidate] | **3** — Wails v2 stable nhưng backward-looking; v3 beta 02/08/2026 + nightlies 09/2026 [candidate] | **1** — không có framework production; Electrobun v2 còn thay Bun bằng runtime riêng [candidate] |
| Agent-velocity 15% | **4** — cargo check 0.66–1.65 s; release loop 68 s; clippy/rust-analyzer mạnh; borrow-check friction (observed) | **5** — go build warm 0.73 s; gopls; GC; ngôn ngữ nhỏ (observed) | **5** — bun build 0.16–0.22 s; TS nội suy với frontend (observed) |
| Migration cost 10% (cao=rẻ) | **5** — giữ nguyên, 0 chi phí | **3** — viết lại 1 141 LOC + IPC shim + gates (ước lượng 2–4 ngày agent, rủi ro thấp) | **1** — viết lại xong vẫn không có webview host → vô phương cho lớp A |
| **Điểm có trọng số (max 5)** | **4.85** | **3.60** | **1.90** |

Chi tiết bằng chứng từng tiêu chí ở §4. Cộng dồn: 0.3·w + 0.25·m + 0.2·e + 0.15·v + 0.1·c.

## 4. Bằng chứng chi tiết

### 4.1 MIT-license readiness

| Thành phần | License | Nhãn + nguồn |
|---|---|---|
| Rust (rustc/cargo) | MIT OR Apache-2.0 | candidate — [github.com/rust-lang/rust](https://github.com/rust-lang/rust) (COPYRIGHT/LICENSE file trong repo) |
| Go toolchain | BSD-3-Clause | candidate — [go.dev/LICENSE](https://go.dev/LICENSE) |
| Bun (runtime) | MIT (code Oven) **+ LGPL-2 cho JavaScriptCore/WebKit static-link** | candidate (nguồn chính) — [bun.com/docs/project/license](https://bun.com/docs/project/license): static-link LGPL-2 ⇒ phải cung cấp object format để relink; Oven công bố WebKit fork patched tại github.com/oven-sh/webkit |
| Tauri | Apache-2.0 (GitHub API) | candidate — [api.github.com/repos/tauri-apps/tauri](https://api.github.com/repos/tauri-apps/tauri) |
| Wails | MIT (GitHub API) | candidate — [api.github.com/repos/wailsapp/wails](https://api.github.com/repos/wailsapp/wails) |
| WKWebView (macOS) | system framework — **không distribute** ⇒ không phát sinh nghĩa vụ | candidate — kiến trúc macOS standard |

**Gate hiện có của repo (observed):** `harness/scripts/check-licenses.mjs` (98 dòng) đã quét **cả** cargo graph (qua `cargo metadata`) **lẫn** node_modules (kể cả layout .pnpm của pnpm). Hệ quả:

- **Bun ở lớp C:** gate node_modules hoạt động gần như nguyên vẹn — Bun dùng chung ecosystem npm + trường `license` trong package.json. Chi phí dựng gate ≈ 0.
- **Go:** phần cargo phải thay bằng tool tương đương — `google/go-licenses` có subcommand `check` exit-nonzero khi gặp license cấm/thiếu ([github.com/google/go-licenses](https://github.com/google/go-licenses), candidate). Chi phí: 1 script gọi wrapper + allowlist tương đương — nhỏ nhưng phải viết mới.
- **AGPL engine tách biên KHÔNG bị ảnh hưởng bởi ngôn ngữ shell:** engine chỉ chạm shell qua bridge + file delivery (rule gốc #4); x2t là sidecar binary không link tĩnh. Quy trình (`upstream-read-only.md`, `mit-purity.md`, gate trong verify.sh) bảo đảm điều này **bất kể** shell là Rust/Go/Bun.

### 4.2 Ultra lightweight — số đo được (observed, 22.09.2026)

Đối tượng đo — công bằng ở mức "release hợp lý":
- **Rust:** app thật của repo (1 141 LOC — cỡ hello-world + protocol/bridge thật), `cargo build --release` với profile sẵn có trong `src-tauri/Cargo.toml` (`lto=true`, `strip=true`, `codegen-units=1`, `panic="abort"`).
- **Go:** Wails v2 hello-world template `vanilla`, `wails build` (production default), tại `/tmp/techstack-spike/wailsspike`.
- **Bun:** `bun build --compile` hello.ts (HTTP server + idle); **Bun không có webview** nên không đo được app-level — đây chính là kết luận của lớp A cho Bun.

| Chỉ số | Rust+Tauri (app thật) | Go+Wails v2 (hello) | Bun (runtime, không webview) |
|---|---|---|---|
| Binary / bundle | **5.0 MB** (binary strip+lto) | **7.9 MB** (.app bundle) | 61 MB (binary compiled) |
| Cold start → window hiện | **1 220 ms** (lần 1) / **342 ms** (warm) | 1 253 ms (lần 1) / 593 ms (warm) | n/a |
| RSS idle tổng (main + WebKit processes) | **~181 MB** (main 107 MB + WebKit 73.6 MB, 3 proc) | ~198 MB (run 1) / ~218 MB (run 2); main 104–112 MB + WebKit 93–105 MB | 12–13 MB (chỉ runtime, không cửa sổ) |
| Installer | chưa đo (bundle `tauri build` cần tauri-cli, chưa cài — để PH-4) | n/a (hello) | n/a |

Lệnh + phương pháp đo (observed):
- Kích thước: `ls -lh` binary; `du -sh` .app.
- Cold start: script `/tmp/techstack-spike/measure.sh` — bấm giờ từ spawn process đến khi `osascript` System Events thấy ≥1 window của PID (độ phân giải ~100–200 ms do polling); số "warm" là lần chạy thứ 2 (dylib/WebKit cache nóng).
- RSS: `ps -o rss=` cho main PID, **cộng thêm** delta các process `com.apple.WebKit.{WebContent,Networking,GPU}` mới sinh ra sau launch (snapshot trước/sau) — WKWebView render ngoài process nên phải cộng, không thì thấp ảo.

**Đọc số đúng cách (bẫy đã nêu trong handoff):**
- Hai stack **ngang nhau practically**: chênh binary 5.0 vs 7.9 MB là vô nghĩa trước engine payload (~tens–hundreds MB); chênh RSS ~181 vs ~198–218 MB nằm trong sai số run-to-run. Phần lớn RSS của cả hai là **WebKit nội bộ macOS** — nhân tử chung, không phụ thuộc ngôn ngữ shell.
- Trần vision Quyết định 2 (<250 MB RAM idle + 1 doc; <2 s cold start): shell hiện đạt dư (~181 MB tổng **chưa tính engine**; start <0.4 s warm). Con số quyết định là **engine load thêm bao nhiêu** — đó là việc PH-0 baseline, không phải việc chọn ngôn ngữ shell.
- Bun "thắng" mọi benchmark JS-runtime nhưng ở lớp A không có thứ để đo — không có webview host.

### 4.3 Multi-platform (desktop + mobile H4)

| | Rust+Tauri | Go+Wails | Bun |
|---|---|---|---|
| macOS/Win/Linux | Có, stable | Có, stable (Wails v2) | Có (runtime) |
| iOS/Android | **Stable từ Tauri 2.0 (02/10/2024)** — "mobile operating system support" trong announcement | **Chưa ship** — thuộc Wails v3; repo có `IOS_ARCHITECTURE.md`, README không claim production mobile | **Không hỗ trợ** — docs installation chỉ liệt kê macOS/Linux/Windows |
| Nhãn + nguồn | candidate — [tauri.app/blog/tauri-20/](https://tauri.app/blog/tauri-20/) | candidate — [github.com/wailsapp/wails](https://github.com/wailsapp/wails) (README: v2 "Stable", v3 "Beta"); [v3.wails.io](https://v3.wails.io) "Wails v3 Beta" 02/08/2026 | candidate (nguồn chính) — [bun.com/docs/installation](https://bun.com/docs/installation) |

Vision Quyết định 4 đặt mobile ở H4 và phương án convert-on-desktop đã gỡ được x2t-mobile, nhưng mobile **vẫn cần shell host engine JS trong webview gốc** — Tauri là ứng viên duy nhất trong 3 có path ổn định cho việc đó hôm nay.

### 4.4 Độ chín ecosystem webview-shell (trạng thái 2026)

- **Tauri (Rust):** v2 stable; dòng release rất hoạt động — observed qua releases atom feed: `tauri v3.0.0-alpha.2` phát hành 21/09/2026 ([github.com/tauri-apps/tauri/releases](https://github.com/tauri-apps/tauri/releases)). v2 là line production của repo hiện nay (pin trong `src-tauri/Cargo.toml`). Custom protocol + updater: có sẵn (`tauri-plugin-*`); updater là việc PH-4.
- **Wails (Go):** v2 "Stable" nhưng hướng v3 là tương lai; **v3 đạt beta 02/08/2026** sau ~2 năm alpha ([v3.wails.io](https://v3.wails.io), candidate — trang SPA, xác nhận qua search + repo); nightlies liên tục giữa 09/2026 (`v3.0.0-beta.22` 14/09 → `beta.26`, releases feed — candidate). Rủi ro định lượng: v2 dừng phát triển tính năng, v3 chưa GA — chọn Go hôm nay = chọn framework đang giữa cuộc chuyển thế hệ. Spike thực tế (observed): `go install` wails CLI + `wails init` + `wails build` chạy mượt trên máy này (~vài phút, không vướng).
- **Bun:** **không có framework webview-shell production dùng Bun làm runtime.** Bằng chứng gián tiếp mạnh nhất: Electrobun — framework desktop của hệ Bun — ra v2.0 với runtime riêng "Cottontail" (Zig + JSC) và thay thế Bun/npm bằng CLI "Hutch" ([blackboard.sh/electrobun](https://blackboard.sh/electrobun), candidate); Bun bản thân chỉ target desktop 3 OS ([bun.com/docs/installation](https://bun.com/docs/installation), nguồn chính). Kết luận: lớp A bằng Bun hiện chỉ khả dụng qua Electron (nhưng Electron dùng Node/Chromium riêng, không phải Bun) hoặc FFI thủ công lên thư viện webview (không production) — **unknown/without path**.

### 4.5 Agent-velocity (đo thật trên máy này, observed)

| Vòng lặp | Rust (app thật) | Go (wails hello) | Bun (hello) |
|---|---|---|---|
| Build/check warm | `cargo check` **0.66 s**; sau khi sửa 1 file **1.65 s** | `go build` **0.73 s** | `bun build --compile` **0.16–0.22 s** |
| Release loop (1 file đổi) | `cargo build --release` **68 s** (lto=true) | — (wails build production ~phút đầu lần đầu) | ~0.2 s |
| Lint/analyze | clippy `-D warnings` (mạnh, đã trong verify.sh), rust-analyzer | `go vet`, gopls | không có typechecker riêng (dùng tsc) |
| An toàn để agent sửa | memory safety + type system chặt — sửa "an toàn" tốt nhất, đổi lại friction lúc viết | GC + đơn giản — friction thấp, dễ sơ suất concurrency nếu sau này dùng goroutine động | TS type-safety, friction thấp nhất |

Đọc: ở quy mô hiện tại (1.1k LOC) cả ba đều đủ nhanh — velocity **không phải lý do đổi**. Lưu ý duy nhất: loop release Rust chậm (68 s) do LTO — nhưng dev loop dùng `cargo check`/debug, release chỉ phục vụ đo/packaging, không nằm đường nóng của agent.

### 4.6 Migration cost nếu rời Rust — xem §7 (mục riêng theo handoff).

## 5. Trả lời THẲNG câu hỏi MIT: "ngôn ngữ nào giúp đạt MIT-app tốt nhất?"

**Không ngôn ngữ nào trong ba là yếu tố quyết định cho mục tiêu MIT — cả ba đều đạt gate; và phần khó của MIT goal không nằm ở ngôn ngữ.**

- **Thuộc ngôn ngữ/toolchain (đã kiểm chứng):**
  - Rust runtime MIT/Apache-2.0; Go BSD-3-Clause; Bun MIT — đều permissive ở tầng runtime.
  - Khác biệt duy nhất có hậu quả pháp lý: **binary compiled bởi `bun build --compile` nhúng JavaScriptCore/WebKit (LGPL-2) static-link** ⇒ khi distribute binary đó phải đáp ứng nghĩa vụ LGPL (source offer + relink opportunity) cho phần JSC. Không disqualify (Oven đã công bố WebKit fork + quy trình relink), nhưng là nghĩa vụ liên tục mà Tauri/Go không có (WKWebView là system framework — mình không distribute WebKit).
  - Ecosystem dependency: risk nằm ở dependency, không ở ngôn ngữ — repo **đã có gate cho cả cargo + npm/node** (`check-licenses.mjs`); Go cần dựng tương đương bằng `go-licenses` (nhỏ); Bun dùng lại gate node nguyên vẹn.
- **KHÔNG thuộc ngôn ngữ (đã bảo đảm ở lớp quy trình, đúng như handoff dự đoán):**
  - Engine sdkjs/web-apps và x2t là **AGPL theo code gốc** — không đổi màu vì shell viết bằng gì; chỉ clean-room rewrite mới tạo engine MIT (ngoài scope — `vision.md` Quyết định 1).
  - Biên sạch AGPL↔MIT do **kiến trúc + rule** bảo đảm: upstream chỉ đọc, bridge là hợp đồng, `mit-purity.md`, verify.sh chặn staged upstream/engines. Shell Rust/Go/Bun nào cũng sống trong khuôn khổ này.

Kết: **MIT-readiness không phải lý do để đổi shell theo hướng nào cả** — chọn theo các tiêu chí kỹ thuật còn lại.

## 6. Khuyến nghị xếp hạng

> **Giữ Rust + Tauri cho lớp A/B (xếp hạng 1); Go+Wails dự bị (2); Bun chỉ có giá trị ở lớp C tooling, không phải shell (3).**

1. **Tauri thắng ở đúng chỗ trọng số đặt vào:** lightweight đo được ngang/tốt hơn (5.0 MB, 342 ms warm, ~181 MB) và là path mobile stable duy nhất — hai tiêu chí sản phẩm nặng nhất của vision.
2. **Go+Wails là fallback hợp pháp nhưng đang giữa cuộc chuyển thế hệ** (v2 stable-looking-back, v3 beta 08/2026, mobile chưa ship) — đổi hôm nay là nhận rủi ro roadmap của framework khác để đạt… cùng một con số đo được.
3. **Bun không phải ứng viên lớp A** (không có webview host production; binary compiled còn kéo nghĩa vụ LGPL-2 của JSC) — giá trị thật duy nhất là thay Node ở lớp C, và điều đó làm được **bất kể** shell là gì, khi có nhu cầu thật (YAGNI: pnpm/node đang hoạt động, gate đã bao phủ).

## 7. Migration cost nếu rời Rust (đo tại thời điểm 22.09.2026)

Chi phí **hôm nay thấp nhất nó từng có** — và sẽ chỉ tăng sau PH-0/PH-1:

- **Phần phải viết lại (observed):**
  - 1 141 LOC Rust (`src-tauri/src/**`) — gồm protocol filesrv (Range/loopback 14321), bridge dispatch, sidecar skeleton.
  - ~7 call-site `window.__TAURI__.core.invoke` trong `src/main.ts` (120 LOC) + `src/shim/desktopEditor.js` (450 LOC) — IPC layer là API riêng của Tauri, phải thay bằng cơ chế của shell mới (Wails runtime IPC / tự dựng).
- **Harness dính Rust (observed):**
  - `verify.sh`: 3 gate cargo (check · fmt --check · clippy -D warnings) — Go: thay bằng `go vet`/`gofmt`/`golangci-lint` tương đương; Bun: tsc/eslint.
  - `check-licenses.mjs`: phần cargo metadata phải viết lại cho Go (`go-licenses check`); phần node giữ nguyên (Bun dùng lại nguyên văn).
  - `conventions.md`/`AGENTS.md` ghi "Rust là mã nguồn chính" + rule rustfmt/clippy — docs phải sửa theo.
- **Ước lượng effort:** Go+Wails: 2–4 ngày agent cho parity hiện tại (protocol/bridge/sidecar đều là việc chuẩn), **cộng** rủi ro học thêm semantics Wails (events, IPC, asset server) — ước lượng, không phải observed.
- **Bun: cost vô hạn cho lớp A** — viết lại xong vẫn không có webview host; chỉ khả dụng nếu kèm luôn quyết định "dùng Electron" (mang Chromium ~85 MB+ vào installer — phá budget 30 MB của Quyết định 2) hoặc Electrobun (runtime mới, không phải Bun). Không phải phương án cùng cấp.
- **Rủi ro phi kỹ thuật của việc đổi:** mất đường evidence PH-0 đang dở (điểm treo open pipeline nằm ở **engine/app-layer**, không phải ngôn ngữ shell — đổi shell không xử được điểm treo đó, xem `docs/diagnostics.md` 22.09).

## 8. Falsifiability — điều gì phải đúng để ứng viên thua cuộc tiếp tục thắng

Để phân tích này **sai** (và đáng đổi quyết định), một trong các điều sau phải xảy ra — kiểm lại tại cuối PH-1:

- **Rust+Tauri thua cuộc nếu:**
  1. PH-0 baseline chứng minh shell+webview Tauri chiếm quá trần sau khi engine load (RAM idle + 1 doc > 250 MB **và** nguyên nhân ở layer Tauri, không phải engine) — đo được, không phải cảm tính.
  2. Agent-velocity thành bottleneck thực: số dõi được — tỉ lệ thời gian phiên chờ compile/fix borrow-check vượt ~25% sau khi codebase > 5k LOC (ngưỡng đề xuất; ghi diagnostics khi thấy).
  3. Tauri stalled: không còn release ổn định trong 6 tháng, hoặc v3 breaking mà v2 bỏ maintenance.
- **Go+Wails thắng nếu:** Wails v3 ra GA + mobile (iOS/Android) ship thật, trong khi Tauri gặp 1 trong 3 điều trên. Khi đó chi phí chuyển theo §7 vẫn được hoàn trả bằng mobile + velocity.
- **Bun thắng nếu:** xuất hiện webview-shell production dùng Bun (hoặc Electrobun-style runtime trở nên tương thích Bun thật sự) **và** hỗ trợ mobile — theo dõi Electrobun/Cottontail định kỳ 6 tháng/lần. Riêng **lớp C**: nếu harness scripts cần single-binary tooling (không cài Node) — Bun thắng ngay hôm nay, không cần chờ.
- **Điều KHÔNG thể hợp lệ hoá việc đổi:** "benchmark tổng quát nhanh hơn" (không đo shell overhead), "license runtime đẹp hơn" (cả ba đạt gate), "ngôn ngữ xịn hơn" (không có consumer thật cho sự xịn đó — YAGNI).

## 9. Verdict (khuyến nghị cho user — KHÔNG tự thực hiện)

**CHỐT giữ nguyên kiến trúc hiện tại (Rust + Tauri v2 shell, engine AGPL tách biên, x2t sidecar), đóng câu hỏi techstack trước khi vào PH-1.** Không mở phiến "đổi shell"; không dựng song song Wails/Bun spike trong repo. Việc duy nhất đáng lấy ra từ phân tích này: ghi vào backlog PH-4 kiểm chứng `tauri build` bundler (installer <30 MB) và đưa baseline size/RAM/start vào CI gate như vision đã định — đó là nơi các con số Lightweight thực sự bị thử thách, chứ không phải ở việc chọn ngôn ngữ shell.

---

### Phụ lục A — Lệnh đo gốc (observed, tái lập được)

```bash
# LOC
git ls-files 'src-tauri/src/*.rs' 'src-tauri/src/**/*.rs' | xargs wc -l   # = 1141

# Rust release (app thật)
cd src-tauri && cargo build --release          # binary: target/release/nanooffice = 5.0 MB
# Go/Wails (trong /tmp)
go install github.com/wailsapp/wails/v2/cmd/wails@latest
wails init -n wailsspike -t vanilla && cd wailsspike && wails build   # .app = 7.9 MB
# Bun
bun build --compile hello.ts --outfile hello                          # = 61 MB
# Cold start + RSS: /tmp/techstack-spike/measure.sh <binary> <proc-name>
#   — spawn → poll osascript System Events "count windows of (first process whose unix id is PID)"
#   — RSS: ps -o rss= (main) + delta các process com.apple.WebKit.* trước/sau launch
```

Số liệu đo đầy đủ: §4.2 + §4.5. Script đo nằm ở `/tmp/techstack-spike/` (ngoài repo, đúng ràng buộc).

### Phụ lục B — Danh sách nguồn ngoài (candidate)

- Tauri 2.0 stable + mobile: <https://tauri.app/blog/tauri-20/>
- Tauri release hoạt động (v3.0.0-alpha.2, 21/09/2026): <https://github.com/tauri-apps/tauri/releases>
- Tauri license: <https://api.github.com/repos/tauri-apps/tauri> (Apache-2.0)
- Wails repo (v2 "Stable", v3 "Beta", IOS_ARCHITECTURE.md): <https://github.com/wailsapp/wails>
- Wails v3 Beta announcement (02/08/2026): <https://v3.wails.io>
- Wails v3 nightlies 09/2026: <https://github.com/wailsapp/wails/releases>
- Wails license: <https://api.github.com/repos/wailsapp/wails> (MIT)
- Bun license (MIT + LGPL-2 JSC static-link): <https://bun.com/docs/project/license>
- Bun platforms (macOS/Linux/Windows, không mobile): <https://bun.com/docs/installation>
- Electrobun (desktop-only, v2 thay Bun bằng runtime Cottontail): <https://blackboard.sh/electrobun/>
- go-licenses (gate license cho Go modules): <https://github.com/google/go-licenses>
- Rust license (MIT OR Apache-2.0): <https://github.com/rust-lang/rust>
- Go license (BSD-3-Clause): <https://go.dev/LICENSE>

### Phụ lục C — those unknown / không đo được

- **Installer size của Tauri** (targets "all"): chưa đo — cần `tauri-cli`/`npm run tauri build`; để PH-4 theo đúng scope. Binary 5.0 MB cho thấy dư địa <30 MB là rộng.
- **RSS/cold start của Bun-as-shell:** không tồn tại path webview nên không có gì để đo (không phải thất bại kỹ thuật của spike).
- **Đo cold-start có độ phân giải ~100–200 ms** (polling osascript) — đủ kết luận với target <2 s, không đủ cho so-sánh tinh hơn giữa hai stack.
- **Wails v3 ngày "beta" trích từ v3.wails.io qua search** (trang SPA fetch trực tiếp bị 404) — ngày 02/08/2026 là candidate, chỉ dùng cho mốc thời gian, không cho nhận định kỹ thuật nào khác.

---

## Phụ lục D — Điều tra frontend sang WASM/Bun (29.09.2026, research — không commit)

**Câu hỏi:** phần frontend NanoOffice chuyển sang WebAssembly hoặc Bun được không, giữ multi-platform? Dư địa tối ưu tới đâu?

**Phân rã "frontend" (đo thật 29.09):** source 1.211 dòng (index.html 14 + main.ts 135 + styles 174 + shim/delivery ~890) · **built = 24 KB** (16 KB JS + 1.9 KB CSS trong `dist/`) · so với engines/ 313 MB payload + ~370 MB RAM WebContent khi mở doc · tỷ lệ frontend ≈ **0.008%** khối lượng shipped.

**3 phương án khả diễn:**

| Phương án | Khả thi? | Multi-platform? | Gain |
|---|---|---|---|
| **A. Bun làm runtime frontend trong webview** | **Không** — WKWebView/WebView2/WebKitGTK dùng JS engine riêng (JSCore/V8), không nhúng runtime ngoài được. "Bun webview runtime" không tồn tại (search 09/2026). Bun chỉ chạy được tiến trình ngoài như node hôm nay. | ❌ | — |
| **B. Bun thay vite (build tooling)** | Khả thi — bundler Bun nhanh hơn (~35% build), nhưng 2026 vẫn **chưa phải drop-in Vite replacement** (dev server/HMR thua; Vite còn đang chuyển sang Rolldown-Rust nâng chuẩn tiếp). Chỉ chạm dev tooling — `node_modules` 67 MB là dev-only, không ship. | ✅ (dev) | ≈ 0 với product; repo này build ~5 s |
| **C. Shim/start-page compile Rust→WASM chạy trong webview** | Khả thi kỹ thuật (cả 3 webview hỗ trợ WASM; JS→wasm export call là sync nên LOCAL_TRUTHS sync-truth giữ được). Nhưng shim là glue I/O-bound (~16 KB) không nằm trên đường đo nào (bridge call ~ms, do IPC/engine); wasm thao tác window/DOM vẫn cần JS glue (web-sys) — không bỏ được lớp JS vì engine cần thấy object `window.AscDesktopEditor`. | ✅ | ≈ 0 |

**Dư địa tối ưu thật (số đã đo trong repo):** T-040 engines trim 313 MB → ≤100 MB (tiết kiệm ~213 MB ≈ **9.000×** toàn bộ frontend built) · RAM ~370 MB WebContent = engine, không trim được · cold start 3.4 s = engine boot (parse 16 KB JS không đo nổi). → **Kết luận: KHÔNG có bằng chứng mới đủ mở lại QĐ stack (vision, đóng 26.09). Layer frontend giữ TypeScript/JS + Vite.** Nếu PH-4 cần build nhanh hơn, B là opt-in tại chỗ (đổi beforeBuildCommand), không phải quyết định kiến trúc.

Nguồn: pkgpulse so sánh Bun-vs-Vite 05/2026 · vallettasoftware review · JavaScript 2026 overview (Vite → Rolldown).
