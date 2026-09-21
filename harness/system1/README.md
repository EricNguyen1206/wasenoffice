# system1 — quick_decision_router cho Main Agent (dev-harness only)

Tool interface DUY NHẤT mà Main Agent dùng để ra quyết định phản xạ (System-1):
phân loại intent, gắn nhãn task, triage bug/ticket, chọn tool kế tiếp, guardrail.

```
Main Agent ──POST /evaluate──▶ system1-router (:11437)
                                 │  cascade: confidence < 0.85 → escalated=true
                                 ▼
                          semif-engine (:11435)
                          MiniCPM5-2B direct logit readout
                          (1 forward pass / question, 0 output token)
                                 │
                                 ▼ escalated → Main Agent tự reason câu đó
```

## Chạy

```zsh
harness/system1/start.sh        # semif stack + router, idempotent
harness/system1/smoke_test.sh   # 3 bài toán mẫu + cascade + latency (PASS = exit 0)
```

## API

`POST http://localhost:11437/evaluate` — contract giống semif-engine (`state` + `questions`
với type `choice` / `score` / `noul`). **Payload 100% tiếng Anh.**

Response thêm `routing`:

```jsonc
{
  "answers": {
    "route": { "type": "choice", "choice": "technical", "confidence": 0.852,
               "router_confidence": 0.852, "escalated": false, "...": "..." },
    "urgency": { "type": "score", "score": 2.72, "router_confidence": 0.73,
                 "escalated": true, "...": "..." }
  },
  "routing": {
    "engine": "semif-direct-v1",
    "conf_threshold": 0.85,
    "escalated": 1, "questions": 2,
    "decision_source": "system1 | system1+main-agent-partial | main-agent-required"
  }
}
```

**Ý nghĩa cascade** (ngưỡng `SYSTEM1_CONF_THRESHOLD`, mặc định 0.85 — user chốt 22.09.2026):

- `escalated=false` → tin kết quả System-1, dùng luôn.
- `escalated=true` → Main Agent PHẢI tự reason câu đó; probabilities vẫn kèm đầy đủ để đối chiếu.
- `decision_source=main-agent-required` → toàn bộ câu hỏi đều yếu, khỏi dùng System-1.
- `confidence` theo primitive: choice/score = xác suất của option cao nhất (conditional),
  noul = max(p, 1−p).

Đo được (máy M1 16GB): warm ~95–103ms/evaluate 3 câu (engine); router +~3ms; call sau idle
có thể ~1.8s do KV/slot nguội — smoke_test in số thật mỗi lần chạy.

## Vì sao engine là SemIf chứ không phải Laya (benchmark 22.09.2026, máy này)

Đã đo thật cả hai trên cùng payload (`harness/system1` + `/tmp/laya_bench.py`, dữ liệu trong
`docs/diagnostics.md`):

| Tiêu chí | SemIf + MiniCPM5-2B (llama-server Metal) | Laya 421M (`pip install laya`, torch) |
|---|---|---|
| Triage 3 câu (warm) | **~95ms** | 338.8ms (CPU) — 33ms của T4 không transfer sang M1 |
| 1 câu guardrail | ~90–100ms | 121.2ms (CPU) / 153.4ms (MPS) |
| Cold load | ~10s | 107s |
| area=bridge (đúng?) | ✓ p=0.949 | ✓ p=0.809 (nhưng conf-metric 0.502) |
| Injection guard | ✓ noul=0.893 | ✓ noul=1.0 |
| Context | 4096+ | 512 (state budget chỉ ~320 tokens) |
| Zero-shot branching | thiết kế gốc | "near chance" trên typed-decisions zero-shot (tài liệu Laya) |
| Confidence shipped | conditional-on-options, đọc được | mis-calibrated thấp → threshold 0.85 = luôn escalate |

Laya (Apache-2.0) giữ vị thế **future specialist**: nếu một ngày cần guardrail/noul hot-loop
tần suất cao, fine-tune Laya theo notebook của họ ("fast base to specialise") rồi cắm vào router
như một tier song song semif-engine.

## Báo cáo sau task — System-1 accounting (bắt buộc)

Khi task có gọi router, dòng CUỐI report phải là một dòng accounting (quy ước đầy đủ ở
`docs/conventions.md` § Chế độ vận hành):

```
System-1: X calls · Y auto · Z escalated → main-agent · tiết kiệm ~Y×300 tokens (ước tính bảo thủ)
```

- X = số request `/evaluate`, Y = số primitive `escalated=false` (tự quyết), Z = số primitive
  `escalated=true`.
- Chỉ Y mới được tính tiết kiệm — escalated đã quay về main agent rồi, không tính công.
- Hệ số 300 tokens/quyết định là ước tính bảo thủ cho cost main-model nếu phải tự reason
  (đọc lại context + suy luận + output); System-1 chạy local nên bản thân nó = 0 token cloud.
- Không gọi → ghi `System-1: 0 calls`.

## Giới hạn

- Score (ordinal) là primitive yếu nhất ở CẢ hai engine — hầu như luôn escalate, chấp nhận vậy.
- `confidence` của semif là xác suất conditional trên options supplied — không phải calibration
  tuyệt đối (SemIf METHOD.md).
- CẤM dùng cho: kiến trúc, license, review/viết code (docs/conventions.md).
