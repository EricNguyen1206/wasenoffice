#!/bin/zsh
# Starter idempotent cho System-1 stack: semif-engine (llama-server) + quick_decision_router.
set -u
cd "$(dirname "$0")"
cd ../semif_engine && ./start.sh || exit 1
cd ../system1
PORT="${SYSTEM1_PORT:-11437}"
LOG="${TMPDIR:-/tmp}/nanooffice-system1.log"
if curl -sf "http://localhost:$PORT/health" >/dev/null 2>&1; then
  echo "system1-router already running on :$PORT"
  exit 0
fi
nohup node router.mjs >> "$LOG" 2>&1 &
for i in {1..20}; do
  sleep 0.5
  if curl -sf "http://localhost:$PORT/health" >/dev/null 2>&1; then
    echo "system1-router started on :$PORT (log: $LOG)"
    exit 0
  fi
done
echo "system1-router failed to start — see $LOG" >&2
exit 1
