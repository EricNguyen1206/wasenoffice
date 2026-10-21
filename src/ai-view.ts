/**
 * AI Agent & MCP Tools View
 *
 * Dedicated start-page view for Model Context Protocol (MCP) server configuration,
 * external AI assistant integrations, and native WasenOffice AI tools documentation.
 * Built using the WasenOffice Atomic Design component system.
 */

import { invoke } from '@tauri-apps/api/core';
import {
  createButton,
  createHeading,
  createSubheading,
  createIcon,
  createCopyBox,
  createMcpClientList,
  createAiToolsGrid,
} from './components';

export interface AiView {
  show(): void;
  hide(): void;
}

export function createAiView(
  host: HTMLElement,
  opts: { onClose: () => void; log: (msg: string) => void },
): AiView {
  host.innerHTML = '';

  const scrollWrapper = document.createElement('div');
  scrollWrapper.className = 'flex-1 overflow-y-auto px-10 py-8';

  const container = document.createElement('div');
  container.className = 'mx-auto max-w-4xl flex flex-col gap-8';

  // ── Header ──────────────────────────────────────────────────────────────
  const header = document.createElement('div');
  header.className = 'flex items-center justify-between gap-4';

  const titleGroup = document.createElement('div');
  titleGroup.className = 'flex items-center gap-3.5';

  const iconBadge = document.createElement('div');
  iconBadge.className =
    'flex h-11 w-11 items-center justify-center rounded-xl bg-gradient-to-br from-[#2a77d4] via-[#4f46e5] to-[#7c3aed] text-white shadow-lg shrink-0';
  iconBadge.appendChild(createIcon('ai', 'w-6 h-6'));

  const textGroup = document.createElement('div');
  const h1 = createHeading(1, 'AI Agent & MCP Tools');
  const sub = createSubheading(
    'Cấu hình Model Context Protocol (MCP) và các công cụ AI tự động hóa tài liệu.',
  );
  textGroup.appendChild(h1);
  textGroup.appendChild(sub);

  titleGroup.appendChild(iconBadge);
  titleGroup.appendChild(textGroup);

  const closeBtn = createButton({
    label: '✕ Đóng',
    variant: 'secondary',
    size: 'sm',
    onClick: () => opts.onClose(),
  });

  header.appendChild(titleGroup);
  header.appendChild(closeBtn.element);
  container.appendChild(header);

  // ── Section 1: External AI Assistant Connections (Organism) ─────────────
  const mcpList = createMcpClientList({
    onError: (msg) => opts.log(msg),
  });
  container.appendChild(mcpList.element);

  // ── Section 2: Built-in AI Tools (Organism) ──────────────────────────────
  const toolsGrid = createAiToolsGrid();
  container.appendChild(toolsGrid);

  // ── Section 3: Custom MCP Server Config (Molecule) ──────────────────────
  const copyBox = createCopyBox({
    title: 'Cấu hình Custom MCP Server (JSON)',
    description:
      'Nếu bạn sử dụng công cụ chưa hỗ trợ tự động (như Cursor, Windsurf, Zed...), hãy sao chép đoạn JSON dưới đây vào mục cấu hình MCP Servers:',
    code: '',
    buttonLabel: 'Copy MCP config',
  });
  container.appendChild(copyBox.element);

  scrollWrapper.appendChild(container);
  host.appendChild(scrollWrapper);

  async function loadSnippet(): Promise<void> {
    try {
      const snippet = await invoke<string>('mcp_get_config_snippet');
      copyBox.setCode(snippet);
    } catch (err) {
      opts.log(`ERROR: load snippet: ${err}`);
    }
  }

  return {
    show(): void {
      void Promise.all([mcpList.refresh(), loadSnippet()]);
    },
    hide(): void {},
  };
}
