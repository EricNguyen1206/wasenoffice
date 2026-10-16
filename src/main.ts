import './shim/desktopEditor.js';
import './shim/delivery.js';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { getVersion } from '@tauri-apps/api/app';
import { check } from '@tauri-apps/plugin-updater';

/**
 * Start page (A01, DD docs/design/A01-startpage/DD.md): Welcome — create/open
 * documents, get back to recents. The editor boot path mirrors the ONLYOFFICE
 * desktop app (desktop-sdk applicationmanager.cpp:89 opens
 * `editors/web-apps/apps/api/documents/index.html`): that page reads query
 * params (url/title/filetype/key/doctype/mode/lang/username) and creates the
 * DocsAPI.DocEditor itself — everything on the `wasenoffice-engine://` origin,
 * so document + fonts + engine assets are same-origin.
 * The document URL flows through `wasenoffice-file://<id>/<name>` (disk path stays in Rust).
 * Shim load order is load-bearing: desktopEditor.js (bridge + shared plumbing)
 * before delivery.js (open-delivery chain, consumes window.__wasenoffice_shim).
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

/**
 * Start-page labels (DD A01 §8): VI/EN by system language, EN fallback.
 * Log lines stay English — they are gate/diagnostics contract, not UI.
 */
const STRINGS = {
  en: {
    tagline: 'Local-first office suite — your documents never leave this machine.',
    newDocument: 'Document',
    newSpreadsheet: 'Spreadsheet',
    newPresentation: 'Presentation',
    openDocument: 'Open document…',
    quickView: 'Quick view…',
    nanoLight: 'Nano Light…',
    recents: 'Recent documents',
    orPastePath: 'or paste a path…',
    open: 'Open',
    removeRecent: 'Remove from recents',
    dialogFailed: 'open dialog failed',
  },
  vi: {
    tagline: 'Bộ văn phòng local-first — tài liệu không bao giờ rời máy bạn.',
    newDocument: 'Tài liệu',
    newSpreadsheet: 'Bảng tính',
    newPresentation: 'Trình chiếu',
    openDocument: 'Mở tài liệu…',
    quickView: 'Xem nhanh…',
    nanoLight: 'Nano Light…',
    recents: 'Tài liệu gần đây',
    orPastePath: 'hoặc dán đường dẫn…',
    open: 'Mở',
    removeRecent: 'Gỡ khỏi danh sách',
    dialogFailed: 'hộp thoại mở file lỗi',
  },
} as const;
type Lang = keyof typeof STRINGS;
const LANG: Lang = navigator.language.toLowerCase().startsWith('vi') ? 'vi' : 'en';
const t = STRINGS[LANG];

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

/** Welcome out, editing surface in (DD A01 §3 OPENING → exit). */
async function mountEditor(doc: OpenedDoc): Promise<void> {
  // T-059: single choke point of every opened doc — the relay (shim) reads
  // this stamp to key delivery/save on THIS document, never "latest".
  (window as unknown as { __wasenoffice_file_id__?: number }).__wasenoffice_file_id__ =
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
    username: 'WasenOffice User',
    type: 'desktop',
  });
  if (format.isForm) params.set('isForm', format.isForm);
  const frame = document.createElement('iframe');
  frame.id = 'editor-frame';
  frame.className = 'absolute inset-0 h-full w-full border-0';
  frame.src = `${boot}?${params.toString()}`;
  frame.allow = 'clipboard-read; clipboard-write';
  // Welcome out, editing surface in (DD A01 §3 OPENING → exit).
  welcome.hidden = true;
  topBar.classList.remove('hidden');
  editorRoot.classList.remove('hidden');
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
 * Create-new (A01 T-A39): copies a bundled Template blank into the app-managed
 * untitled dir and opens it — a real file, so the whole open pipeline works
 * unchanged. The FIRST save later routes through Save-As (Rust-side seam).
 */
