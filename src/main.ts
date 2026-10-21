import './shim/desktopEditor.js';
import './shim/delivery.js';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { getVersion } from '@tauri-apps/api/app';
import { check } from '@tauri-apps/plugin-updater';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { listen } from '@tauri-apps/api/event';
import { createSettingsView, type SettingsTab } from './settings-view';
import { createAiView } from './ai-view';

/** Keymap (T-A41, DD K02): one resolved list from Rust (defaults + overrides).
 *  Shell entries bind main-frame keydown; the whole list is pushed DOWN to the
 *  engine on request — the shim applies engine entries via native sdkjs APIs
 *  (asc_applyAscShortcuts) and stamps reveal hints on toolbar buttons. */
interface KeyChord {
  cmd: boolean;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  key: string;
}
interface KeyEntry {
  id: string;
  label: string;
  group: string;
  scope: 'engine' | 'shell';
  shell: string | null;
  chord: KeyChord;
  keys: string;
  hint: string;
}
const KM_MSG_TYPE = '__wasenoffice_keymap';
let keymap: KeyEntry[] = [];

async function loadKeymap(): Promise<void> {
  try {
    keymap = await invoke<KeyEntry[]>('keymap_load');
  } catch (err) {
    logLine(`ERROR: keymap: ${err}`);
  }
}

function chordOf(ev: KeyboardEvent): KeyChord | null {
  return ev.key.length === 1
    ? { cmd: ev.metaKey, ctrl: ev.ctrlKey, shift: ev.shiftKey, alt: ev.altKey, key: ev.key }
    : null;
}

/** Shell actions (main-frame focus only — engine owns its own keys, DD K02). */
function bindShellKeys(): void {
  window.addEventListener(
    'keydown',
    (ev) => {
      const chord = chordOf(ev);
      if (!chord) return;
      const hit = keymap.find(
        (e) =>
          e.scope === 'shell' &&
          e.chord.cmd === chord.cmd &&
          e.chord.ctrl === chord.ctrl &&
          e.chord.shift === chord.shift &&
          e.chord.alt === chord.alt &&
          e.chord.key.toUpperCase() === chord.key.toUpperCase(),
      );
      if (!hit) return;
      // A keymap chord capture in the settings view owns the keyboard (⌘W
      // must be recorded, not close the window — T-A46).
      if (settingsView?.isCapturing()) return;
      ev.preventDefault();
      void runShellAction(hit.shell);
    },
    true,
  );
}

async function runShellAction(action: string | null): Promise<void> {
  switch (action) {
    case 'open': {
      const path = await pickPath();
      if (path) void openPath(path);
      break;
    }
    case 'create':
      void createDocument('docx');
      break;
    case 'settings':
    case 'cheatsheet':
      showSettings(activeView !== 'settings');
      break;
    case 'close':
      await getCurrentWindow().close();
      break;
  }
}

/** Engine asks (up-relay) → answer with the fresh list (down-relay). */
function bindKeymapPush(): void {
  window.addEventListener('message', (ev) => {
    const d = ev.data as { type?: string } | null;
    if (d && d.type === `${KM_MSG_TYPE}:req`) {
      engineFrame?.contentWindow?.postMessage({ type: KM_MSG_TYPE, entries: keymap }, '*');
    }
  });
}

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
    docxFilter: 'Word documents (.docx)',
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
    docxFilter: 'Tài liệu Word (.docx)',
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

/**
 * Editor chrome tabs (T-A40, DD U01 slice 6): our one-row header [menu tabs |
 * filename | account] replaces the engine's own tabs strip. Actions mirror the
 * engine's tab inventory (upstream web-apps documenteditor Toolbar view:179-186
 * + controller addTab :3934-4075): file/home/ins/layout/links + review
 * (caption Collaboration) + view. Driving goes through the shim tab driver
 * (`__wasenoffice_tab` postMessage, relayed down to the frame holding
 * Common.NotificationCenter): plain tabs → tab:set-active, FILE toggles
 * menu:show/menu:hide. The engine strip itself is hidden by the shim
 * (WASEN_OFFICE_COMPACT_CHROME) — one tools box (engine toolbar) below us.
 */
const ENGINE_TABS: ReadonlyArray<{ action: string; label: string }> = [
  { action: 'file', label: 'File' },
  { action: 'home', label: 'Home' },
  { action: 'ins', label: 'Insert' },
  { action: 'layout', label: 'Layout' },
  { action: 'links', label: 'Links' },
  { action: 'review', label: 'Collaboration' },
  { action: 'view', label: 'View' },
];
const TAB_MSG_TYPE = '__wasenoffice_tab';
const TAB_SKIN =
  'cursor-pointer border-0 bg-transparent px-2.5 py-1.5 text-[0.82em] outline-none transition-colors duration-150 hover:text-fg';
