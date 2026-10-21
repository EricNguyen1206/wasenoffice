/**
 * Settings view INSIDE the start page (T-A46 — K02-Q3 amended 20.10: the
 * separate white settings window is gone; the ONLYOFFICE pattern is
 * sidebar-item → main-area swap, one UI system). Self-contained like
 * DocumentSurface (T-064): owns its markup, tabs, keymap editor and MCP
 * toggles; the start page mounts it hidden inside the welcome main area and
 * toggles visibility. VI-only (K02). Theme = start-page Graphite tokens.
 *
 * The fake "Agents"/"Agent Names"/"Chat" tabs of the old window are NOT
 * ported (diagnostics 20.10: fabricated data for agents that don't exist —
 * the honest agent story is the MCP tab, where external agents connect).
 */
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';

interface Chord {
  cmd: boolean;
  ctrl: boolean;
  shift: boolean;
  alt: boolean;
  key: string;
}

interface Entry {
  id: string;
  label: string;
  group: string;
  scope: 'engine' | 'shell';
  chord: Chord;
  default_chord: Chord | null;
  keys: string;
  hint: string;
}

const BTN =
  'cursor-pointer rounded-lg border border-[#555555] bg-[#383838] px-2.5 py-1 text-xs font-medium text-[#d8d8d8] shadow-xs outline-none hover:bg-[#484848]';

export type SettingsTab = 'keymap' | 'account';

export interface SettingsView {
  show(tab?: SettingsTab): void;
  hide(): void;
  /** True while a keymap chord capture is armed — the shell keydown handler
   * must stand down (a captured ⌘W must not close the window mid-capture). */
  isCapturing(): boolean;
}

