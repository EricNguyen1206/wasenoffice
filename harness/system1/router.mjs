#!/usr/bin/env node
// quick_decision_router — tool interface duy nhất cho Main Agent gọi System-1 decisions.
//
// Kiến trúc: Main Agent → router (:11437) → semif-engine (:11435, direct logit readout)
//           → [confidence < threshold] → escalate về Main Agent (full reasoning).
// Router KHÔNG tự gọi main agent — nó gắn cờ `escalated` + trả đầy đủ probabilities
// để Main Agent tự re-reason những primitive yếu (tham khảo README.md § cascade).
//
// Payload 100% tiếng Anh theo quy ước (docs/conventions.md). Contract giống /evaluate
// của semif-engine nên agent có thể trỏ về router như một drop-in.
import http from 'node:http';

const PORT = Number(process.env.SYSTEM1_PORT) || 11437;
const SEMIF = process.env.SEMIF_ENGINE_URL || 'http://localhost:11435';
// Ngưỡng an toàn do user đặt (22.09.2026): confidence < 0.85 → escalate.
const CONF_THRESHOLD = Number(process.env.SYSTEM1_CONF_THRESHOLD ?? 0.85);

function confidenceOf(answer) {
  if (answer.type === 'choice') return answer.confidence ?? 0;
  if (answer.type === 'score') return answer.confidence ?? 0;
  if (answer.type === 'noul') return Math.max(answer.noul ?? 0, 1 - (answer.noul ?? 0));
  return 0;
}

async function handleEvaluate(res, raw) {
  const t0 = performance.now();
  let body;
  try { body = JSON.parse(raw); } catch { res.writeHead(400); res.end('{"error":"invalid JSON"}'); return; }
  if (!body || body.state === undefined || !body.questions) {
    res.writeHead(400, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: "Missing 'state' or 'questions'" }));
    return;
  }
  let upstream;
  try {
    const r = await fetch(`${SEMIF}/evaluate`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: raw,
      signal: AbortSignal.timeout(60_000),
    });
    upstream = { status: r.status, data: await r.json() };
  } catch (e) {
    res.writeHead(503, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: `semif-engine unreachable: ${e.message}` }));
    return;
  }
  if (upstream.status !== 200) {
    res.writeHead(upstream.status, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify(upstream.data));
    return;
  }
  // Cascade: từng primitive đánh giá độc lập — thấp điểm hơn ngưỡng → gắn cờ escalate.
  let escalated = 0;
  for (const [name, answer] of Object.entries(upstream.data.answers ?? {})) {
    const conf = confidenceOf(answer);
    answer.router_confidence = Math.round(conf * 1000) / 1000;
    answer.escalated = conf < CONF_THRESHOLD;
    if (answer.escalated) escalated++;
  }
  const total = Object.keys(upstream.data.answers ?? {}).length;
  res.writeHead(200, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({
    ...upstream.data,
    routing: {
      engine: 'semif-direct-v1',
      conf_threshold: CONF_THRESHOLD,
      questions: total,
      escalated,
      decision_source: escalated === 0 ? 'system1' : (escalated === total ? 'main-agent-required' : 'system1+main-agent-partial'),
      note: 'escalated=true nghĩa là Main Agent phải tự reason câu hỏi đó; probabilities vẫn kèm đầy đủ để đối chiếu.',
    },
    meta: { ...upstream.data.meta, router_latency_ms: Math.round(performance.now() - t0) },
  }));
}

async function handleHealth(res) {
  let semif = false, health = null;
  try {
    const h = await fetch(`${SEMIF}/health`, { signal: AbortSignal.timeout(2000) });
    semif = h.ok;
    health = await h.json();
  } catch { /* down */ }
  res.writeHead(200, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify({
    ok: true, service: 'system1-router', semif, semif_health: health,
    conf_threshold: CONF_THRESHOLD,
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
  console.log(`[system1-router] quick_decision_router on http://localhost:${PORT} → ${SEMIF} (threshold ${CONF_THRESHOLD})`);
});