let engineFrame: HTMLIFrameElement | null = null;
const tabButtons = new Map<string, HTMLButtonElement>();
let fileMenuOpen = false;

function setActiveTab(action: string): void {
  for (const [a, btn] of tabButtons) {
    const active = a === action;
    btn.classList.toggle('text-accent', active);
    btn.classList.toggle('font-semibold', active);
  }
}

function sendTab(action: string): void {
  const frame = engineFrame?.contentWindow;
  if (!frame) return;
  // Evidence line (frontend_log contract, ADDITIVE): gate/diagnostics trace of
  // every chrome tab drive (DD U01 slice 6 criterion 3).
  window.__wasenoffice_shim.forwardLog('info', `tab-drive: ${action}`);
  if (action === 'file') {
    fileMenuOpen = !fileMenuOpen;
    setActiveTab(fileMenuOpen ? 'file' : 'home');
    frame.postMessage({ type: TAB_MSG_TYPE, action, state: fileMenuOpen ? 'show' : 'hide' }, '*');
    return;
  }
  if (fileMenuOpen) {
    // Leaving FILE: close the menu first (engine keeps tab 'file' until menu:hide).
    fileMenuOpen = false;
    frame.postMessage({ type: TAB_MSG_TYPE, action: 'file', state: 'hide' }, '*');
  }
  setActiveTab(action);
  frame.postMessage({ type: TAB_MSG_TYPE, action }, '*');
}

interface DocumentTab {
  id: string;
  doc: OpenedDoc;
  format: { filetype: string; doctype: string; isForm?: string };
  frame: HTMLIFrameElement;
  dirty: boolean;
}

const documentTabs: DocumentTab[] = [];
let activeTabId: string | null = null;
let tabCounter = 0;

function renderDocumentTabs(): void {
  const strip = document.getElementById('doc-tabs-strip');
  if (!strip) return;
  strip.replaceChildren();

  for (const tab of documentTabs) {
    const isActive = tab.id === activeTabId;
    const tabEl = document.createElement('div');
    tabEl.className = `flex items-center gap-2 px-3 py-1 rounded-t text-xs font-medium cursor-pointer transition-colors border-t-2 select-none max-w-[200px] ${
      isActive
        ? 'bg-[#2e2e2e] text-white border-accent shadow-sm'
        : 'bg-[#202020] text-[#888888] hover:bg-[#282828] hover:text-[#cccccc] border-transparent'
    }`;

    const badgeInfo = getFileBadge(tab.format.filetype, tab.doc.name);
    const badge = document.createElement('span');
    badge.className = `flex items-center justify-center w-3.5 h-3.5 rounded-xs ${badgeInfo.bg} text-white font-bold text-[7px] shrink-0`;
    badge.textContent = badgeInfo.text;

    const title = document.createElement('span');
    title.className = 'truncate flex-1 text-[11px]';
    title.textContent = (tab.dirty ? '*' : '') + tab.doc.name;

    const closeBtn = document.createElement('button');
    closeBtn.type = 'button';
    closeBtn.className =
      'w-4 h-4 flex items-center justify-center rounded-full hover:bg-[#444444] text-[#888888] hover:text-white text-xs leading-none transition-colors p-0 border-0 bg-transparent cursor-pointer';
    closeBtn.textContent = '×';
    closeBtn.title = 'Đóng tab';
    closeBtn.addEventListener('click', (ev) => {
      ev.stopPropagation();
      closeTab(tab.id);
    });

    tabEl.append(badge, title, closeBtn);
    tabEl.addEventListener('click', () => {
      switchTab(tab.id);
    });
    strip.appendChild(tabEl);
  }
}

async function syncTouchBar(): Promise<void> {
  try {
    const titles = documentTabs.map((t) => (t.dirty ? '*' : '') + t.doc.name);
    const activeIndex = documentTabs.findIndex((t) => t.id === activeTabId);
    await invoke('touchbar_update', {
      payload: {
        titles,
        active_index: activeIndex >= 0 ? activeIndex : -1,
      },
    });
  } catch (err) {
    console.warn('touchbar_update error:', err);
  }
}

