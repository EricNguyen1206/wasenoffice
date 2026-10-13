#!/bin/zsh
# WasenOffice verification harness — run before claiming any phase complete.
set -u
cd "$(dirname "$0")/../.."
fail=0

# Lệnh gate = HẰNG SỐ 1 nhà (gate-cmds.sh) — verify.sh và dev-check.sh cùng
# source; "local check" không thể trôi khỏi gate (sự cố 04.10: fmt thiếu
# --all pass ảo ×2).
. harness/scripts/gate-cmds.sh

# --workspace: manifest src-tauri là package + workspace root — không có flag này
# cargo mặc định chỉ chạy package gốc, crate A2 (crates/wasenoffice-ooxml, T-A20) không được phủ.
echo "▸ cargo check (src-tauri workspace)"
eval "$GATE_CARGO_CHECK" || fail=1

echo "▸ cargo fmt --check (src-tauri workspace)"
eval "$GATE_FMT" || fail=1

echo "▸ cargo clippy -D warnings (src-tauri workspace)"
eval "$GATE_CLIPPY" || fail=1

echo "▸ pnpm build (src)"
eval "$GATE_BUILD" || fail=1

echo "▸ license allowlist (cargo graph + node_modules)"
node harness/scripts/check-licenses.mjs || fail=1

echo "▸ doc OKF-thin (frontmatter type + links + index coverage)"
node harness/scripts/check-docs.mjs || fail=1

echo "▸ bridge surface 3 chiều (shim KNOWN/LOCAL_TRUTHS ↔ rust dispatch ↔ reference.map — T-044)"
node harness/scripts/check-bridge-surface.mjs || fail=1

echo "▸ layout core unit tests (gate 14: tsc-compile + node — T-V7)"
eval "$GATE_LAYOUT" || fail=1

echo "▸ smoke render behavioral (TC-D01-06: boot app, render fixture DOCX)"
harness/scripts/smoke-render.sh || fail=1

echo "▸ smoke render xlsx (TC-E01-01: boot app, render fixture XLSX — T-048)"
harness/scripts/smoke-render.sh /tmp/wasenoffice-smoke-fixture.xlsx || fail=1

echo "▸ smoke render pptx (TC-P01-01: boot app, render fixture PPTX — T-048)"
harness/scripts/smoke-render.sh /tmp/wasenoffice-smoke-fixture.pptx || fail=1

echo "▸ smoke render pdf (TC-F01-01: boot app, render fixture PDF — T-048)"
harness/scripts/smoke-render.sh /tmp/wasenoffice-smoke-fixture.pdf || fail=1

echo "▸ smoke save round-trip behavioral (TC-D01-10: edit → save → disk)"
harness/scripts/smoke-save.sh || fail=1

echo "▸ smoke save multi-format round-trip (gate 16: xlsx 257 + pptx fallback-extension — T-049)"
eval "$GATE_SAVE_MULTIFORMAT" || fail=1

echo "▸ smoke clean-machine behavioral (gate 11: open→edit→save, 0 tham chiếu ONLYOFFICE.app — T-A13)"
harness/scripts/smoke-clean-machine.sh || fail=1

echo "▸ smoke quick-view metrics (gate 12: layout asserts + <500 ms + RAM tree — T-V2)"
harness/scripts/smoke-quick-view.sh || fail=1

echo "▸ smoke quick-view UX wiring E2E (gate 13: start-page probe → viewer_view → render — T-V5)"
harness/scripts/smoke-quick-view-ui.sh || fail=1

echo "▸ smoke Nano Light editor round-trip (gate 15: edit → save write_docx → parse-back → reopen — T-A27)"
harness/scripts/smoke-nano-light.sh || fail=1

echo "▸ upstream submodules clean?"
if [ -n "$(git submodule status | grep '^[+MU]')" ]; then
  echo "  ✗ submodules dirty/different:"; git submodule status; fail=1
else
  echo "  ✓ clean"
fi

echo "▸ no upstream/engines files staged for commit?"
staged=$(git diff --cached --name-only | grep -E '^(upstream/|engines/)' | grep -v '^engines/.gitkeep' | wc -l | tr -d ' ')
if [ "$staged" != "0" ]; then echo "  ✗ $staged upstream/engines files staged"; git diff --cached --name-only | grep -E '^(upstream/|engines/)'; fail=1; else echo "  ✓ none"; fi

if [ $fail -eq 0 ]; then echo "ALL CHECKS PASSED"; else echo "CHECKS FAILED"; fi
exit $fail
