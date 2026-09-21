#!/bin/zsh
# Chuẩn bị model GGUF cho SemIf engine (llama-server).
# Ưu tiên tái dùng blob GGUF mà Ollama đã tải (không download lại); fallback: tải từ HF.
# Model: openbmb/MiniCPM5-2B GGUF Q4_K_M (chính là model SemIf dùng trong browser ladder).
set -euo pipefail
cd "$(dirname "$0")"
MODELS_DIR="models"
MANIFEST="$HOME/.ollama/models/manifests/hf.co/openbmb/MiniCPM5-2B-GGUF/Q4_K_M"
HF_URL="https://huggingface.co/openbmb/MiniCPM5-2B-GGUF/resolve/main/MiniCPM5-2B-Q4_K_M.gguf"

mkdir -p "$MODELS_DIR"
TARGET="$MODELS_DIR/MiniCPM5-2B-Q4_K_M.gguf"

if [ -s "$TARGET" ]; then
  echo "model sẵn sàng: $TARGET ($(du -h "$TARGET" | cut -f1 | tr -d ' '))"
  exit 0
fi

# 1) Thử tái dùng blob Ollama: manifest → layer media_type image.model → blob sha256
if [ -f "$MANIFEST" ]; then
  BLOB_NAME=$(jq -r '.layers[] | select(.media_type == "application/vnd.ollama.image.model") | .digest' "$MANIFEST" | head -1)
  if [ -n "$BLOB_NAME" ] && [ -f "$HOME/.ollama/models/blobs/$BLOB_NAME" ]; then
    BLOB="$HOME/.ollama/models/blobs/$BLOB_NAME"
    # symlink cho nhẹ disk (blob Ollama là GGUF thuần)
    ln -sf "$BLOB" "$TARGET"
    echo "reused ollama blob → $TARGET ($(du -h "$BLOB" | cut -f1 | tr -d ' '))"
    exit 0
  fi
fi

# 2) Fallback: tải thẳng từ Hugging Face
echo "không tìm thấy blob ollama — tải từ HF (1.6 GB)..."
curl -L --fail --progress-bar "$HF_URL" -o "$TARGET"
echo "downloaded → $TARGET ($(du -h "$TARGET" | cut -f1 | tr -d ' '))"