function switchTab(tabId: string): void {
  activeTabId = tabId;
  const current = documentTabs.find((t) => t.id === tabId);
  if (!current) return;

  (window as unknown as { __wasenoffice_file_id__?: number }).__wasenoffice_file_id__ =
    current.doc.fileId;
  engineFrame = current.frame;
  fileMenuOpen = false;

  for (const t of documentTabs) {
    t.frame.classList.toggle('hidden', t.id !== tabId);
  }

  welcome.classList.add('hidden');
  log.classList.add('hidden');
  topBar.classList.remove('hidden');
  editorRoot.classList.remove('hidden');

  const topFn = document.getElementById('top-filename');
  if (topFn) topFn.textContent = current.doc.name;
  setActiveTab('home');

  renderDocumentTabs();
  void syncTouchBar();
}

function closeTab(tabId: string): void {
  const index = documentTabs.findIndex((t) => t.id === tabId);
  if (index === -1) return;

  const [tab] = documentTabs.splice(index, 1);
  tab.frame.remove();

  if (activeTabId === tabId) {
    if (documentTabs.length > 0) {
      const nextIndex = Math.min(index, documentTabs.length - 1);
      switchTab(documentTabs[nextIndex].id);
    } else {
      activeTabId = null;
      editorRoot.classList.add('hidden');
      topBar.classList.add('hidden');
      welcome.classList.remove('hidden');
      log.classList.remove('hidden');
      void renderRecents();
      void syncTouchBar();
    }
  } else {
    renderDocumentTabs();
    void syncTouchBar();
  }
}

/** Welcome out, editing surface in (DD A01 §3 OPENING → exit). */
async function mountEditor(doc: OpenedDoc): Promise<void> {
  const existing = documentTabs.find((t) => t.doc.fileId === doc.fileId);
  if (existing) {
    switchTab(existing.id);
    return;
  }

  const boot = await getBootPage();
  const ext = doc.name.includes('.') ? doc.name.split('.').pop()!.toLowerCase() : '';
  const format = FILE_FORMATS[ext];
  if (!format) {
    logLine(`ERROR: unsupported file extension ".${ext}" (known: ${Object.keys(FILE_FORMATS).join(' ')})`);
    return;
  }

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

  const tabId = 'doc-tab-' + (++tabCounter);
  const frame = document.createElement('iframe');
  frame.id = 'editor-frame-' + tabId;
  frame.className = 'absolute inset-0 h-full w-full border-0';
  frame.src = `${boot}?${params.toString()}`;
  frame.allow = 'clipboard-read; clipboard-write';

  document.getElementById('editor-root')!.appendChild(frame);
  documentTabs.push({ id: tabId, doc, format, frame, dirty: false });

  switchTab(tabId);
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
  // The strip shows ~3 lines: keep the newest (usually the error just raised) in view.
  log.scrollTop = log.scrollHeight;
  void invoke('frontend_log', { level: 'info', message: text }).catch(() => {});
}

// ── Welcome (A01 DD §4): one centered column — brand, actions, recents ──

/** Open dialog of the OS, filtered to the formats this app accepts (DD §5). */
const DOC_FILTERS = [{ name: 'Documents', extensions: ['docx', 'xlsx', 'pptx', 'pdf'] }];
/** Quick view (A3) and Nano Light (L01) are docx-only surfaces whose refusal
 * guard only reaches the log strip — the picker must not offer a file they
 * would silently turn away. */
