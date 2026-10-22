import {
  createOfficeViewerError,
  type OfficeViewerError,
  type OfficeWatermarkConfig,
  type OfficeWatermarkOptions,
} from '@office-view-suite/viewer-core';

type Operation = 'initialize' | 'update' | 'refresh';
export interface WatermarkTile { url: string; width: number; height: number }
interface Line { text: string; x: number; y: number }
interface TilePlan {
  width: number; height: number; pixelWidth: number; pixelHeight: number;
  dpr: number; lines: Line[];
}
interface WatermarkRuntime {
  measure(config: OfficeWatermarkConfig, code: OfficeViewerError['code']): TilePlan;
  draw(config: OfficeWatermarkConfig): Promise<WatermarkTile>;
}
interface Hooks {
  commit(tile: WatermarkTile): void;
  hide(): void;
  error(operation: Operation, error: OfficeViewerError): void;
}
interface Task {
  id: number; config: OfficeWatermarkConfig; operation: Operation;
  promise: Promise<void>; resolve(): void; reject(error: OfficeViewerError): void;
}

const failure = (message: string): OfficeViewerError => createOfficeViewerError('WATERMARK_FAILED', message);
const invalid = (): OfficeViewerError => createOfficeViewerError('INVALID_WATERMARK', '水印参数无效或图块超出限制');
const cancelled = (destroyed = false): OfficeViewerError => createOfficeViewerError(
  destroyed ? 'DESTROYED' : 'ABORTED', destroyed ? 'viewer 已销毁' : '水印更新已取消',
);
const snapshot = (config: OfficeWatermarkConfig): OfficeWatermarkConfig => ({
  ...config, content: [...config.content], gap: [...config.gap],
});

