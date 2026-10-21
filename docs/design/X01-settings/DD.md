---
type: design
---
# Detail Design (DD) — X01 · Cài đặt (settings) — view trong start page

| Trường | Giá trị |
|---|---|
| Mã màn hình | `X01` (nhóm X — cài đặt) |
| Tên màn hình | `settings` (in-page view trên start page A01) |
| Mã cha | A01 (sidebar item "Settings") |
| Trạng thái | 🟨 in-development |
| Phase | Track A (A5, T-A46) |
| Cập nhật gần nhất | 2026-10-21 |

## 1. Quyết định kiến trúc (K02-Q3 amended, T-A46 — user chỉ thị 20.10)

Thay vì cửa sổ riêng theme trắng (bản K02 v1 — `settings.html`/`settings.ts` + command
`open_settings`, đã XOÁ), Settings là **view trong start page**: bấm item sidebar
"Settings" (hoặc ⌘, / gear ⚙) → vùng nội dung chính chuyển sang Settings, sidebar đứng
yên với item active — đúng pattern ONLYOFFICE, một UI system (theme Graphite chung).

## 2. Kiến trúc hiện hành (21.10 — 3 view)

`main.ts` `setActiveView: 'home' | 'settings' | 'ai'` — ba section anh em trong
`#welcome`: `#view-home` (Welcome + Recents) · `#settings-view` (`src/settings-view.ts`)
· `#ai-view` (`src/ai-view.ts`). Sidebar active state đổi theo view; Esc không-capture
trong Settings = về Home; ✕ Đóng = về Home.

- **Settings view** (`src/settings-view.ts`, VI-only): tab **Phím tắt** (13 hàng keymap +
  capture chord + confirm "Đè" + reset ↺ + "Khôi phục tất cả" + Esc hủy capture — giữ
  nguyên 100% hành vi K02 S2; capture đang arm thì shell keydown nhường — `isCapturing()`)
  và tab **Account & License** (version qua `getVersion` + MIT + MCP protocol).
- **AI view** (`src/ai-view.ts`, 21.10): nội dung **MCP** chuyển đến đây — "AI Agent &
  MCP Tools" (kết nối agent ngoài; WasenOffice = MCP Server) — tách khỏi Settings để
  sidebar "AI agent" có đích trung thực (hướng grill AI-native 03.10: agent nằm hệ ngoài,
  cắm qua MCP).

## 3. Ceilings

⌘,/gear chỉ ăn khi main-frame focus (engine sở hữu phím khi soạn thảo — như K02 §shell);
Settings VI-only; trong web dev mode (T-068) view chạy qua HTTP bridge đầy đủ.

## 4. Kiểm thử

Gate 17-settings (`STARTPAGE_PROBE=settings`, verify.sh): probe → sidebar Settings →
log `settings view: keymap=N rows mcp=ok` (hằng `SMOKE_RE_SP_SETTINGS`, BRE-safe).
E2E browser-use 21.10: capture ⌘K hiện đúng, Esc hủy, ✕ Đóng về Home (diagnostics).