async function createDocument(kind: 'docx' | 'xlsx' | 'pptx'): Promise<void> {
  logLine(`create: ${kind}`);
  try {
    const doc = await invoke<OpenedDoc>('create_document', { kind });
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
    await invoke('wasen_light_view', { path });
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

// ── Welcome (A01 DD §4): one centered column — brand, actions, recents ──

/** Open dialog of the OS, filtered to the formats this app accepts (DD §5). */
const DOC_FILTERS = [{ name: 'Documents', extensions: ['docx', 'xlsx', 'pptx', 'pdf'] }];
async function pickPath(): Promise<string | null> {
  try {
    const picked = await openDialog({ multiple: false, directory: false, filters: DOC_FILTERS });
    return typeof picked === 'string' ? picked : null;
  } catch (err) {
    logLine(`ERROR: ${t.dialogFailed}: ${err}`);
    return null;
  }
}

/** One recents row (recents_list payload — serde camelCase, T-059 gotcha). */
interface RecentRow {
  path: string;
  name: string;
  kind: string;
  openedAt: number;
  exists: boolean;
}

/** Act on a recents row: missing file → loud error + removal hint (DD §6). */
function activateRecent(row: RecentRow): void {
  if (!row.exists) {
    logLine(`ERROR: file not found: ${row.path} — remove it from recents`);
    return;
  }
  void openPath(row.path);
}

async function renderRecents(): Promise<void> {
  let rows: RecentRow[] = [];
  try {
    rows = await invoke<RecentRow[]>('recents_list');
  } catch (err) {
    logLine(`ERROR: recents list: ${err}`);
  }
  const missing = rows.filter((r) => !r.exists).length;
  // Gate 17 log contract (frozen: smoke-lib.sh SMOKE_RE_SP_READY) — change in the SAME commit.
  logLine(`startpage ready (welcome): recents=${rows.length} missing=${missing} lang=${LANG}`);

  const list = document.getElementById('recents-list')!;
  list.replaceChildren();
  document.getElementById('recents-box')!.classList.toggle('hidden', rows.length === 0);
  for (const row of rows) {
    const li = document.createElement('li');
    li.className = `flex cursor-pointer items-center gap-3 rounded-panel border border-edge bg-panel px-3 py-2 text-fg shadow-sm outline-none transition-colors duration-150 hover:bg-panel-active ${
      row.exists ? '' : 'opacity-50'
    }`;
    const kind = document.createElement('span');
    kind.className = 'rounded bg-panel-active px-1.5 py-0.5 text-[0.65em] font-semibold uppercase';
    kind.textContent = row.kind || '?';
    const name = document.createElement('span');
    name.className = 'flex-1 truncate text-[0.88em] font-medium';
    name.textContent = row.name;
    const dir = document.createElement('span');
    dir.className = 'truncate text-[0.72em] text-fg/45';
    dir.textContent = row.path;
    const remove = document.createElement('button');
    remove.type = 'button';
    remove.title = t.removeRecent;
    remove.textContent = '×';
    remove.className =
      'cursor-pointer border-0 bg-transparent p-0 text-[1.1em] leading-none text-fg/40 hover:text-fg';
    remove.addEventListener('click', (ev) => {
      ev.stopPropagation();
      void invoke('recents_remove', { path: row.path }).then(() => renderRecents());
    });
    li.append(kind, name, dir, remove);
    li.addEventListener('click', () => activateRecent(row));
    list.appendChild(li);
  }
  return runStartpageProbe(rows);
}

/** Gate 17 driver (WASEN_OFFICE_STARTPAGE_PROBE, pattern UX_PROBE): click a
 * REAL recents row (`recent:N`) or create button (`create:docx|xlsx|pptx`) —
 * the genuine handler chain runs. No env → nothing. */
function runStartpageProbe(rows: RecentRow[]): void {
  const probe = (window as unknown as { __WASEN_OFFICE_STARTPAGE_PROBE__?: string })
    .__WASEN_OFFICE_STARTPAGE_PROBE__;
  const recent = probe?.match(/^recent:(\d+)$/);
  if (recent) {
    const row = rows[Number(recent[1]) - 1];
    if (row) activateRecent(row);
    else logLine(`ERROR: startpage probe: no recents row #${recent[1]}`);
    return;
  }
  const create = probe?.match(/^create:(docx|xlsx|pptx)$/);
  if (create) {
    document.getElementById(`create-${create[1]}`)!.click();
  }
}

const log: HTMLDivElement = document.createElement('div');
log.id = 'ph0-log';
// was the `#ph0-log` rule (styles.css pre-T-058); font-mono's first family
// ui-monospace renders the same face the old 2-family stack picked
log.className = 'max-h-[4.5em] overflow-auto px-4 py-1.5 font-mono text-[0.72em] opacity-80 border-t border-edge bg-panel text-fg';

// Shared control skin (input + button) using Graphite tokens (DD U01 §3).
const CONTROL_SKIN =
  'rounded-panel border border-edge bg-panel px-[1.2em] py-[0.5em] text-[0.9em] font-medium font-[family-name:inherit] text-fg shadow-sm outline-none transition-colors duration-150 hover:bg-panel-active active:bg-panel-active focus:border-accent disabled:opacity-50';

document.querySelector<HTMLDivElement>('#app')!.innerHTML = `
  <header id="top-bar" class="hidden items-center gap-2.5 border-b border-edge bg-panel px-4 py-2.5 shadow-sm">
    <img src="/logo.png" alt="WasenOffice" class="h-6 w-6 rounded object-contain" />
    <h1 class="m-0 text-[1.05em] font-semibold text-fg tracking-tight">WasenOffice</h1>
  </header>
  <main id="editor-root" class="relative hidden flex-1 bg-paper"></main>
  <section id="welcome" class="flex flex-1 flex-col overflow-auto bg-bg text-fg">
    <div class="m-auto flex w-full max-w-xl flex-col items-center gap-7 px-6 py-10">
      <div class="flex flex-col items-center gap-2 text-center">
        <img src="/logo.png" alt="WasenOffice" class="h-16 w-16 rounded-xl object-contain" />
        <h1 class="m-0 text-[2em] font-semibold tracking-tight">WasenOffice</h1>
        <p class="m-0 max-w-md text-[0.9em] text-fg/60">${t.tagline}</p>
      </div>
      <div class="flex flex-wrap items-center justify-center gap-2.5">
        <button type="button" id="create-docx" class="cursor-pointer ${CONTROL_SKIN}">${t.newDocument}</button>
        <button type="button" id="create-xlsx" class="cursor-pointer ${CONTROL_SKIN}">${t.newSpreadsheet}</button>
        <button type="button" id="create-pptx" class="cursor-pointer ${CONTROL_SKIN}">${t.newPresentation}</button>
      </div>
      <div class="flex flex-wrap items-center justify-center gap-2.5">
        <button type="button" id="welcome-open" class="cursor-pointer rounded-panel border border-accent bg-accent px-5 py-2 text-[0.95em] font-semibold text-white shadow-sm outline-none transition-[filter] duration-150 hover:brightness-110 focus:border-accent">${t.openDocument}</button>
        <button type="button" id="welcome-quick" class="cursor-pointer ${CONTROL_SKIN}">${t.quickView}</button>
        <button type="button" id="welcome-nano" class="cursor-pointer ${CONTROL_SKIN}">${t.nanoLight}</button>
      </div>
      <section id="recents-box" class="hidden w-full flex-col gap-2">
        <h2 class="m-0 text-[0.8em] font-semibold uppercase tracking-wide text-fg/50">${t.recents}</h2>
        <ul id="recents-list" class="m-0 flex w-full list-none flex-col gap-1.5 p-0"></ul>
      </section>
      <footer class="flex flex-col items-center gap-2.5">
        <button type="button" id="paste-toggle" class="cursor-pointer border-0 bg-transparent p-0 text-[0.8em] text-fg/50 underline outline-none hover:text-fg">${t.orPastePath}</button>
        <form id="open-form" class="hidden flex items-center gap-2">
          <input
            id="doc-path"
            type="text"
            placeholder="/absolute/path/to/document (.docx .xlsx .pptx .pdf)"
            class="w-[26em] ${CONTROL_SKIN}"
          />
          <button type="submit" class="cursor-pointer ${CONTROL_SKIN}">${t.open}</button>
          <button type="button" id="quick-view-btn" class="cursor-pointer ${CONTROL_SKIN}">${t.quickView}</button>
          <button type="button" id="nano-light-btn" class="cursor-pointer ${CONTROL_SKIN}">${t.nanoLight}</button>
        </form>
        <span id="version" class="text-[0.72em] text-fg/40"></span>
      </footer>
    </div>
  </section>
`;
const app = document.querySelector<HTMLDivElement>('#app')!;
app.appendChild(log);
const welcome = document.getElementById('welcome')!;
const topBar = document.getElementById('top-bar')!;
const editorRoot = document.getElementById('editor-root')!;

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
document.querySelector<HTMLButtonElement>('#paste-toggle')!.addEventListener('click', () => {
  document.getElementById('open-form')!.classList.remove('hidden');
  pathInput.focus();
});
document.querySelector<HTMLButtonElement>('#welcome-open')!.addEventListener('click', async () => {
  const path = await pickPath();
  if (path) void openPath(path);
});
for (const kind of ['docx', 'xlsx', 'pptx'] as const) {
  document.getElementById(`create-${kind}`)!.addEventListener('click', () => {
    void createDocument(kind);
  });
}
document.querySelector<HTMLButtonElement>('#welcome-quick')!.addEventListener('click', async () => {
  const path = await pickPath();
  if (path) void quickView(path);
});
document.querySelector<HTMLButtonElement>('#welcome-nano')!.addEventListener('click', async () => {
  const path = await pickPath();
  if (path) void nanoLight(path);
});

// V5 UX E2E driver (gate 13; WASEN_OFFICE_UX_PROBE, pattern SAVE_PROBE): fill
// the input and click the real Quick view button — the genuine handler chain
// runs. No env → this block never does anything (product default untouched).
// The form stays in the DOM (hidden inside Welcome) so the probe reaches it.
const uxProbe = (window as unknown as { __WASEN_OFFICE_UX_PROBE__?: string }).__WASEN_OFFICE_UX_PROBE__;
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

// Startup document (env WASEN_OFFICE_OPEN, already registered in Rust).
void invoke<OpenedDoc | null>('get_startup_document')
  .then((doc) => {
    if (doc) mountEditor(doc);
  })
  .catch((err) => logLine(`startup doc ERROR: ${err}`));

// Welcome + recents (A01): the ready line is gate 17's log contract.
void renderRecents();

void getVersion()
  .then((v) => {
    document.getElementById('version')!.textContent = `WasenOffice v${v}`;
  })
  .catch(() => {});

// Check for updates on startup (PH-4 T-065).
async function checkForAppUpdates(): Promise<void> {
  try {
    const update = await check();
    if (update?.available) {
      logLine(`update: phát hiện phiên bản mới v${update.version}`);
      const ok = window.confirm(
        `Đã có phiên bản WasenOffice mới (v${update.version}). Bạn có muốn cập nhật ngay không?`
      );
      if (ok) {
        logLine(`update: đang tải bản v${update.version}...`);
        await update.downloadAndInstall();
        logLine(`update: tải hoàn tất! Vui lòng khởi động lại.`);
        alert(`Bản cập nhật v${update.version} đã sẵn sàng. Vui lòng khởi động lại app.`);
      }
    }
  } catch (err) {
    // Expected when offline or running locally before first GitHub release
    console.warn('updater check:', err);
  }
}
void checkForAppUpdates();
