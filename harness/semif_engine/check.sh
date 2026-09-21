#!/bin/zsh
# Startup verification + E2E integration test — SemIf engine (direct logit readout).
# PASS khi exit 0. Payload test 100% tiếng Anh (quy ước agent↔model).
set -u
cd "$(dirname "$0")"
PORT="${SEMIF_ENGINE_PORT:-11435}"
BASE="http://localhost:$PORT"

echo "▸ 1/5 start stack (llama-server + service, idempotent)"
./start.sh || exit 1

echo "▸ 2/5 health: llama-server + model"
HEALTH=$(curl -sf "$BASE/health")
if [ -z "$HEALTH" ]; then echo "  ✗ no health response"; exit 1; fi
if ! echo "$HEALTH" | jq -e '.ok == true and .llama == true and .model_ready == true' >/dev/null; then
  echo "  ✗ not healthy: $HEALTH"
  exit 1
fi
echo "  ✓ $(echo "$HEALTH" | jq -c .)"

echo "▸ 3/5 E2E evaluate — triage CI failure (choice + score + noul, một call)"
PAYLOAD=$(cat <<'JSON'
{
  "state": "CI run #482 failed on branch feat/ph0-bridge. Log excerpt: thread 'main' panicked at 'called `Result::unwrap()` on an `Err` value: UNIMPL:LocalFileSave', src-tauri/src/lib.rs:57. Checks: cargo fmt OK, clippy OK, license gate OK. The failure appeared right after the bridge dispatch was wired to the new LocalFileSave stub.",
  "questions": {
    "area": {
      "type": "choice",
      "instructions": "Which component does this failure belong to?",
      "criteria": {
        "bridge": "AscDesktopEditor bridge dispatch and stubs in src-tauri/src/bridge",
        "sidecar": "x2t converter subprocess management",
        "frontend": "Tauri webview frontend, shim, or build tooling",
        "infra": "CI pipeline, toolchain, or environment issues"
      }
    },
    "severity": {
      "type": "score",
      "instructions": "How severe is this failure for the current phase?",
      "criteria": ["Low", "Medium", "High", "Critical"]
    },
    "is_flaky": {
      "type": "noul",
      "instructions": "This failure is likely flaky (nondeterministic) rather than a real regression."
    }
  }
}
JSON
)
RESP=$(curl -sf -X POST "$BASE/evaluate" -H 'Content-Type: application/json' --data "$PAYLOAD")
if [ -z "$RESP" ]; then echo "  ✗ evaluate call failed"; exit 1; fi
if ! echo "$RESP" | jq -e '
    (.answers.area.type == "choice") and
    (.answers.area.choice | type == "string") and
    ((.answers.area.probabilities | keys | length) == 4) and
    (.answers.area.probabilities[.answers.area.choice] != null) and
    (.answers.severity.type == "score") and
    (.answers.severity.score | type == "number") and
    (.answers.is_flaky.noul | type == "number") and
    (.answers.is_flaky.noul >= 0 and .answers.is_flaky.noul <= 1) and
    (.answers.is_flaky.boolean | type == "boolean") and
    (.meta.latency_ms | type == "number")' >/dev/null; then
  echo "  ✗ response shape invalid:"
  echo "$RESP" | jq -c .
  exit 1
fi
echo "  ✓ $(echo "$RESP" | jq -c '{area: .answers.area.choice, p_area: .answers.area.probabilities[.answers.area.choice], severity: .answers.severity.score, is_flaky: .answers.is_flaky.noul, latency_ms: .meta.latency_ms}')"

echo "▸ 4/5 latency: đo 5 call liên tiếp (target < 100ms/call)"
i=0; over=0
while [ $i -lt 5 ]; do
  MS=$(curl -sf -o /dev/null -w '%{time_total}' -X POST "$BASE/evaluate" -H 'Content-Type: application/json' --data "$PAYLOAD" | awk '{print int($1*1000)}')
  echo "  call $((i+1)): ${MS}ms"
  [ "$MS" -gt 100 ] && over=$((over+1))
  i=$((i+1))
done
echo "  → $over/5 call vượt 100ms (chỉ báo, không fail test — model 2B/Q4 trên máy này)"

echo "▸ 5/5 negative: missing state → 400; sai type → 400; 1 option → 400"
CODE1=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/evaluate" -H 'Content-Type: application/json' -d '{"questions":{"a":{"type":"noul"}}}')
CODE2=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/evaluate" -H 'Content-Type: application/json' -d '{"state":"x","questions":{"a":{"type":"freeform"}}}')
CODE3=$(curl -s -o /dev/null -w '%{http_code}' -X POST "$BASE/evaluate" -H 'Content-Type: application/json' -d '{"state":"x","questions":{"a":{"type":"choice","criteria":{"only":"one"}}}}')
if [ "$CODE1" != "400" ] || [ "$CODE2" != "400" ] || [ "$CODE3" != "400" ]; then
  echo "  ✗ expected 400/400/400, got $CODE1/$CODE2/$CODE3"
  exit 1
fi
echo "  ✓ 400/400/400 như mong đợi"

echo "E2E PASSED"
