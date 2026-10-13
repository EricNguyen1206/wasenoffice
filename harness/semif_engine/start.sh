#!/bin/zsh
# Starter idempotent cho SemIf engine: llama-server (Metal) + Node service.
set -u
cd "$(dirname "$0")"
PORT="${SEMIF_ENGINE_PORT:-11435}"
LLAMA_PORT="${LLAMA_SERVER_PORT:-11436}"
LOG="${TMPDIR:-/tmp}/wasenoffice-semif-engine.log"

# 1) llama-server
if ! curl -sf "http://127.0.0.1:$LLAMA_PORT/health" >/dev/null 2>&1; then
  ./model_setup.sh || exit 1
  echo "starting llama-server on :$LLAMA_PORT (tải model ~5-15s lần đầu)..."
  nohup llama-server -m models/MiniCPM5-2B-Q4_K_M.gguf \
    --port "$LLAMA_PORT" -c 4096 -ngl 99 --host 127.0.0.1 >> "$LOG" 2>&1 &
  for i in {1..60}; do
    sleep 1
    curl -sf "http://127.0.0.1:$LLAMA_PORT/health" >/dev/null 2>&1 && break
  done
fi
if ! curl -sf "http://127.0.0.1:$LLAMA_PORT/health" >/dev/null 2>&1; then
  echo "llama-server failed to start — see $LOG" >&2
  exit 1
fi

# 2) Node service
if curl -sf "http://localhost:$PORT/health" >/dev/null 2>&1; then
  echo "semif-engine already running on :$PORT"
  exit 0
fi
nohup node server.mjs >> "$LOG" 2>&1 &
for i in {1..20}; do
  sleep 0.5
  if curl -sf "http://localhost:$PORT/health" >/dev/null 2>&1; then
    echo "semif-engine started on :$PORT (llama-server :$LLAMA_PORT, log: $LOG)"
    exit 0
  fi
done
echo "semif-engine failed to start — see $LOG" >&2
exit 1
