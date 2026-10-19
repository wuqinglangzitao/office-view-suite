import { createOfficeViewer, type OfficeViewerError } from '@office-view-suite/viewer';

const $ = <T extends HTMLElement = HTMLElement>(sel: string): T => {
  const el = document.querySelector<T>(sel);
  if (!el) throw new Error(`missing element: ${sel}`);
  return el;
};

const SAMPLES: { file: string; label: string }[] = [
  { file: 'req.docx', label: '需求内容(悬挂缩进)' },
  { file: 'manual.docx', label: '操作手册(多级编号)' },
  { file: 'styles-word.docx', label: '全样式矩阵' },
  { file: 'rich.docx', label: '富格式 docx' },
  { file: 'text.doc', label: 'Word 97-03' },
];

const drop = $('#drop');
const fileInput = $<HTMLInputElement>('#file');
const status = $('#status');
const statusText = $('#status-text');
const errorPanel = $('#error');
const errorText = $('#error-text');
const viewerHost = $('#viewer');
const meta = $('#meta');
const intake = $('#intake');
const officeViewer = createOfficeViewer(viewerHost);
let currentAbort: AbortController | undefined;
const lifetime = new AbortController();
let disposed = false;
const sampleButtons: HTMLButtonElement[] = [];

function dispose(): void {
  if (disposed) return;
  disposed = true;
  currentAbort?.abort();
  lifetime.abort();
  officeViewer.destroy();
  sampleButtons.forEach(button => button.remove());
}

window.addEventListener('pagehide', event => {
  if (!event.persisted) dispose();
}, { signal: lifetime.signal });
import.meta.hot?.dispose(dispose);
let firstConvert = true;

drop.addEventListener('click', () => fileInput.click(), { signal: lifetime.signal });
fileInput.addEventListener('change', () => {
  const file = fileInput.files?.[0];
  if (file) void openFile(file.name, () => file.arrayBuffer());
  fileInput.value = '';
}, { signal: lifetime.signal });
drop.addEventListener('dragover', event => {
  event.preventDefault();
  drop.classList.add('drag');
}, { signal: lifetime.signal });
drop.addEventListener('dragleave', () => drop.classList.remove('drag'), { signal: lifetime.signal });
drop.addEventListener('drop', event => {
  event.preventDefault();
  drop.classList.remove('drag');
  const file = event.dataTransfer?.files?.[0];
  if (file) void openFile(file.name, () => file.arrayBuffer());
}, { signal: lifetime.signal });

const samplesBar = $('#samples-bar');
for (const sample of SAMPLES) {
  const button = document.createElement('button');
  button.className = 'sample-btn';
  button.textContent = sample.label;
  button.addEventListener('click', () =>
    void openFile(sample.file, async signal => {
      const response = await fetch(`./samples/${sample.file}`, { signal });
      if (!response.ok) throw new Error(`示例文件加载失败 (${response.status})`);
      return response.arrayBuffer();
    }),
    { signal: lifetime.signal },
  );
  sampleButtons.push(button);
  samplesBar.appendChild(button);
}

async function openFile(name: string, loadBytes: (signal: AbortSignal) => Promise<ArrayBuffer>): Promise<void> {
  if (disposed) return;
  currentAbort?.abort();
  const request = new AbortController();
  currentAbort = request;
  hide(errorPanel);
  hide(meta);
  statusText.textContent = firstConvert ? '首次使用:正在加载解析引擎(约 2.6MB)…' : `正在解析「${name}」…`;
  firstConvert = false;
  show(status);
  try {
    const buffer = await loadBytes(request.signal);
    if (request.signal.aborted || disposed || currentAbort !== request) return;
    // Layout-based renderers must measure a visible host on the first open.
    show(viewerHost);
    const result = await officeViewer.open({ bytes: buffer, name, signal: request.signal });
    if (request.signal.aborted || disposed || currentAbort !== request) return;
    hide(status);
    hide(intake);
    show(viewerHost);
    meta.textContent = '';
    meta.append(
      chip(`⏱ ${result.durationMs < 1 ? '<1' : result.durationMs.toFixed(0)} ms`),
      chip(`¶ ${result.stats.paragraphs ?? 0}`),
      chip(`⧉ ${result.stats.tables ?? 0} 表`),
      chip(`🖼 ${result.stats.images ?? 0} 图`),
    );
    show(meta);
  } catch (error) {
    if (request.signal.aborted || disposed || currentAbort !== request) return;
    const viewerError = error as Partial<OfficeViewerError>;
    if (viewerError.code === 'ABORTED') return;
    showError(error instanceof Error ? error.message : String(error));
  }
}

function chip(text: string): HTMLElement {
  const element = document.createElement('span');
  element.className = 'chip';
  element.textContent = text;
  return element;
}

function showError(message: string): void {
  hide(status);
  show(intake);
  errorText.textContent = `解析失败:${message}`;
  show(errorPanel);
}

function show(element: HTMLElement): void {
  element.classList.remove('hidden');
}
function hide(element: HTMLElement): void {
  element.classList.add('hidden');
}
