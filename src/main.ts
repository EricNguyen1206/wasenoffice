import './shim/desktopEditor.js';
import './shim/delivery.js';

/**
 * PH-0 start page: open one real DOCX and boot the editor.
 *
 * Boot path mirrors the ONLYOFFICE desktop app (desktop-sdk applicationmanager.cpp:89
 * opens `editors/web-apps/apps/api/documents/index.html`): that page reads query
 * params (url/title/filetype/key/doctype/mode/lang/username) and creates the
 * DocsAPI.DocEditor itself — everything on the `nanooffice-engine://` origin,
 * so document + fonts + engine assets are same-origin.
 * The document URL flows through `nanooffice-file://<id>/<name>` (disk path stays in Rust).
 * Shim load order is load-bearing: desktopEditor.js (bridge + shared plumbing)
 * before delivery.js (open-delivery chain, consumes window.__nanooffice_shim).
 */

/**
 * ENGINE_BOOT — the engine boot contract, single home (T-045). The boot page
 * path mirrors the ONLYOFFICE desktop app (desktop-sdk applicationmanager.cpp:89).
 * `_offline_` marks the engine's offline open path (apiBase:682): the URL never
 * carries the document — the bytes arrive via the LocalStartOpen delivery chain
 * (src/shim/delivery.js) after Rust converts with x2t.
 */
const ENGINE_BOOT = {
  pagePath: '/editors/web-apps/apps/api/documents/index.html',
  offlineUrlMarker: '_offline_',
} as const;

const { invoke } = window.__TAURI__.core;

// Loopback origin serving the engine + documents (Rust command).
let bootPage: string | null = null;

async function getBootPage(): Promise<string> {
  if (!bootPage) {
    const base = await invoke<string>('get_engine_base');
    bootPage = `${base}${ENGINE_BOOT.pagePath}`;
  }
  return bootPage;
}

interface OpenedDoc {
  url: string;
  name: string;
}

async function mountEditor(doc: OpenedDoc): Promise<void> {
  const boot = await getBootPage();
  // `url` = ENGINE_BOOT.offlineUrlMarker — see ENGINE_BOOT above for why the
  // document itself never travels in this URL.
  const params = new URLSearchParams({
    url: ENGINE_BOOT.offlineUrlMarker,
    title: doc.name,
    filetype: 'docx',
    doctype: 'word',
    mode: 'edit',
    lang: navigator.language.slice(0, 2) || 'en',
    username: 'NanoOffice User',
    type: 'desktop',
  });
  const frame = document.createElement('iframe');
  frame.id = 'editor-frame';
  frame.src = `${boot}?${params.toString()}`;
  frame.allow = 'clipboard-read; clipboard-write';
  document.getElementById('editor-root')!.replaceChildren(frame);
  logLine(`editor booting (delivery via bridge): ${doc.name}`);
}

async function openPath(path: string): Promise<void> {
  logLine(`opening ${path}`);
  try {
    const doc = await invoke<OpenedDoc>('open_document', { path });
    mountEditor(doc);
  } catch (err) {
    logLine(`ERROR: ${err}`);
  }
}

function logLine(text: string): void {
  const el = document.createElement('div');
  el.textContent = text;
  log.appendChild(el);
  void invoke('frontend_log', { level: 'info', message: text }).catch(() => {});
}

const log: HTMLDivElement = document.createElement('div');
log.id = 'ph0-log';

document.querySelector<HTMLDivElement>('#app')!.innerHTML = `
  <header>
    <h1>NanoOffice <small>PH-0 spike</small></h1>
    <form id="open-form">
      <input id="doc-path" type="text" placeholder="/absolute/path/to/document.docx" />
      <button type="submit">Open</button>
    </form>
  </header>
  <main id="editor-root"></main>
`;
document.querySelector<HTMLDivElement>('#app')!.appendChild(log);

document.querySelector<HTMLFormElement>('#open-form')!.addEventListener('submit', (ev) => {
  ev.preventDefault();
  const input = document.querySelector<HTMLInputElement>('#doc-path')!;
  if (input.value.trim()) void openPath(input.value.trim());
});

// PH-0 observability: nothing DOM-side may fail silently — forward to stdout.
// capture=true so RESOURCE load errors (script/img) also surface here.
window.addEventListener(
  'error',
  (ev) => {
    const target = ev.target as (EventTarget & { src?: string; href?: string }) | null;
    const resource = target && (target.src || target.href);
    void invoke('frontend_log', {
      level: 'error',
      message: resource
        ? `resource error: ${resource}`
        : `${ev.message} @ ${ev.filename}:${ev.lineno}:${ev.colno}`,
    }).catch(() => {});
  },
  true,
);
window.addEventListener('unhandledrejection', (ev) => {
  void invoke('frontend_log', {
    level: 'error',
    message: `unhandled rejection: ${String((ev as PromiseRejectionEvent).reason)}`,
  }).catch(() => {});
});

// Startup document (env NANO_OFFICE_OPEN, already registered in Rust).
void invoke<OpenedDoc | null>('get_startup_document')
  .then((doc) => {
    if (doc) mountEditor(doc);
  })
  .catch((err) => logLine(`startup doc ERROR: ${err}`));