const DOCX_FILTERS = [{ name: t.docxFilter, extensions: ['docx'] }];
type PathFilters = { name: string; extensions: string[] }[];
async function pickPath(filters: PathFilters = DOC_FILTERS): Promise<string | null> {
  try {
    const picked = await openDialog({ multiple: false, directory: false, filters });
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

function formatOpenedDate(ms: number): string {
  if (!ms || isNaN(ms)) return '';
  const d = new Date(ms);
  const pad = (n: number) => n.toString().padStart(2, '0');
  const day = pad(d.getDate());
  const month = pad(d.getMonth() + 1);
  const year = d.getFullYear();
  const hours = pad(d.getHours());
  const minutes = pad(d.getMinutes());
  return `${day}.${month}.${year} ${hours}:${minutes}`;
}

function getFileBadge(kind: string, name: string): { bg: string; text: string } {
  const ext = (kind || (name.includes('.') ? name.split('.').pop() : ''))?.toLowerCase();
  switch (ext) {
    case 'pdf':
      return { bg: 'bg-[#e53935]', text: 'PDF' };
    case 'docx':
    case 'doc':
      return { bg: 'bg-[#2a77d4]', text: 'DOC' };
    case 'xlsx':
    case 'xls':
      return { bg: 'bg-[#43a047]', text: 'XLS' };
    case 'pptx':
    case 'ppt':
      return { bg: 'bg-[#e66a2b]', text: 'PPT' };
    default:
      return { bg: 'bg-[#616161]', text: ext ? ext.slice(0, 3).toUpperCase() : 'FILE' };
  }
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
  const box = document.getElementById('recents-box')!;
  box.classList.remove('hidden');

  if (rows.length === 0) {
    const emptyLi = document.createElement('li');
    emptyLi.className = 'py-8 text-center text-[#777777] text-sm italic';
    emptyLi.textContent = 'No recent documents';
    list.appendChild(emptyLi);
  } else {
    for (const row of rows) {
      const li = document.createElement('li');
      li.className = `flex cursor-pointer items-center py-2 px-3 rounded-lg hover:bg-[#3d3d3d] transition-colors text-sm text-[#e0e0e0] group ${
        row.exists ? '' : 'opacity-50'
      }`;
      const badgeInfo = getFileBadge(row.kind, row.name);
      const kind = document.createElement('span');
      kind.className = `flex items-center justify-center w-6 h-6 rounded ${badgeInfo.bg} text-white font-bold text-[9px] shrink-0`;
      kind.textContent = badgeInfo.text;

      const name = document.createElement('span');
      name.className = 'flex-1 font-medium truncate text-white ml-3';
      name.textContent = row.name;

      const dir = document.createElement('span');
      dir.className = 'flex-1 truncate text-[#888888] text-xs px-2';
      dir.textContent = row.path;

      const date = document.createElement('span');
      date.className = 'w-40 text-right text-xs text-[#888888] shrink-0';
      date.textContent = formatOpenedDate(row.openedAt);

      const remove = document.createElement('button');
      remove.type = 'button';
      remove.title = t.removeRecent;
      remove.textContent = '×';
      remove.className =
        'w-6 text-right cursor-pointer border-0 bg-transparent p-0 text-base leading-none text-[#666666] opacity-0 group-hover:opacity-100 hover:text-white transition-opacity';
      remove.addEventListener('click', (ev) => {
        ev.stopPropagation();
        void invoke('recents_remove', { path: row.path }).then(() => renderRecents());
      });

      li.append(kind, name, dir, date, remove);
      li.addEventListener('click', () => activateRecent(row));
      list.appendChild(li);
    }
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
    return;
  }
  // T-A46: open the in-page settings view (sidebar Settings click equivalent).
  if (probe === 'settings') {
    showSettings(true);
  }
}

const log: HTMLDivElement = document.createElement('div');
log.id = 'ph0-log';
log.className = 'max-h-[4.5em] overflow-auto px-4 py-1.5 font-mono text-[0.72em] opacity-80 border-t border-[#3c3c3c] bg-[#222222] text-[#f4f4f5]';

document.querySelector<HTMLDivElement>('#app')!.innerHTML = `
  <header id="top-bar" class="hidden flex flex-col border-b border-[#2b2b2b] bg-[#1e1e1e] select-none">
    <!-- Row 1: Document Tab Bar (ONLYOFFICE Style) -->
    <div class="flex items-center h-9 px-3 gap-1.5 border-b border-[#272727] text-xs">
      <!-- Brand / Home button -->
      <button type="button" id="brand-home-btn" class="flex items-center gap-1.5 px-2 py-1 rounded hover:bg-[#2e2e2e] text-white font-semibold cursor-pointer outline-none transition-colors mr-1" title="Trang chủ WasenOffice">
        <img src="/logo.png" alt="WasenOffice" class="h-4 w-4 rounded object-contain" />
        <span class="font-bold tracking-tight text-[11px]">WasenOffice</span>
      </button>

      <!-- Document Tabs Strip -->
      <div id="doc-tabs-strip" class="flex items-center gap-1 overflow-x-auto flex-1 h-full py-0.5"></div>

      <!-- Add New Tab (+) button -->
      <button type="button" id="new-tab-btn" class="flex items-center justify-center w-6 h-6 rounded hover:bg-[#333333] text-[#aaaaaa] hover:text-white text-sm leading-none cursor-pointer outline-none transition-colors" title="Mở tài liệu mới (+)">
        +
      </button>

      <!-- Right controls -->
      <div class="flex items-center gap-2 ml-auto">
        <button type="button" id="settings-btn" class="cursor-pointer border-0 bg-transparent p-1 text-xs text-[#aaaaaa] outline-none hover:text-white" title="Cài đặt (⌘,)">⚙</button>
        <span class="flex h-5 w-5 items-center justify-center rounded-full bg-accent text-[9px] font-semibold text-white" title="WasenOffice User">WU</span>
      </div>
    </div>

    <!-- Row 2: Menu / Tools Tabs (compact chrome row) -->
    <div id="engine-tabs-row" class="hidden grid grid-cols-[1fr_auto_1fr] items-center gap-2 px-3 py-1 bg-[#252525]">
      <nav id="top-tabs" class="flex items-center gap-0.5">${ENGINE_TABS.map(
        (tab) =>
          `<button type="button" id="top-tab-${tab.action}" data-action="${tab.action}" class="${TAB_SKIN} text-fg/70">${tab.label}</button>`
      ).join('')}</nav>
      <div id="top-filename" class="max-w-[28em] truncate text-xs font-medium text-fg/80"></div>
    </div>
  </header>
  <main id="editor-root" class="relative hidden flex-1 bg-paper"></main>
  <section id="welcome" class="flex flex-1 flex-row overflow-hidden bg-[#333333] text-[#e0e0e0] font-sans select-none">
    <!-- Left Sidebar (ONLYOFFICE Style) -->
    <aside class="w-56 bg-[#252525] border-r border-[#1a1a1a] flex flex-col justify-between p-3 select-none shrink-0 text-sm">
      <div class="flex flex-col gap-1">
        <!-- Logo & Brand -->
        <div class="flex items-center gap-2.5 px-3 py-3 mb-2">
          <img src="/logo.png" alt="WasenOffice" class="h-6 w-6 rounded object-contain" />
          <span class="font-bold tracking-tight text-white text-base">WasenOffice</span>
        </div>

        <!-- Navigation items -->
        <button type="button" id="sidebar-home-btn" class="w-full flex items-center gap-3 px-3 py-2.5 rounded-md bg-[#383838] text-white font-medium outline-none text-left cursor-pointer transition-colors">
          <svg class="w-4 h-4 text-white" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"></path><polyline points="9 22 9 12 15 12 15 22"></polyline></svg>
          <span>Home</span>
        </button>

        <button type="button" id="welcome-open" class="w-full flex items-center gap-3 px-3 py-2.5 rounded-md text-[#bbbbbb] hover:bg-[#2e2e2e] hover:text-white outline-none text-left cursor-pointer transition-colors">
          <svg class="w-4 h-4 text-[#bbbbbb]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"></path></svg>
          <span>Open local file</span>
        </button>

        <button type="button" id="sidebar-templates-btn" class="w-full flex items-center gap-3 px-3 py-2.5 rounded-md text-[#bbbbbb] hover:bg-[#2e2e2e] hover:text-white outline-none text-left cursor-pointer transition-colors">
          <svg class="w-4 h-4 text-[#bbbbbb]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="7" height="7"></rect><rect x="14" y="3" width="7" height="7"></rect><rect x="14" y="14" width="7" height="7"></rect><rect x="3" y="14" width="7" height="7"></rect></svg>
          <span>Templates</span>
        </button>

        <div class="mt-6 px-3 flex items-center justify-between text-xs font-semibold text-[#777777] uppercase tracking-wider">
          <span>Clouds</span>
          <button type="button" class="text-base leading-none text-[#777777] hover:text-white cursor-pointer">+</button>
        </div>
      </div>

      <!-- Bottom items -->
      <div class="flex flex-col gap-1 border-t border-[#333333] pt-3">
        <button type="button" id="sidebar-ai-btn" class="w-full flex items-center gap-3 px-3 py-2.5 rounded-md text-[#bbbbbb] hover:bg-[#2e2e2e] hover:text-white outline-none text-left cursor-pointer transition-colors">
          <svg class="w-4 h-4 text-[#bbbbbb]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"></polygon></svg>
          <span>AI agent</span>
        </button>

        <button type="button" id="sidebar-settings-btn" class="w-full flex items-center gap-3 px-3 py-2.5 rounded-md text-[#bbbbbb] hover:bg-[#2e2e2e] hover:text-white outline-none text-left cursor-pointer transition-colors">
          <svg class="w-4 h-4 text-[#bbbbbb]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="4" y1="21" x2="4" y2="14"></line><line x1="4" y1="10" x2="4" y2="3"></line><line x1="12" y1="21" x2="12" y2="12"></line><line x1="12" y1="8" x2="12" y2="3"></line><line x1="20" y1="21" x2="20" y2="16"></line><line x1="20" y1="12" x2="20" y2="3"></line><line x1="1" y1="14" x2="7" y2="14"></line><line x1="9" y1="8" x2="15" y2="8"></line><line x1="17" y1="16" x2="23" y2="16"></line></svg>
          <span>Settings</span>
        </button>
      </div>
    </aside>

    <!-- Main Content Area -->
    <main id="view-home" class="flex-1 bg-[#333333] flex flex-col overflow-y-auto px-10 py-8 gap-8">
      <!-- 4 Creation Cards -->
      <section class="flex flex-wrap items-center gap-6">
        <!-- DOCX Card -->
        <button type="button" id="create-docx" class="group flex flex-col items-center justify-between w-40 h-44 bg-[#3c3c3c] hover:bg-[#484848] rounded-xl p-4 cursor-pointer transition-all border border-transparent hover:border-[#555555] shadow-md outline-none">
          <div class="relative w-20 h-24 bg-white rounded-md flex flex-col p-2.5 shadow transition-transform group-hover:scale-105">
            <span class="absolute -top-2 -left-2 bg-[#2a77d4] text-white text-[9px] font-black px-1.5 py-0.5 rounded shadow">DOCX</span>
            <div class="flex-1 flex flex-col justify-center gap-1.5 pt-1.5">
              <div class="h-1 bg-[#90caf9] rounded-full w-full"></div>
              <div class="h-1 bg-[#90caf9] rounded-full w-4/5"></div>
              <div class="h-1 bg-[#90caf9] rounded-full w-full"></div>
              <div class="h-1 bg-[#90caf9] rounded-full w-3/4"></div>
              <div class="h-1 bg-[#90caf9] rounded-full w-5/6"></div>
            </div>
          </div>
          <span class="text-sm font-medium text-[#d8d8d8] group-hover:text-white transition-colors">Document</span>
        </button>

        <!-- XLSX Card -->
        <button type="button" id="create-xlsx" class="group flex flex-col items-center justify-between w-40 h-44 bg-[#3c3c3c] hover:bg-[#484848] rounded-xl p-4 cursor-pointer transition-all border border-transparent hover:border-[#555555] shadow-md outline-none">
          <div class="relative w-20 h-24 bg-white rounded-md flex flex-col p-2 shadow transition-transform group-hover:scale-105">
            <span class="absolute -top-2 -left-2 bg-[#43a047] text-white text-[9px] font-black px-1.5 py-0.5 rounded shadow">XLSX</span>
            <div class="flex-1 grid grid-cols-4 grid-rows-5 gap-0.5 pt-1.5">
              <div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div>
              <div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div>
              <div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div>
              <div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div>
              <div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div><div class="bg-[#c8e6c9] rounded-xs"></div>
            </div>
          </div>
          <span class="text-sm font-medium text-[#d8d8d8] group-hover:text-white transition-colors">Spreadsheet</span>
        </button>

        <!-- PPTX Card -->
        <button type="button" id="create-pptx" class="group flex flex-col items-center justify-between w-40 h-44 bg-[#3c3c3c] hover:bg-[#484848] rounded-xl p-4 cursor-pointer transition-all border border-transparent hover:border-[#555555] shadow-md outline-none">
          <div class="relative w-24 h-20 bg-white rounded-md flex flex-col p-2 shadow my-auto transition-transform group-hover:scale-105">
            <span class="absolute -top-2 -left-2 bg-[#e66a2b] text-white text-[9px] font-black px-1.5 py-0.5 rounded shadow">PPTX</span>
            <div class="flex-1 grid grid-cols-3 gap-1 pt-1.5">
              <div class="bg-[#ffccbc] rounded-xs col-span-1"></div>
              <div class="flex flex-col gap-1 col-span-2">
                <div class="bg-[#ffccbc] rounded-xs h-2.5"></div>
                <div class="bg-[#ffe0b2] rounded-xs flex-1"></div>
              </div>
            </div>
          </div>
          <span class="text-sm font-medium text-[#d8d8d8] group-hover:text-white transition-colors">Presentation</span>
        </button>

        <!-- PDF Card -->
        <button type="button" id="create-pdf" class="group flex flex-col items-center justify-between w-40 h-44 bg-[#3c3c3c] hover:bg-[#484848] rounded-xl p-4 cursor-pointer transition-all border border-transparent hover:border-[#555555] shadow-md outline-none">
          <div class="relative w-20 h-24 bg-white rounded-md flex flex-col p-2 shadow transition-transform group-hover:scale-105">
            <span class="absolute -top-2 -left-2 bg-[#e53935] text-white text-[9px] font-black px-1.5 py-0.5 rounded shadow">PDF</span>
            <div class="flex-1 flex flex-col justify-center gap-1.5 pt-1.5">
              <div class="h-1.5 bg-[#ffcdd2] rounded-xs w-full"></div>
              <div class="grid grid-cols-2 gap-1 flex-1">
                <div class="bg-[#ffebee] rounded-xs"></div>
                <div class="bg-[#ffebee] rounded-xs"></div>
              </div>
              <div class="h-1 bg-[#ffcdd2] rounded-full w-2/3"></div>
            </div>
          </div>
          <span class="text-sm font-medium text-[#d8d8d8] group-hover:text-white transition-colors">PDF</span>
        </button>
      </section>

      <!-- Recent files Section -->
      <section id="recents-box" class="flex flex-col gap-2">
        <h2 class="m-0 text-xl font-bold text-white tracking-tight">Recent files</h2>
        <div class="flex items-center text-xs font-semibold text-[#888888] pb-2 border-b border-[#444444] px-3">
          <span class="flex-1">File name</span>
          <span class="flex-1">Location</span>
          <span class="w-40 text-right">Last opened</span>
          <span class="w-6"></span>
        </div>
        <ul id="recents-list" class="m-0 flex w-full list-none flex-col gap-0.5 p-0"></ul>
      </section>

      <!-- Auxiliary / Dev Controls (collapsible/clean footer) -->
      <footer class="mt-auto pt-6 border-t border-[#3c3c3c] flex flex-wrap items-center justify-between text-xs text-[#777777] gap-3">
        <div class="flex items-center gap-3">
          <button type="button" id="welcome-quick" class="hover:text-white transition-colors cursor-pointer">${t.quickView}</button>
          <span>•</span>
          <button type="button" id="welcome-nano" class="hover:text-white transition-colors cursor-pointer">${t.nanoLight}</button>
          <span>•</span>
          <button type="button" id="paste-toggle" class="hover:text-white underline cursor-pointer">${t.orPastePath}</button>
        </div>
        <form id="open-form" class="hidden flex items-center gap-2">
          <input
            id="doc-path"
            type="text"
            placeholder="/absolute/path/to/document (.docx .xlsx .pptx .pdf)"
            class="w-[22em] bg-[#27272a] border border-[#444444] text-[#f4f4f5] px-2.5 py-1 rounded text-xs outline-none focus:border-accent"
          />
          <button type="submit" class="bg-[#383838] hover:bg-[#484848] text-white px-2.5 py-1 rounded text-xs cursor-pointer">${t.open}</button>
          <button type="button" id="quick-view-btn" class="bg-[#383838] hover:bg-[#484848] text-white px-2.5 py-1 rounded text-xs cursor-pointer">${t.quickView}</button>
          <button type="button" id="nano-light-btn" class="bg-[#383838] hover:bg-[#484848] text-white px-2.5 py-1 rounded text-xs cursor-pointer">${t.nanoLight}</button>
        </form>
        <span id="version" class="text-[0.75em] text-[#666666]"></span>
      </footer>
    </main>

    <!-- Settings view (T-A46 — K02-Q3 amended): sidebar item swaps THIS in
         place of the home view; the separate white settings window is gone. -->
    <section id="settings-view" class="hidden flex-1 flex-col overflow-hidden bg-[#333333] text-[#e0e0e0] font-sans select-none"></section>

    <!-- AI Agent & MCP Tools view: sidebar AI agent item opens THIS dedicated screen -->
    <section id="ai-view" class="hidden flex-1 flex-col overflow-hidden bg-[#333333] text-[#e0e0e0] font-sans select-none"></section>
  </section>
`;
const app = document.querySelector<HTMLDivElement>('#app')!;
app.appendChild(log);
const welcome = document.getElementById('welcome')!;
const topBar = document.getElementById('top-bar')!;
const editorRoot = document.getElementById('editor-root')!;
const viewHome = document.getElementById('view-home')!;
const settingsViewEl = document.getElementById('settings-view')!;
const aiViewEl = document.getElementById('ai-view')!;
const sidebarHomeBtn = document.getElementById('sidebar-home-btn') as HTMLButtonElement;
const sidebarSettingsBtn = document.getElementById('sidebar-settings-btn') as HTMLButtonElement;
const sidebarAiBtn = document.getElementById('sidebar-ai-btn') as HTMLButtonElement;

type ActiveView = 'home' | 'settings' | 'ai';
let activeView: ActiveView = 'home';
const SIDEBAR_ACTIVE = ['bg-[#383838]', 'text-white', 'font-medium'];
const SIDEBAR_IDLE = ['text-[#bbbbbb]', 'hover:bg-[#2e2e2e]', 'hover:text-white'];

function setSidebarActive(btn: HTMLButtonElement, active: boolean): void {
  for (const cls of [...SIDEBAR_ACTIVE, ...SIDEBAR_IDLE]) btn.classList.remove(cls);
  btn.classList.add(...(active ? SIDEBAR_ACTIVE : SIDEBAR_IDLE));
}

function setActiveView(view: ActiveView, tab?: SettingsTab): void {
  activeView = view;
  viewHome.classList.toggle('hidden', view !== 'home');
  settingsViewEl.classList.toggle('hidden', view !== 'settings');
  aiViewEl.classList.toggle('hidden', view !== 'ai');

  setSidebarActive(sidebarHomeBtn, view === 'home');
  setSidebarActive(sidebarSettingsBtn, view === 'settings');
  setSidebarActive(sidebarAiBtn, view === 'ai');

  if (view === 'settings') {
    settingsView.show(tab);
    aiView.hide();
  } else if (view === 'ai') {
    aiView.show();
    settingsView.hide();
  } else {
    settingsView.hide();
    aiView.hide();
  }
}

function showSettings(open: boolean, tab?: SettingsTab): void {
  setActiveView(open ? 'settings' : 'home', tab);
}

const settingsView = createSettingsView(settingsViewEl, {
  onClose: () => setActiveView('home'),
  log: logLine,
});

const aiView = createAiView(aiViewEl, {
  onClose: () => setActiveView('home'),
  log: logLine,
});

// T-A40: header menu tabs drive the engine tools box through the shim.
for (const tab of ENGINE_TABS) {
  const btn = document.getElementById(`top-tab-${tab.action}`) as HTMLButtonElement;
  tabButtons.set(tab.action, btn);
  btn.addEventListener('click', () => sendTab(tab.action));
}
setActiveTab('home');

function goHome(): void {
  for (const t of documentTabs) {
    t.frame.classList.add('hidden');
  }
  editorRoot.classList.add('hidden');
  welcome.classList.remove('hidden');
  log.classList.remove('hidden');
  renderDocumentTabs();
}

document.getElementById('brand-home-btn')?.addEventListener('click', () => {
  goHome();
  showSettings(false);
});

// The top-bar gear lives in the editor chrome: returning to the start page
// with the settings view open is the in-page equivalent of the old popup.
document.querySelector<HTMLButtonElement>('#settings-btn')!.addEventListener('click', () => {
  goHome();
  showSettings(true);
});

document.getElementById('new-tab-btn')?.addEventListener('click', async () => {
  const path = await pickPath();
  if (path) void openPath(path);
});

// Touch Bar interaction listener (macOS MacBook Pro Touch Bar)
try {
  void listen<{ action: string; index: number }>('touchbar-action', (event) => {
    const { action, index } = event.payload;
    if (action === 'select') {
      if (index >= 0 && index < documentTabs.length) {
        switchTab(documentTabs[index].id);
      }
    } else if (action === 'new') {
      void pickPath().then((path) => {
        if (path) void openPath(path);
      });
    } else if (action === 'save') {
      if (engineFrame?.contentWindow) {
        engineFrame.contentWindow.postMessage(
          { type: 'asc_desktop_editor', command: 'save' },
          '*'
        );
      }
    } else if (action === 'close') {
      if (activeTabId) {
        closeTab(activeTabId);
      }
    }
  }).catch((err) => {
    console.warn('listen touchbar-action failed:', err);
  });
} catch (e) {
  console.warn('setup touchbar listener failed:', e);
}

// Cmd-W keyboard shortcut to close active document tab
window.addEventListener('keydown', (ev) => {
  if ((ev.metaKey || ev.ctrlKey) && ev.key.toLowerCase() === 'w') {
    if (activeTabId && !welcome.classList.contains('hidden')) {
      ev.preventDefault();
      closeTab(activeTabId);
    }
  }
});

sidebarSettingsBtn.addEventListener('click', () => {
  setActiveView(activeView === 'settings' ? 'home' : 'settings');
});
sidebarHomeBtn.addEventListener('click', () => {
  setActiveView('home');
});
sidebarAiBtn.addEventListener('click', () => {
  setActiveView(activeView === 'ai' ? 'home' : 'ai');
});
document.getElementById('create-pdf')?.addEventListener('click', async () => {
  const path = await pickPath([{ name: 'PDF documents (.pdf)', extensions: ['pdf'] }]);
  if (path) void openPath(path);
});

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
  const path = await pickPath(DOCX_FILTERS);
  if (path) void quickView(path);
});
document.querySelector<HTMLButtonElement>('#welcome-nano')!.addEventListener('click', async () => {
  const path = await pickPath(DOCX_FILTERS);
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

// Keymap (T-A41): load once, re-read on focus (settings window may have
// changed it — lazy refresh beats cross-window event wiring, DD K02 §4).
void loadKeymap().then(() => {
  bindShellKeys();
  bindKeymapPush();
});
window.addEventListener('focus', () => void loadKeymap());

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
