#!/bin/bash
# link-dev-engines.sh — unblock MÁY DEV cho app ĐÃ CÀI (/Applications): engines/
# hiện chưa bundle vào .app (T-066, PH-4) nên bản cài tìm không thấy sdkjs/web-apps
# + fonts (GUI launch cwd=/ — diagnostics 17.10). Code `engines_root()` đã walk
# exe-ancestors, nên symlink `Contents/MacOS/engines` -> repo engines là đủ.
# SYMLINK BỊ MẤT KHI CÀI ĐÈ APP — chạy lại script này sau mỗi lần cài mới.
# Distribution thật (người dùng cuối) phải đợi T-066 bundle engines vào bundle.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
APP="/Applications/WasenOffice.app"
LINK_TARGET="$APP/Contents/MacOS/engines"

[ -d "$ROOT/engines/editors/sdkjs" ] || { echo "link-dev-engines: $ROOT/engines/editors/sdkjs không tồn tại — bundle engines trước (bundle-x2t.sh/bundle-fonts.sh)"; exit 1; }
[ -d "$APP" ] || { echo "link-dev-engines: $APP chưa được cài"; exit 1; }

ln -sfn "$ROOT/engines" "$LINK_TARGET"
[ -e "$LINK_TARGET/editors/sdkjs" ] || { echo "link-dev-engines: symlink không resolve — kiểm tra lại"; exit 1; }
echo "link-dev-engines: OK — $LINK_TARGET -> $ROOT/engines"

# Clear quarantine/provenance and ad-hoc sign to prevent SIGKILL (Code Signature Invalid) on Apple Silicon
xattr -cr "$APP" 2>/dev/null || true
codesign --force --sign - "$APP/Contents/MacOS/wasenoffice" 2>/dev/null || true
codesign --force --sign - "$APP" 2>/dev/null || true
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$APP" 2>/dev/null || true
echo "link-dev-engines: Codesigned & registered $APP"
