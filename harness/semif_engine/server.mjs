#!/usr/bin/env node
// NanoOffice SemIf engine — System-1 decision service (direct logit readout, dev-harness only).
//
// Nguồn method: SemIf (MIT License) — https://github.com/theoleecj/semif
// (HEAD ca3ba65, clone 2026-09-21). Copyright (c) 2026 TheoLeeCJ. SemIf formerly = OpenJev.
// Method gốc (src/semif_phase1/core.py + direct.py): frozen prompt
//   system: "Apply the supplied criterion... Respond with only its uppercase letter..."
//   user:   JSON {evidence, criterion, options:[{letter, description}]}
//   → 1 forward pass → softmax CHỈ trên logits của letter token đơn ("A".."P").
// Khác biệt runtime so với gốc (ghi rõ ở README): gốc dùng transformers/MLX đọc full-vocab
// logits; ở đây dùng llama-server /v1/chat/completions với top_logprobs (top-100) + giới hạn
// cho letter token — xác suất là conditional-on-top-k, không phải full vocab.
// Prompt gửi model 100% tiếng Anh theo quy ước dự án (docs/conventions.md).
//
// Contract (giữ nguyên từ thời decision-engine để agent workflow ổn định):
//   POST /evaluate {state, questions: {name: {type: choice|score|noul|boolean, instructions?, criteria}}}
//     choice          → criteria: {key: description}        → {choice, confidence, probabilities}
//     score           → criteria: [label, ...]              → {score, confidence, legend, probabilities}
//     noul / boolean  → instructions (mệnh đề tiếng Anh)    → {noul, boolean}
// Mỗi câu hỏi = đúng 1 forward pass của model (0 token suy luận có nghĩa).
import http from 'node:http';
import { existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)));
const PORT = Number(process.env.SEMIF_ENGINE_PORT) || 11435;
const LLAMA = process.env.LLAMA_SERVER_URL || 'http://127.0.0.1:11436';
const MODEL_FILE = process.env.SEMIF_MODEL || join(ROOT, 'models', 'MiniCPM5-2B-Q4_K_M.gguf');
const TIMEOUT_MS = Number(process.env.SEMIF_ENGINE_TIMEOUT_MS) || 30_000;
const MAX_QUESTIONS = 16;
const LETTERS = 'ABCDEFGHIJKLMNOP'; // đúng như LETTERS của SemIf core.py

const DIRECT_SYSTEM =
  'Apply the supplied criterion to the supplied evidence. Choose exactly one listed option. ' +
  'Respond with only its uppercase letter, with no explanation or reasoning.';