export function normalizeWatermark(doc: Document, value: OfficeWatermarkOptions): OfficeWatermarkConfig {
  if (!value || typeof value !== 'object') throw invalid();
  const raw = typeof value.content === 'string' ? [value.content] : value.content;
  if (!Array.isArray(raw) || Array.from(raw).some(line => typeof line !== 'string')) throw invalid();
  const content = raw.flatMap(line => line.split(/\r\n|\r|\n/));
  while (content.length && !content[0].trim()) content.shift();
  while (content.length && !content[content.length - 1].trim()) content.pop();
  const lengths = content.map(line => [...line].length);
  if (!content.length || content.length > 4 || lengths.some(length => length > 128)
    || lengths.reduce((sum, length) => sum + length, 0) > 256) throw invalid();
  const number = (input: number | undefined, fallback: number, min: number, max: number): number => {
    const result = input === undefined ? fallback : input;
    if (typeof result !== 'number' || !Number.isFinite(result) || result < min || result > max) throw invalid();
    return result;
  };
  const fontFamily = value.fontFamily === undefined ? 'sans-serif' : value.fontFamily;
  if (typeof fontFamily !== 'string' || !fontFamily.trim() || fontFamily.length > 256
    || /(?:url|var|env)\s*\(|[;{}\u0000-\u001f]/i.test(fontFamily)
    || /^(?:initial|inherit|unset|revert|revert-layer)$/i.test(fontFamily.trim())) throw invalid();
  const style = doc.createElement('span').style;
  style.fontFamily = fontFamily;
  if (!style.fontFamily) throw invalid();
  let color = value.color === undefined ? '#000000' : value.color;
  if (typeof color !== 'string' || !/^#(?:[\da-f]{3}|[\da-f]{6})$/i.test(color)) throw invalid();
  if (color.length === 4) color = '#' + [...color.slice(1)].map(char => char + char).join('');
  const gap = value.gap === undefined ? [120, 80] : value.gap;
  if (!Array.isArray(gap) || gap.length !== 2 || typeof gap[0] !== 'number' || typeof gap[1] !== 'number') throw invalid();
  return {
    content, fontFamily: style.fontFamily, color: color.toLowerCase(),
    rotate: number(value.rotate, -25, -180, 180),
    opacity: number(value.opacity, 0.12, 0, 1),
    fontSize: number(value.fontSize, 14, 8, 72),
    gap: [number(gap[0], 120, 0, 512), number(gap[1], 80, 0, 512)],
  };
}

function dprOf(doc: Document): number {
  return Math.min(3, Math.max(1, doc.defaultView?.devicePixelRatio || 1));
}

function canvasContext(doc: Document): { canvas: HTMLCanvasElement; ctx: CanvasRenderingContext2D } {
  const canvas = doc.createElement('canvas');
  canvas.width = canvas.height = 1;
  const ctx = canvas.getContext('2d');
  if (!ctx) throw failure('无法创建水印 Canvas 上下文');
  return { canvas, ctx };
}

function measureTile(doc: Document, config: OfficeWatermarkConfig, code: OfficeViewerError['code']): TilePlan {
  const { canvas, ctx } = canvasContext(doc);
  try {
    // CSS escape sequences may spell global keywords too. Canvas rejects an
    // invalid shorthand silently, so use a sentinel before budget validation.
    ctx.font = '7px serif';
    ctx.font = `${config.fontSize}px ${config.fontFamily}`;
    if (ctx.font === '7px serif') throw createOfficeViewerError(code, '水印字体不能用于 Canvas');
    const metrics = config.content.map(text => ctx.measureText(text));
    const lineHeight = config.fontSize * 1.4;
    let width = 0, top = Infinity, bottom = -Infinity;
    const lines = metrics.map((metric, index) => {
      const left = Math.min(0, -(metric.actualBoundingBoxLeft || 0));
      const right = Math.max(metric.width, metric.actualBoundingBoxRight || 0);
      width = Math.max(width, right - left);
      top = Math.min(top, index * lineHeight - (metric.actualBoundingBoxAscent || config.fontSize));
      bottom = Math.max(bottom, index * lineHeight + (metric.actualBoundingBoxDescent || config.fontSize * 0.3));
      return { text: config.content[index], x: -(left + right) / 2, y: index * lineHeight };
    });
    for (const line of lines) line.y -= (top + bottom) / 2;
    const radians = config.rotate * Math.PI / 180;
    const height = bottom - top;
    const tileWidth = Math.ceil(Math.abs(width * Math.cos(radians)) + Math.abs(height * Math.sin(radians)) + 4 + config.gap[0]);
    const tileHeight = Math.ceil(Math.abs(width * Math.sin(radians)) + Math.abs(height * Math.cos(radians)) + 4 + config.gap[1]);
    const dpr = dprOf(doc);
    const pixelWidth = Math.ceil(tileWidth * dpr), pixelHeight = Math.ceil(tileHeight * dpr);
    if (!Number.isFinite(pixelWidth * pixelHeight) || pixelWidth > 4096 || pixelHeight > 4096
      || pixelWidth * pixelHeight > 4_194_304) {
      throw createOfficeViewerError(code, '水印图块超出位图尺寸限制');
    }
    return { width: tileWidth, height: tileHeight, pixelWidth, pixelHeight, dpr, lines };
  } finally { canvas.width = canvas.height = 0; }
}

function browserRuntime(doc: Document): WatermarkRuntime {
  return {
    measure: (config, code) => measureTile(doc, config, code),
    async draw(config) {
      const plan = measureTile(doc, config, 'WATERMARK_FAILED');
      const { canvas, ctx } = canvasContext(doc);
      let url: string;
      try {
        canvas.width = plan.pixelWidth;
        canvas.height = plan.pixelHeight;
        ctx.scale(plan.dpr, plan.dpr);
        ctx.translate(plan.width / 2, plan.height / 2);
        ctx.rotate(config.rotate * Math.PI / 180);
        ctx.font = `${config.fontSize}px ${config.fontFamily}`;
        ctx.fillStyle = config.color;
        ctx.globalAlpha = config.opacity;
        ctx.textBaseline = 'alphabetic';
        for (const line of plan.lines) ctx.fillText(line.text, line.x, line.y);
        url = canvas.toDataURL('image/png');
        if (!url.startsWith('data:image/png')) throw failure('水印图块编码失败');
      } finally { canvas.width = canvas.height = 0; }
      const image = doc.createElement('img');
      try {
        await new Promise<void>((resolve, reject) => {
          image.onerror = () => reject(failure('水印图块加载失败，请检查 img-src data: 策略'));
          if (typeof image.decode !== 'function') image.onload = () => resolve();
          image.src = url;
          if (typeof image.decode === 'function') image.decode().then(resolve, () => reject(failure('水印图块解码失败')));
        });
      } finally {
        image.onload = image.onerror = null;
        image.removeAttribute('src');
      }
      return { url, width: plan.width, height: plan.height };
    },
  };
}

/** Preflight runs before DOM attachment / supersession. A missing Canvas is a rendering failure. */
export function validateWatermark(doc: Document, value: OfficeWatermarkOptions): OfficeWatermarkConfig {
  const config = normalizeWatermark(doc, value);
  try { measureTile(doc, config, 'INVALID_WATERMARK'); }
  catch (error) { if ((error as OfficeViewerError)?.code === 'INVALID_WATERMARK') throw error; }
  return config;
}

/** Internal runtime injection is used for deterministic cancellation/timeout regression tests. */
export function createWatermarkController(
  doc: Document, initial: OfficeWatermarkConfig | null, hooks: Hooks,
  runtime: WatermarkRuntime = browserRuntime(doc), decodeTimeout = 5000,
) {
  let applied: OfficeWatermarkConfig | null = null;
  let tile: WatermarkTile | null = null;
  let initialIntent = initial;
  let initialError: OfficeViewerError | undefined;
  let pending: Task | undefined;
  let sequence = 0, running = false, paused = false, destroyed = false;
  let unlisten: (() => void) | undefined;
  let refreshQueued = false;
  let refreshDirty = false;
  let refreshOnOpen = false;
  let activeTimer: ReturnType<typeof setTimeout> | undefined;
  const win = doc.defaultView;

  const stopListening = (): void => { unlisten?.(); unlisten = undefined; };
  const report = (task: Task, error: unknown): void => {
    if (pending !== task || destroyed) return;
    const wrapped = error instanceof Error && (error as OfficeViewerError).code === 'WATERMARK_FAILED'
      ? error as OfficeViewerError : failure('水印图块生成失败');
    pending = undefined;
    if (task.operation === 'initialize') initialError = wrapped;
    task.reject(wrapped);
    if (!applied) stopListening();
    hooks.error(task.operation, wrapped);
  };
  const cancelPending = (dead = false): void => {
    const task = pending;
    pending = undefined;
    task?.reject(cancelled(dead));
  };

  const listen = (): void => {
    if (unlisten || destroyed || (!applied && !pending)) return;
    let dpr = dprOf(doc);
    let media: MediaQueryList | undefined;
    const fontChange = () => refresh();
    const checkDpr = () => {
      const next = dprOf(doc);
      if (next === dpr) return;
      dpr = next;
      bindMedia();
      refresh();
    };
    const bindMedia = () => {
      media?.removeEventListener('change', checkDpr);
      media = win?.matchMedia?.(`(resolution: ${win.devicePixelRatio || 1}dppx)`);
      media?.addEventListener('change', checkDpr);
    };
    bindMedia();
    win?.addEventListener('resize', checkDpr);
    doc.fonts?.addEventListener('loadingdone', fontChange);
    unlisten = () => {
      media?.removeEventListener('change', checkDpr);
      win?.removeEventListener('resize', checkDpr);
      doc.fonts?.removeEventListener('loadingdone', fontChange);
    };
  };

  const pump = (): void => {
    if (running || paused || destroyed || !pending) return;
    const task = pending;
    running = true;
    // Do not start initialization before callers can subscribe to its error event.
    queueMicrotask(() => {
      if (destroyed || pending !== task) { running = false; pump(); return; }
      let timedOut = false;
      const timer = setTimeout(() => {
        timedOut = true;
        paused = true;
        refreshDirty = false;
        if (pending) report(pending, failure('水印图块解码超时；解码结束前暂停新绘制'));
      }, decodeTimeout);
      activeTimer = timer;
      void Promise.resolve().then(() => runtime.draw(task.config)).then(candidate => {
        if (timedOut || destroyed || pending !== task) return;
        hooks.commit(candidate);
        tile = candidate;
        applied = task.config;
        refreshOnOpen = false;
        initialIntent = null;
        initialError = undefined;
        pending = undefined;
        listen();
        task.resolve();
      }).catch(error => {
        if (!timedOut) report(task, error);
      }).finally(() => {
        clearTimeout(timer);
        if (activeTimer === timer) activeTimer = undefined;
        running = false;
        paused = false;
        pump();
        if (!pending && refreshDirty && applied && unlisten) {
          refreshDirty = false;
          refresh();
        }
      });
    });
  };

  const enqueue = (config: OfficeWatermarkConfig, operation: Operation): Promise<void> => {
    if (paused) {
      const error = failure('上一水印图块仍在解码，请稍后重试');
      if (operation === 'initialize') initialError = error;
      hooks.error(operation, error);
      return Promise.reject(error);
    }
    sequence++;
    cancelPending();
    let resolve!: () => void, reject!: (error: OfficeViewerError) => void;
    const promise = new Promise<void>((yes, no) => { resolve = yes; reject = no; });
    // Initialization and background refresh have no external Promise consumer.
    void promise.catch(() => {});
    pending = { id: sequence, config, operation, promise, resolve, reject };
    listen();
    pump();
    return promise;
  };

  function refresh(): void {
    if (destroyed) return;
    if (pending) { refreshDirty = true; return; }
    if (refreshQueued || !applied) return;
    refreshQueued = true;
    queueMicrotask(() => {
      refreshQueued = false;
      if (destroyed || !applied || pending || !unlisten) return;
      void enqueue(applied, 'refresh').catch(() => {});
    });
  }

  if (initial) void enqueue(initial, 'initialize').catch(() => {});

  return {
    set(value: OfficeWatermarkOptions | null): Promise<void> {
      if (destroyed) return Promise.reject(cancelled(true));
      if (value === null) {
        sequence++;
        cancelPending();
        applied = tile = initialIntent = null;
        initialError = undefined;
        refreshDirty = false;
        refreshOnOpen = false;
        stopListening();
        hooks.hide();
        return Promise.resolve();
      }
      try {
        const config = normalizeWatermark(doc, value);
        // Preserve an earlier valid request when validation fails.
        try { runtime.measure(config, 'INVALID_WATERMARK'); }
        catch (error) { if ((error as OfficeViewerError)?.code === 'INVALID_WATERMARK') throw error; }
        return enqueue(config, 'update');
      } catch (error) { return Promise.reject(error); }
    },
    get(): OfficeWatermarkConfig | null { return applied && snapshot(applied); },
    hasPending(): boolean { return Boolean(pending); },
    async wait(): Promise<void> {
      if (destroyed) throw cancelled(true);
      if (!pending && !applied && initialIntent && !initialError) void enqueue(initialIntent, 'initialize').catch(() => {});
      // Font/DPR listeners are deliberately removed by clear(). Revalidate the
      // cached tile before reopening, including changes that happened while idle.
      if (!pending && applied && refreshOnOpen) {
        refreshOnOpen = false;
        void enqueue(applied, 'refresh').catch(() => {});
      }
      while (pending) {
        const task = pending;
        try { await task.promise; }
        catch (error) {
          if (task.id !== sequence && (error as OfficeViewerError).code === 'ABORTED') continue;
          if (task.operation === 'refresh' && applied && (error as OfficeViewerError).code === 'WATERMARK_FAILED') continue;
          throw error;
        }
        if (destroyed) throw cancelled(true);
      }
      if (initialError) throw initialError;
    },
    mount(): void { if (tile && applied && !destroyed) { hooks.commit(tile); listen(); } },
    clear(): void {
      sequence++;
      cancelPending();
      refreshDirty = false;
      refreshOnOpen = Boolean(applied);
      stopListening();
      hooks.hide();
    },
    destroy(): void {
      if (destroyed) return;
      destroyed = true;
      sequence++;
      cancelPending(true);
      if (activeTimer !== undefined) clearTimeout(activeTimer);
      activeTimer = undefined;
      refreshDirty = false;
      applied = tile = initialIntent = null;
      refreshOnOpen = false;
      initialError = undefined;
      stopListening();
      hooks.hide();
    },
  };
}