export function createSettingsView(
  host: HTMLElement,
  opts: { onClose: () => void; log: (msg: string) => void },
): SettingsView {
  host.innerHTML = `
    <div class="flex-1 overflow-y-auto px-10 py-8">
      <div class="mx-auto max-w-3xl">
        <div class="mb-6 flex items-center justify-between border-b border-[#4a4a4a] pb-3">
          <nav class="flex items-center gap-1">
            <button type="button" data-sv-tab="keymap" class="sv-tab cursor-pointer rounded-lg px-3.5 py-1.5 text-[13px] font-medium outline-none transition-colors">Phím tắt</button>
            <button type="button" data-sv-tab="account" class="sv-tab cursor-pointer rounded-lg px-3.5 py-1.5 text-[13px] font-medium outline-none transition-colors">Account & License</button>
          </nav>
          <button type="button" id="sv-close-btn" class="cursor-pointer rounded-lg border border-[#555555] bg-[#383838] px-3 py-1.5 text-xs font-medium text-[#d8d8d8] shadow-xs outline-none hover:bg-[#484848] transition-colors">
            ✕ Đóng
          </button>
        </div>

        <section id="sv-panel-keymap">
          <div class="mb-4 flex items-center justify-between">
            <div>
              <h1 class="text-2xl font-bold tracking-tight text-white">Cài đặt · Phím tắt</h1>
              <p class="mt-1 text-[13.5px] text-[#bbbbbb]">Tùy biến phím tắt và thao tác trong ứng dụng.</p>
            </div>
            <button type="button" id="sv-reset-all" class="hidden ${BTN}">Khôi phục tất cả</button>
          </div>
          <div id="sv-rows" class="space-y-1.5"></div>
        </section>

        <section id="sv-panel-account" class="hidden">
          <h1 class="text-2xl font-bold tracking-tight text-white">Account & License</h1>
          <p class="mt-1 mb-6 text-[14px] text-[#bbbbbb]">Thông tin phiên bản và giấy phép phần mềm.</p>
          <div class="space-y-3 rounded-xl border border-[#4a4a4a] bg-[#3c3c3c] p-6 shadow-xs">
            <div class="flex justify-between border-b border-[#4a4a4a] py-2 text-sm">
              <span class="text-[#bbbbbb]">Phiên bản:</span>
              <span id="sv-version" class="font-medium text-[#e0e0e0]">WasenOffice Desktop</span>
            </div>
            <div class="flex justify-between border-b border-[#4a4a4a] py-2 text-sm">
              <span class="text-[#bbbbbb]">Bản quyền:</span>
              <span class="font-medium text-[#e0e0e0]">MIT License</span>
            </div>
            <div class="flex justify-between py-2 text-sm">
              <span class="text-[#bbbbbb]">MCP Protocol:</span>
              <span class="font-medium text-[#e0e0e0]">2024-11-05 (stdio)</span>
            </div>
          </div>
        </section>
      </div>
    </div>`;

  document.getElementById('sv-close-btn')?.addEventListener('click', () => opts.onClose());

  // ── tabs ─────────────────────────────────────────────────────────────────
  const tabs = [...host.querySelectorAll<HTMLButtonElement>('.sv-tab')];
  const panels: Record<SettingsTab, string> = {
    keymap: 'sv-panel-keymap',
    account: 'sv-panel-account',
  };
  const TAB_ACTIVE = ['bg-[#383838]', 'text-white'];
  const TAB_IDLE = ['text-[#bbbbbb]', 'hover:bg-[#2e2e2e]', 'hover:text-white'];
  let visible = false;
  let currentTab: SettingsTab = 'keymap';

  function selectTab(tab: SettingsTab): void {
    currentTab = tab;
    for (const t of tabs) {
      const active = t.dataset.svTab === tab;
      // classList tokens are single names — toggle()/add() reject spaces.
      for (const cls of [...TAB_ACTIVE, ...TAB_IDLE]) t.classList.remove(cls);
      t.classList.add(...(active ? TAB_ACTIVE : TAB_IDLE));
      document.getElementById(panels[t.dataset.svTab as SettingsTab])!.classList.toggle('hidden', !active);
    }
  }
  for (const t of tabs) t.addEventListener('click', () => selectTab(t.dataset.svTab as SettingsTab));

  // ── keymap editor (ported verbatim from the old window, dark skin) ───────
  let entries: Entry[] = [];
  let capturing: string | null = null;
  let pendingChord: Chord | null = null;

  function chordEquals(a: Chord, b: Chord): boolean {
    return (
      a.cmd === b.cmd &&
      a.ctrl === b.ctrl &&
      a.shift === b.shift &&
      a.alt === b.alt &&
      a.key.toUpperCase() === b.key.toUpperCase()
    );
  }

  function toChord(ev: KeyboardEvent): Chord | null {
    const key = ev.key.length === 1 ? ev.key : '';
    if (!key) return null;
    return { cmd: ev.metaKey, ctrl: ev.ctrlKey, shift: ev.shiftKey, alt: ev.altKey, key };
  }

  async function refreshKeymap(): Promise<void> {
    entries = await invoke<Entry[]>('keymap_load');
    renderKeymap();
    document.getElementById('sv-reset-all')?.classList.toggle(
      'hidden',
      !entries.some((e) => e.default_chord && !chordEquals(e.chord, e.default_chord)),
    );
  }

  function renderKeymap(): void {
    const rowsHost = document.getElementById('sv-rows')!;
    let html = '';
    let group = '';
    for (const e of entries) {
      if (e.group !== group) {
        group = e.group;
        html += `<h2 class="mb-2 mt-4 text-[0.75em] font-semibold uppercase tracking-wide text-[#888888] first:mt-0">${group}</h2>`;
      }
      const editing = capturing === e.id;
      html += `<div class="mb-1.5 flex items-center gap-3 rounded-xl border px-3.5 py-2.5 bg-[#3c3c3c] shadow-xs ${editing ? 'border-accent ring-2 ring-accent/30' : 'border-[#4a4a4a]'}">
        <span class="flex-1 text-[0.9em] font-medium text-[#e0e0e0]">${e.label}</span>${editCell(e)}
      </div>`;
    }
    rowsHost.innerHTML = html;

    for (const e of entries) {
      document.getElementById(`sv-km-${e.id}`)?.addEventListener('click', () => startCapture(e.id));
      document.getElementById(`sv-km-apply-${e.id}`)?.addEventListener('click', () => void applyOverride(e));
      document.getElementById(`sv-km-cancel-${e.id}`)?.addEventListener('click', () => {
        capturing = null;
        pendingChord = null;
        renderKeymap();
      });
      document.getElementById(`sv-km-reset-${e.id}`)?.addEventListener('click', () => void resetOne(e));
    }
  }

  function editCell(e: Entry): string {
    if (capturing === e.id) {
      const target = entries.find((o) => o.id !== e.id && pendingChord && chordEquals(o.chord, pendingChord));
      const note = pendingChord
        ? target
          ? `<span class="text-[0.8em] font-medium text-amber-400">Đè "${target.label}"?</span>`
          : `<span class="text-[0.8em] font-semibold text-white">${pendingChord.cmd ? '⌘' : ''}${pendingChord.key.toUpperCase()}</span>`
        : `<span class="text-[0.8em] text-[#bbbbbb]">Nhấn tổ hợp phím… (Esc hủy)</span>`;
      return `${note}
        ${pendingChord && target ? `<button type="button" id="sv-km-apply-${e.id}" class="${BTN} font-semibold">Đè</button>` : ''}
        <button type="button" id="sv-km-cancel-${e.id}" class="${BTN}">Hủy</button>`;
    }
    const reset =
      e.default_chord && !chordEquals(e.chord, e.default_chord)
        ? `<button type="button" id="sv-km-reset-${e.id}" class="${BTN}" title="Khôi phục mặc định">↺</button>`
        : '';
    return `<kbd class="rounded-md border border-[#555555] bg-[#2a2a2a] px-2 py-0.5 text-[0.85em] font-mono text-[#e0e0e0]">${e.keys}</kbd>${reset}
      <button type="button" id="sv-km-${e.id}" class="${BTN}">Đổi</button>`;
  }

  function startCapture(id: string): void {
    capturing = id;
    pendingChord = null;
    renderKeymap();
  }

  async function applyOverride(e: Entry): Promise<void> {
    if (!pendingChord) return;
    const isDefault = e.default_chord && chordEquals(e.default_chord, pendingChord);
    const map = await currentOverrides();
    if (isDefault) map.delete(e.id);
    else map.set(e.id, pendingChord);
    entries = await invoke<Entry[]>('keymap_save', { overrides: Object.fromEntries(map) });
    capturing = null;
    pendingChord = null;
    renderKeymap();
  }

  async function resetOne(e: Entry): Promise<void> {
    const map = await currentOverrides();
    map.delete(e.id);
    entries = await invoke<Entry[]>('keymap_save', { overrides: Object.fromEntries(map) });
    renderKeymap();
  }

  async function currentOverrides(): Promise<Map<string, Chord>> {
    const map = new Map<string, Chord>();
    for (const e of entries) {
      if (e.default_chord && !chordEquals(e.chord, e.default_chord)) map.set(e.id, e.chord);
    }
    return map;
  }

  document.getElementById('sv-reset-all')?.addEventListener('click', () => {
    void invoke<Entry[]>('keymap_save', { overrides: {} }).then((fresh) => {
      entries = fresh;
      renderKeymap();
    });
  });

  // ── MCP toggles + config copy (ported; visuals now class-driven) ─────────
  async function refreshMcp(): Promise<boolean> {
    try {
      await invoke('mcp_get_status');
      return true;
    } catch {
      return false;
    }
  }

  // ── show / hide + capture keydown (gated by visibility) ──────────────────
  window.addEventListener('keydown', (ev) => {
    if (!visible) return;
    if (ev.key === 'Escape') {
      if (capturing) {
        capturing = null;
        pendingChord = null;
        renderKeymap();
        return;
      }
      opts.onClose();
      return;
    }
    if (capturing) {
      ev.preventDefault();
      ev.stopPropagation();
      pendingChord = toChord(ev);
      renderKeymap();
    }
  });

  return {
    show(tab?: SettingsTab): void {
      visible = true;
      selectTab(tab ?? currentTab);
      void getVersion()
        .then((v) => {
          const el = document.getElementById('sv-version');
          if (el) el.textContent = `WasenOffice Desktop v${v}`;
        })
        .catch(() => {});
      void Promise.all([refreshKeymap().catch((err) => opts.log(`ERROR: settings keymap: ${err}`)), refreshMcp()]).then(
        ([, mcpOk]) => {
          // ADDITIVE log contract (gate 17-settings; SMOKE_RE_SP_SETTINGS) — change in the SAME commit.
          opts.log(`settings view: keymap=${entries.length} rows mcp=${mcpOk ? 'ok' : 'fail'}`);
        },
      );
    },
    hide(): void {
      visible = false;
      capturing = null;
      pendingChord = null;
    },
    isCapturing(): boolean {
      return visible && capturing !== null;
    },
  };
}
