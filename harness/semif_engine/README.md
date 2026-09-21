# semif_engine — System-1 decision service (direct logit readout, dev-harness only)

Engine quyết định phản xạ: nhận `state` + câu hỏi có kiểu → **1 forward pass** đọc trực tiếp
phân phối xác suất từ logits của answer-letter token — **0 token suy luận, không sinh text**.
KHÔNG thuộc sản phẩm NanoOffice — chỉ phục vụ agent trong repo.

## Nguồn & license

Method port từ [SemIf](https://github.com/theoleecj/semif) — **MIT**, Copyright (c) 2026
TheoLeeCJ (HEAD `ca3ba65`, clone 2026-09-21). SemIf formerly = OpenJev. Tham chiếu method:
`src/semif_phase1/core.py` (frozen prompt, LETTERS, softmax) + `docs/METHOD.md`.

**Method gốc:** system prompt cố định + user payload JSON `{evidence, criterion,
options:[{letter, description}]}` → 1 forward pass → softmax **chỉ trên logits của letter
token đơn** ("A".."P"). Không decode token nào.

## Runtime (khác gốc — đọc kỹ)

| | SemIf gốc | Engine này |
|---|---|---|
| Backend | transformers (CUDA) / MLX | **llama-server** (llama.cpp, Metal) |
| Logits | full vocab tại last position | `top_logprobs: 100` (top-k) |
| Điều kiện | `enable_thinking=False` trong chat template | `chat_template_kwargs: {enable_thinking: false}` |
| Xác suất | full-vocab | **conditional-on-top-k** (letter vắng top-k ⇒ p=0) |

Trick then chốt: template MiniCPM5 mặc định mở thinking (token đầu là "We/The/..." — prose);
`enable_thinking: false` chèn `<think>\n\n</think>\n\n` rỗng → model trả thẳng letter
(A/B/C/D chiếm top-4, token thường tụt xuống −11 logprob).

Model: `MiniCPM5-2B-Q4_K_M` GGUF (SemIf dùng đúng model này trong browser ladder —
balanced accuracy 0.686 theo đo của họ). `model_setup.sh` tái dùng blob Ollama hoặc tải HF.

## Chạy

```zsh
harness/semif_engine/start.sh   # llama-server :11436 + service :11435, idempotent
harness/semif_engine/check.sh   # startup check + E2E + đo latency (PASS = exit 0)
```

## API

`GET /health` → `{ok, llama, model_ready, method}`

`POST /evaluate` — payload tiếng Anh:

```jsonc
{
  "state": "CI run #482 failed ... UNIMPL:LocalFileSave ...",
  "questions": {
    "area":     { "type": "choice", "instructions": "Which component?",
                  "criteria": { "bridge": "...", "sidecar": "..." } },
    "severity": { "type": "score", "instructions": "How severe?",
                  "criteria": ["Low", "Medium", "High", "Critical"] },
    "is_flaky": { "type": "noul",  "instructions": "This failure is likely flaky." }
  }
}
```

Mỗi câu hỏi = 1 forward pass. Response: `{answers, meta:{latency_ms, questions}}`.

## Đo được trên máy này (check.sh, warm)

- Latency: **92–103ms/call** (mục tiêu <100ms; call đầu tiên ~1s do warmup slot/KV).
- E2E triage: `area=bridge` (p=0.949), `severity=2.69` (giữa High và Critical).

## Use case chuẩn cho main agent

| Use case | Cách gọi |
|---|---|
| Phân loại intent / gắn nhãn task | `choice` |
| Triage bug / lỗi CI-CD | 1 call: choice + score + noul |
| Chọn tool/function kế tiếp | `choice` trên tool list |
| Guardrail, Retry/Abort | `noul` → `boolean` (ngưỡng 0.5) |

## Giới hạn (đọc trước khi tin)

- **Model 2B chất lượng borderline** (SemIf tự đo 0.686 balanced accuracy). `confidence < 0.6`
  hoặc `noul` gần 0.5 → coi như UNSURE, đưa về model chính. State càng đủ ngữ cảnh càng chính xác
  (cùng câu hỏi, state ters hơn đã cho kết quả khác — đã thấy trong test).
- Xác suất là **conditional trên options supplied + top-k logprobs**, không phải confidence
  hiệu chỉnh (SemIf METHOD.md nói rõ điều này).
- **CẤM** dùng cho: kiến trúc, license, review/viết code (docs/conventions.md).
- Upgrade path: `Q8_0` GGUF (đổi URL trong `model_setup.sh`) hoặc backend MLX (path gốc của
  SemIf cho Apple Silicon, cho full-vocab logits + prefix reuse tốt hơn).