// 1 call llama-server = 1 forward pass; đọc phân phối next-token giới hạn top-100.
async function letterLogits(evidence, criterion, optionDescriptions, temperature = 0) {
  const payload = {
    evidence,
    criterion,
    options: optionDescriptions.map((description, i) => ({ letter: LETTERS[i], description })),
  };
  const body = JSON.stringify({
    messages: [
      { role: 'system', content: DIRECT_SYSTEM },
      { role: 'user', content: JSON.stringify(payload) },
    ],
    max_tokens: 1, // prefill-only decision: chỉ đi tiếp đúng 1 vị trí
    temperature,
    logprobs: true,
    top_logprobs: 100,
    chat_template_kwargs: { enable_thinking: false }, // chèn <think></think> rỗng — trả lời thẳng
  });
  let res;
  try {
    res = await fetch(`${LLAMA}/v1/chat/completions`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
  } catch (e) {
    throw Object.assign(new Error(`llama-server unreachable: ${e.message}`), { status: 503 });
  }
  if (!res.ok) {
    throw Object.assign(new Error(`llama-server ${res.status}: ${(await res.text()).slice(0, 200)}`), { status: 502 });
  }
  const data = await res.json();
  const topLp = data?.choices?.[0]?.logprobs?.content?.[0]?.top_logprobs;
  if (!Array.isArray(topLp) || topLp.length === 0) {
    throw Object.assign(new Error('no logprobs in llama-server response'), { status: 502 });
  }
  const count = optionDescriptions.length;
  const wanted = new Map(); // letter -> logprob
  for (let i = 0; i < count; i++) wanted.set(LETTERS[i], null);
  for (const entry of topLp) {
    if (wanted.has(entry.token) && wanted.get(entry.token) === null) {
      wanted.set(entry.token, entry.logprob);
    }
  }
  const missing = [...wanted.entries()].filter(([, lp]) => lp === null).map(([l]) => l);
  if (missing.length === count) {
    throw Object.assign(new Error('no answer letters found in top-logprobs'), { status: 502 });
  }
  // Softmax trên đúng các letter token (SemIf softmax; letter vắng mặt trong top-k ⇒ p = 0)
  const lps = [...wanted.values()].map((lp) => (lp === null ? -Infinity : lp));
  const max = Math.max(...lps.filter((v) => Number.isFinite(v)));
  const weights = lps.map((lp) => (Number.isFinite(lp) ? Math.exp(lp - max) : 0));
  const total = weights.reduce((a, b) => a + b, 0) || 1;
  return weights.map((w) => w / total);
}

function round(x, digits = 4) {
  const f = 10 ** digits;
  return Math.round(x * f) / f;
}

async function evaluateOne(name, q, stateStr) {
  if (q.type === 'choice') {
    const keys = Object.keys(q.criteria);
    const probs = await letterLogits(stateStr, q.instructions ?? `Select the best label for "${name}"`,
      keys.map((k) => q.criteria[k]));
    const probabilities = {};
    keys.forEach((k, i) => { probabilities[k] = round(probs[i]); });
    let best = 0;
    for (let i = 1; i < probs.length; i++) if (probs[i] > probs[best]) best = i;
    return { answer: { type: 'choice', choice: keys[best], confidence: round(probs[best]), probabilities } };
  }
  if (q.type === 'score') {
    const levels = q.criteria;
    const probs = await letterLogits(stateStr, q.instructions ?? `Rate "${name}"`,
      levels.map((label, i) => `level ${i}: ${label}`));
    const score = round(probs.reduce((s, p, i) => s + p * i, 0), 3);
    const legend = {}, probabilities = {};
    levels.forEach((l, i) => { legend[String(i)] = l; probabilities[String(i)] = round(probs[i]); });
    return { answer: { type: 'score', score, confidence: round(Math.max(...probs)), legend, probabilities } };
  }
  // noul / boolean — 2 letter: A = mệnh đề đúng, B = sai
  const probs = await letterLogits(stateStr, q.instructions ?? `The proposition "${name}" is true.`,
    ['True — the proposition holds.', 'False — the proposition does not hold.']);
  const noul = round(probs[0]);
  return { answer: { type: 'noul', noul, boolean: noul >= 0.5 } };
}

function validate(body) {
  if (body.state === undefined || body.state === null) return "Missing 'state'";
  const qs = body.questions;
  if (!qs || typeof qs !== 'object' || Array.isArray(qs) || Object.keys(qs).length === 0)
    return "Missing or empty 'questions' object";
  if (Object.keys(qs).length > MAX_QUESTIONS) return `Too many questions (max ${MAX_QUESTIONS})`;
  for (const [name, q] of Object.entries(qs)) {
    if (!q || typeof q !== 'object' || !['choice', 'score', 'noul', 'boolean'].includes(q.type))
      return `Question "${name}": invalid type (choice | score | noul | boolean)`;
    if (q.type === 'choice') {
      const n = q.criteria && typeof q.criteria === 'object' && !Array.isArray(q.criteria) ? Object.keys(q.criteria).length : 0;
      if (n < 2) return `Question "${name}": choice needs >= 2 options`;
      if (n > LETTERS.length) return `Question "${name}": too many options (max ${LETTERS.length})`;
    }
    if (q.type === 'score') {
      if (!Array.isArray(q.criteria) || q.criteria.length < 2) return `Question "${name}": score needs >= 2 levels`;
      if (q.criteria.length > LETTERS.length) return `Question "${name}": too many levels (max ${LETTERS.length})`;
    }
  }
  return null;
}

async function handleEvaluate(res, raw) {
  const t0 = performance.now();
  let body;
  try { body = JSON.parse(raw); } catch { res.writeHead(400); res.end('{"error":"invalid JSON"}'); return; }
  const problem = validate(body);
  if (problem) { res.writeHead(400, { 'Content-Type': 'application/json' }); res.end(JSON.stringify({ error: problem })); return; }
  const stateStr = typeof body.state === 'string' ? body.state : JSON.stringify(body.state);
  try {
    const settled = await Promise.all(
      Object.entries(body.questions).map(([name, q]) => evaluateOne(name, q, stateStr))
    );
    const answers = {};
    for (let i = 0; i < settled.length; i++) answers[Object.keys(body.questions)[i]] = settled[i].answer;
    res.writeHead(200, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({
      model: 'MiniCPM5-2B-Q4_K_M',
      readout: 'semif-direct-v1 (letter-token logit softmax, 1 forward pass/question)',
      answers,
      meta: { latency_ms: Math.round(performance.now() - t0), questions: settled.length },
    }));
  } catch (e) {
    res.writeHead(e.status ?? 500, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: e.message }));
  }
}

async function handleHealth(res) {
  let llama = false;
  try {
    const h = await fetch(`${LLAMA}/health`, { signal: AbortSignal.timeout(2000) });
    llama = h.ok;
  } catch { /* down */ }
  res.writeHead(200, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({
    ok: true, service: 'semif-engine', llama,
    model_ready: existsSync(MODEL_FILE),
    model: 'MiniCPM5-2B-Q4_K_M', method: 'semif-direct-v1',
  }));
}

const server = http.createServer((req, res) => {
  if (req.method === 'GET' && req.url === '/health') return void handleHealth(res);
  if (req.method === 'POST' && req.url === '/evaluate') {
    let raw = '';
    req.on('data', (c) => { raw += c; if (raw.length > 2 * 1024 * 1024) req.destroy(); });
    req.on('end', () => void handleEvaluate(res, raw));
    return;
  }
  res.writeHead(404, { 'Content-Type': 'application/json' });
  res.end('{"error":"not found — use GET /health or POST /evaluate"}');
});

server.listen(PORT, () => {
  console.log(`[semif-engine] System-1 direct-logit service on http://localhost:${PORT}`);
  console.log(`[semif-engine] llama-server=${LLAMA} model=${MODEL_FILE}`);
});
