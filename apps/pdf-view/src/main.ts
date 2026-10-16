interface PageSize {
  wPt: number;
  hPt: number;
}

interface MetaMsg {
  type: 'meta';
  id: number;
  pages: PageSize[];
}
interface PageMsg {
  type: 'page';
  id: number;
  page: number;
  image: ImageData;
  ms: number;
}
interface ErrorMsg {
  type: 'error';
  id: number;
  message: string;
}
type WorkerMsg = MetaMsg | PageMsg | ErrorMsg;

const $ = <T extends HTMLElement = HTMLElement>(sel: string): T => {
  const el = document.querySelector<T>(sel);
  if (!el) throw new Error(`missing element: ${sel}`);
  return el;
};

const worker = new Worker('/worker/pdf-worker.js'); // classic worker
let nextId = 1;
let currentId = 0;
let totalRenderMs = 0;

const drop = $('#drop');
const fileInput = $<HTMLInputElement>('#file');
const statusBox = $('#status');
const statusText = $('#status-text');
const errorPanel = $('#error');
const errorText = $('#error-text');
const pagesEl = $('#pages');
const meta = $('#meta');
const intake = $('#intake');

const pendingRenders = new Map<number, (m: PageMsg) => void>();
let pendingMeta: ((m: MetaMsg) => void) | null = null;
let pendingError: ((m: ErrorMsg) => void) | null = null;

worker.onmessage = (ev: MessageEvent<WorkerMsg>) => {
  const msg = ev.data;
  // page 响应携带独立渲染 id,先于会话守卫处理
  if (msg.type === 'page') {
    const resolve = pendingRenders.get(msg.id);
    if (resolve) {
      pendingRenders.delete(msg.id);
      resolve(msg);
    }
    return;
  }
  if (msg.id !== currentId) return;
  if (msg.type === 'meta' && pendingMeta) {
    pendingMeta(msg);
    pendingMeta = null;
  } else if (msg.type === 'error' && pendingError) {
    pendingError(msg);
    pendingError = null;
  }
};

const SAMPLES = [
  { file: 'text.pdf', label: '文本文档 PDF' },
  { file: 'handmade-mixed.pdf', label: '混合内容 PDF' },
];

const sampleBar = $('#samples-bar');
for (const sample of SAMPLES) {
  const btn = document.createElement('button');
  btn.className = 'sample-btn';
  btn.textContent = sample.label;
  btn.addEventListener('click', () =>
    void openFile(sample.file, fetch(`./samples/${sample.file}`).then(r => r.arrayBuffer())),
  );
  sampleBar.appendChild(btn);
}

drop.addEventListener('click', () => fileInput.click());
fileInput.addEventListener('change', () => {
  const file = fileInput.files?.[0];
  if (file) void openFile(file.name, file.arrayBuffer());
  fileInput.value = '';
});
drop.addEventListener('dragover', e => {
  e.preventDefault();
  drop.classList.add('drag');
});
drop.addEventListener('dragleave', () => drop.classList.remove('drag'));
drop.addEventListener('drop', e => {
  e.preventDefault();
  drop.classList.remove('drag');
  const file = e.dataTransfer?.files?.[0];
  if (file) void openFile(file.name, file.arrayBuffer());
});

async function openFile(name: string, bytes: Promise<ArrayBuffer>): Promise<void> {
  hide(errorPanel);
  hide(meta);
  if (!/\.pdf$/i.test(name)) {
    showError('pdf-view 仅支持 .pdf');
    return;
  }
  statusText.textContent = `正在解析「${name}」(首次需加载 pdfium ≈5MB + 胶水 1.8MB)…`;
  show(statusBox);
  let buffer: ArrayBuffer;
  try {
    buffer = await bytes;
  } catch {
    showError('文件读取失败');
    return;
  }

  const id = nextId++;
  currentId = id;
  const metaPromise = new Promise<MetaMsg>(resolve => {
    pendingMeta = resolve;
  });
  const errPromise = new Promise<ErrorMsg>(resolve => {
    pendingError = resolve;
  });
  worker.postMessage({ type: 'open', id, bytes: buffer }, [buffer]);

  const msg = await Promise.race([metaPromise, errPromise]);
  if (msg.type === 'error') {
    showError(`解析失败:${msg.message}`);
    return;
  }

  pagesEl.textContent = '';
  totalRenderMs = 0;
  const containerWidth = Math.min(pagesEl.clientWidth - 24, 900);

  const observer = new IntersectionObserver(async entries => {
    for (const entry of entries) {
      if (!entry.isIntersecting) continue;
      const canvas = entry.target as HTMLCanvasElement;
      observer.unobserve(canvas);
      const page = Number(canvas.dataset.page);
      const rid = nextId++;
      const renderPromise = new Promise<PageMsg>(resolve => {
        pendingRenders.set(rid, resolve);
      });
      worker.postMessage({ type: 'render', id: rid, page, width: Math.round(containerWidth * devicePixelRatio) });
      const rendered = await renderPromise;
      totalRenderMs += rendered.ms;
      canvas.width = rendered.image.width;
      canvas.height = rendered.image.height;
      canvas.style.height = `${rendered.image.height / devicePixelRatio}px`;
      canvas.getContext('2d')?.putImageData(rendered.image, 0, 0);
    }
  }, { rootMargin: '300px' });

  for (let n = 0; n < msg.pages.length; n++) {
    const size = msg.pages[n];
    const canvas = document.createElement('canvas');
    canvas.className = 'page';
    canvas.dataset.page = String(n);
    canvas.style.height = `${(size.hPt / size.wPt) * containerWidth}px`;
    pagesEl.appendChild(canvas);
    observer.observe(canvas);
  }

  hide(statusBox);
  hide(intake);
  const chip = (t: string) => {
    const el = document.createElement('span');
    el.className = 'chip';
    el.textContent = t;
    return el;
  };
  meta.textContent = '';
  meta.append(
    chip(`📄 ${msg.pages.length} 页`),
    chip('PDFium(Chromium 引擎)视觉级'),
    chip('Worker 渲染 + 懒加载'),
  );
  show(meta);
}

function showError(message: string): void {
  hide(statusBox);
  show(intake);
  errorText.textContent = message;
  show(errorPanel);
}

function show(el: HTMLElement): void {
  el.classList.remove('hidden');
}
function hide(el: HTMLElement): void {
  el.classList.add('hidden');
}
