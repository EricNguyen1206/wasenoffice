# WasenOffice

**A lightweight, local-first, open-source office suite.** View and edit DOCX · XLSX · PPTX · PDF in a native Rust + Tauri shell — no account, no telemetry, 100% offline.

> **Tiếng Việt:** Bộ ứng dụng văn phòng mã nguồn mở, nhẹ và tôn trọng quyền riêng tư — soạn thảo DOCX/XLSX/PPTX/PDF bằng shell Rust + Tauri, chạy offline hoàn toàn, không telemetry.

![Status](https://img.shields.io/badge/status-alpha-orange)
![License: MIT](https://img.shields.io/badge/first--party%20code-MIT-blue)
![Platform](https://img.shields.io/badge/platform-macOS-lightgrey)

## Why WasenOffice?

- 🪶 **Light** — the Rust-native Quick View opens documents in well under 500 ms; release builds run at ~90 MB RAM, a fraction of what web-engine editors need.
- 🏠 **Local-first** — your files never leave the machine. No account, no cloud, no telemetry.
- 🔓 **Clean licensing** — all first-party code is MIT. AGPL engine components stay isolated in git submodules with full attribution ([NOTICE](NOTICE)).
- 🤖 **AI-native roadmap** — document operations exposed via MCP so AI agents can read/edit documents with a human in the loop.
- 🇻🇳 **Vietnamese-first input** — IME composition (typing diacritics while composing) is a first-class feature, not an afterthought.

## Status — alpha

WasenOffice is under active development. What works today:

| Capability | Status |
|---|---|
| Quick View — DOCX viewer, Rust-only, no engine | ✅ |
| Nano Light editing core — text, selection, tables, undo/redo, save fidelity | ✅ |
| Full engine mode (ONLYOFFICE sdkjs — default while migrating) | ✅ |
| Format conversion via bundled x2t | ✅ |
| XLSX / PPTX native editing | 🚧 roadmap |
| Windows / Linux builds | 🚧 roadmap |

Development happens fully in the open: working queue in [docs/TODO.md](docs/TODO.md), design documents per milestone in [docs/design/](docs/design/), and measured evidence in [docs/diagnostics.md](docs/diagnostics.md).

## Download

Prebuilt binaries live on the [Releases](https://github.com/EricNguyen1206/wasenoffice/releases) page (macOS DMG, ~6.4 MB). The in-app auto-updater checks the same releases.

## Build from source

Prerequisites: macOS, Rust (stable), Node.js ≥ 20 with pnpm, and an installed
ONLYOFFICE.app — the current bootstrap source for engine artifacts.

```bash
git clone --recurse-submodules https://github.com/EricNguyen1206/wasenoffice.git
cd nanooffice
pnpm install

# Engine artifacts (x2t converter, fonts, editors build) are NOT committed
# (.gitignore engines/*). Bootstrap them from an installed ONLYOFFICE.app:
bash harness/scripts/bundle-x2t.sh
bash harness/scripts/bundle-fonts.sh
# bundle-x2t.sh also expects an editors build at engines/editors —
# see the script headers for details.

pnpm tauri dev
```

Run the full verification suite: `bash harness/scripts/verify.sh`

## Architecture

```
src-tauri/                      Rust: shell, bridge, sidecar, editor sessions
├── crates/nanooffice-ooxml/    clean-room OOXML (docx) read/write crate
src/                            Frontend: start page, viewer, editor (canvas)
harness/                        Rules, skills, verification gates (verify.sh)
upstream/                       AGPL submodules (sdkjs, web-apps) — read-only
engines/                        Engine artifacts (generated, never committed)
docs/                           Design docs, diagnostics, reference map
```

## Contributing

Human and AI-agent contributors are both welcome. This repo ships a full
agent-harness contract ([AGENTS.md](AGENTS.md)), and every design decision is
documented with file-path evidence in [docs/reference.map.md](docs/reference.map.md).

## License

First-party code: **MIT** ([LICENSE](LICENSE)).
Bundled third-party components — ONLYOFFICE sdkjs / web-apps / x2t under
AGPL-3.0, fonts under OFL — are attributed in [NOTICE](NOTICE).
