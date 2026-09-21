#!/bin/zsh
# quick-decide — gọi model local (Ollama) cho quyết định cơ học rẻ token.
# Dùng cho: phân loại, trích xuất, tóm tắt, đặt tên, routing (xem conventions.md § Công cụ cục bộ).
# KHÔNG dùng cho: kiến trúc, license, review/viết code — việc đó thuộc model chính.
# Lưu ý: MiniCPM5 là reasoning model — phải think:false, nếu không content bị rỗng vì cạn num_predict.
set -euo pipefail
MODEL="${QUICK_MODEL:-hf.co/openbmb/MiniCPM5-2B-GGUF:Q4_K_M}"
PROMPT="${1:-}"
[ -z "$PROMPT" ] && PROMPT="$(cat)"
[ -z "$PROMPT" ] && { echo "usage: quick-decide.sh \"<câu hỏi>\" (hoặc pipe nội dung qua stdin)" >&2; exit 2; }

curl -sS --max-time 120 http://localhost:11434/api/chat \
  -H 'Content-Type: application/json' \
  -d "$(jq -n --arg m "$MODEL" --arg p "$PROMPT" '{
    model: $m,
    messages: [
      {role: "system", content: "You answer briefly, max 100 words, no preamble, no markdown unless asked. If unsure, answer exactly: UNSURE."},
      {role: "user", content: $p}
    ],
    think: false,
    stream: false,
    options: {temperature: 0.2, num_predict: 200}
  }')" | jq -r '.message.content // .error // "ERROR: no content"'
