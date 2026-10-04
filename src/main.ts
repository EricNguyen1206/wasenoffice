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

/**
 * FILE_FORMATS — extension → engine boot params, single home (T-048).
 * `doctype` picks the editor app on the shared boot page (api.js appMap:
 * word→documenteditor, cell→spreadsheeteditor, slide→presentationeditor,
 * pdf→pdfeditor); `filetype` mirrors the source extension. `isForm=false`
 * accompanies pdf exactly like the real desktop shell's boot params.
 * Values are the engine's public api contract, not our invention.
 */
const FILE_FORMATS: Record<string, { filetype: string; doctype: string; isForm?: string }> = {
  docx: { filetype: 'docx', doctype: 'word' },
  xlsx: { filetype: 'xlsx', doctype: 'cell' },
  pptx: { filetype: 'pptx', doctype: 'slide' },
  pdf: { filetype: 'pdf', doctype: 'pdf', isForm: 'false' },
};

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
  /** Registry key of this document (T-059): the shims stamp it onto every
   * later engine call (delivery/save) so Rust resolves exactly this entry. */
  fileId: number;
}

async function mountEditor(doc: OpenedDoc): Promise<void> {
  // T-059: single choke point of every opened doc — the relay (shim) reads
  // this stamp to key delivery/save on THIS document, never "latest".
  (window as unknown as { __nanooffice_file_id__?: number }).__nanooffice_file_id__ =
    doc.fileId;
  const boot = await getBootPage();
  const ext = doc.name.includes('.') ? doc.name.split('.').pop()!.toLowerCase() : '';
  const format = FILE_FORMATS[ext];
  if (!format) {
    logLine(`ERROR: unsupported file extension ".${ext}" (known: ${Object.keys(FILE_FORMATS).join(' ')})`);
    return;
  }
  // `url` = ENGINE_BOOT.offlineUrlMarker — see ENGINE_BOOT above for why the
  // document itself never travels in this URL.
  const params = new URLSearchParams({
    url: ENGINE_BOOT.offlineUrlMarker,
    title: doc.name,
    filetype: format.filetype,
    doctype: format.doctype,
    mode: 'edit',
    lang: navigator.language.slice(0, 2) || 'en',
    username: 'NanoOffice User',
    type: 'desktop',
  });
  if (format.isForm) params.set('isForm', format.isForm);
  const frame = document.createElement('iframe');
  frame.id = 'editor-frame';
  frame.className = 'absolute inset-0 h-full w-full border-0';
  frame.src = `${boot}?${params.toString()}`;
  frame.allow = 'clipboard-read; clipboard-write';
  document.getElementById('editor-root')!.replaceChildren(frame);
  logLine(`editor booting (${format.doctype}): ${doc.name}`);
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

/**
 * Quick view (V5, DD docs/design/viewer-a3/DD.md §4): opens the docx in the
 * read-only viewer window via `viewer_view` — no engine boot. A3 scope is
 * docx-only; everything else keeps going through Open (editor).
 */
async function quickView(path: string): Promise<void> {
  const ext = path.includes('.') ? path.split('.').pop()!.toLowerCase() : '';
  if (ext !== 'docx') {
    logLine(`ERROR: quick view supports .docx (got ".${ext}") — use Open for the editor`);
    return;
  }
  logLine(`quick view: ${path}`);
  try {
    await invoke('viewer_view', { path });
  } catch (err) {
    logLine(`ERROR: ${err}`);
  }
}

/**
 * Nano Light (L01, DD docs/design/editor-a4/DD.md): opens the docx in the
 * lightweight editor window — no engine boot, no Editor.bin (save goes
 * straight through the Rust OOXML core). The DEFAULT open path stays the
 * engine (D1); this button is Nano Light's own entry.
 */
async function nanoLight(path: string): Promise<void> {
  const ext = path.includes('.') ? path.split('.').pop()!.toLowerCase() : '';
  if (ext !== 'docx') {
    logLine(`ERROR: nano light supports .docx (got ".${ext}") — use Open for the engine`);
    return;
  }
  logLine(`nano light: ${path}`);
  try {
    await invoke('nano_light_view', { path });
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
// was the `#ph0-log` rule (styles.css pre-T-058); font-mono's first family
// ui-monospace renders the same face the old 2-family stack picked
log.className = 'max-h-[4.5em] overflow-auto px-4 font-mono text-[0.72em] opacity-75';

// Shared control skin (input + button) — one home; states/differences append.
const CONTROL_SKIN =
  'rounded-panel border border-transparent bg-panel px-[1.2em] py-[0.6em] text-[1em] font-medium font-[family-name:inherit] text-fg shadow-[0_2px_2px_rgba(0,0,0,0.2)] outline-none transition-[border-color] duration-[250ms] ease-[ease] dark:text-white';

document.querySelector<HTMLDivElement>('#app')!.innerHTML = `
  <header class="flex items-center gap-3 px-4 py-2">
    <div class="flex items-center gap-2">
      <img src="/src/assets/logo.png" alt="NanoOffice" class="h-6 w-6 rounded object-contain" />
      <h1 class="m-0 text-[1.1em]">NanoOffice <small class="font-normal opacity-60">PH-0 spike</small></h1>
    </div>
    <form id="open-form" class="flex flex-1 gap-2">
      <input
        id="doc-path"
        type="text"
        placeholder="/absolute/path/to/document (.docx .xlsx .pptx .pdf)"
        class="flex-1 ${CONTROL_SKIN}"
      />
      <button type="submit" class="cursor-pointer hover:border-edge active:border-edge active:bg-panel-active ${CONTROL_SKIN}">Open</button>
      <button type="button" id="quick-view-btn" class="cursor-pointer hover:border-edge active:border-edge active:bg-panel-active ${CONTROL_SKIN}">Quick view</button>
      <button type="button" id="nano-light-btn" class="cursor-pointer hover:border-edge active:border-edge active:bg-panel-active ${CONTROL_SKIN}">Nano Light</button>
    </form>
  </header>
  <main id="editor-root" class="relative flex-1"></main>
`;
document.querySelector<HTMLDivElement>('#app')!.appendChild(log);

const pathInput = document.querySelector<HTMLInputElement>('#doc-path')!;
document.querySelector<HTMLFormElement>('#open-form')!.addEventListener('submit', (ev) => {
  ev.preventDefault();
  if (pathInput.value.trim()) void openPath(pathInput.value.trim());
});
document.querySelector<HTMLButtonElement>('#quick-view-btn')!.addEventListener('click', () => {
  if (pathInput.value.trim()) void quickView(pathInput.value.trim());
});
document.querySelector<HTMLButtonElement>('#nano-light-btn')!.addEventListener('click', () => {
  if (pathInput.value.trim()) void nanoLight(pathInput.value.trim());
});

// V5 UX E2E driver (gate 13; NANO_OFFICE_UX_PROBE, pattern SAVE_PROBE): fill
// the input and click the real Quick view button — the genuine handler chain
// runs. No env → this block never does anything (product default untouched).
const uxProbe = (window as unknown as { __NANO_OFFICE_UX_PROBE__?: string }).__NANO_OFFICE_UX_PROBE__;
if (uxProbe) {
  pathInput.value = uxProbe;
  document.querySelector<HTMLButtonElement>('#quick-view-btn')!.click();
}

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
