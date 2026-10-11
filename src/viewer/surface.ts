/**
 * DocumentSurface (T-064, Candidate 4 from Architecture Review #7).
 *
 * Deep module encapsulating the canvas viewport lifecycle for both read-only
 * (Quick View) and editable (Nano Light) surfaces:
 * - Canvas context & devicePixelRatio setup
 * - Viewport dimension measurement (window vs container)
 * - Coordinated layoutBlocks() + drawModel() execution
 * - Common inked-pixel diagnostic calculation
 * - Warning & fatal diagnostic banner management
 *
 * This provides a single, uniform surface host interface ready for Milestone A5 chrome.
 */
import {
  canvasMeasurer,
  columnWidth,
  drawFatal,
  drawModel,
  pageOrigin,
} from './render';
import {
  layoutBlocks,
  type Block,
  type FontMetrics,
  type LineModel,
  type Measurer,
} from './layout';

export interface LayoutOptions {
  blocks: Block[];
  fonts: FontMetrics;
  page?: { w: number | null; h: number | null; orient?: string | null } | null;
  mar?: { top: number | null; right: number | null; bottom: number | null; left: number | null } | null;
}

export interface DrawOptions {
  bitmaps: ReadonlyMap<string, ImageBitmap>;
  overlay?: (ctx: CanvasRenderingContext2D) => void;
}

export class DocumentSurface {
  readonly cv: HTMLCanvasElement;
  readonly ctx: CanvasRenderingContext2D;
  readonly dpr: number;
  readonly measure: Measurer;
  readonly container: HTMLElement;

  constructor(canvasOrId: HTMLCanvasElement | string = 'cv') {
    const el = typeof canvasOrId === 'string'
      ? document.getElementById(canvasOrId)
      : canvasOrId;
    if (!(el instanceof HTMLCanvasElement)) {
      throw new Error(`canvas element not found: ${canvasOrId}`);
    }
    this.cv = el;
    this.container = this.cv.parentElement || document.body;
    this.dpr = window.devicePixelRatio || 1;
    const ctx = this.cv.getContext('2d');
    if (!ctx) {
      throw new Error('canvas 2d context not available');
    }
    this.ctx = ctx;
    this.measure = canvasMeasurer(this.ctx);
  }

  get vw(): number {
    return this.container === document.body ? window.innerWidth : this.container.clientWidth;
  }

  get vh(): number {
    return this.container === document.body ? window.innerHeight : this.container.clientHeight;
  }

  layout(opts: LayoutOptions): { colW: number; model: LineModel } {
    const colW = columnWidth(opts.page, opts.mar, this.vw);
    const model = layoutBlocks({
      blocks: opts.blocks,
      ...pageOrigin(opts.mar),
      w: colW,
      fonts: opts.fonts,
      measure: this.measure,
    });
    return { colW, model };
  }

  draw(model: LineModel, opts: DrawOptions): number {
    return drawModel(
      model,
      this.cv,
      this.ctx,
      this.dpr,
      this.vw,
      this.vh,
      opts.bitmaps,
      opts.overlay,
    );
  }

  layoutAndDraw(layoutOpts: LayoutOptions, drawOpts: DrawOptions): { colW: number; model: LineModel; docH: number } {
    const { colW, model } = this.layout(layoutOpts);
    const docH = this.draw(model, drawOpts);
    return { colW, model, docH };
  }

  countInked(): number {
    const img = this.ctx.getImageData(0, 0, this.cv.width, this.cv.height).data;
    let inked = 0;
    for (let i = 0; i < img.length; i += 4 * 97) {
      if (img[i] < 128) inked++;
    }
    return inked;
  }

  showFatal(message: string): void {
    drawFatal(this.cv, this.ctx, this.dpr, this.vw, this.vh, message);
  }

  showWarnings(warnings: readonly string[]): void {
    if (!warnings || warnings.length === 0) return;
    const banner = document.createElement('div');
    banner.textContent = `Nano Light giữ được văn bản cơ bản; sẽ MẤT khi save: ${warnings.join(', ')}. Dùng chế độ nâng cao để giữ đủ.`;
    banner.style.cssText =
      'position:fixed;top:0;left:0;right:0;z-index:9;padding:6px 12px;background:#b45309;color:#fff;font:13px system-ui';
    document.body.appendChild(banner);
  }
}
