import { renderWordInto } from '../core/render';
import type { WordResponse } from '../worker/worker';

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

const worker = new Worker(new URL('../worker/worker.ts', import.meta.url), { type: 'module' });
let nextId = 1;

const drop = $('#drop');
const fileInput = $<HTMLInputElement>('#file');
const status = $('#status');
const statusText = $('#status-text');
const errorPanel = $('#error');
const errorText = $('#error-text');
const viewer = $('#viewer');
const meta = $('#meta');
const intake = $('#intake');

let firstConvert = true;

worker.onmessage = (ev: MessageEvent<WordResponse>) => {
  const res = ev.data;
  if (res.id !== currentId) return;
  if (!res.ok) {
    showError(`解析失败:${res.message}`);
    return;
  }
  viewer.textContent = '';
  // 分页依赖真实布局尺寸，容器必须在首次同步测量前可见。
  show(viewer);
  const result = renderWordInto(viewer, res.document);
  hide(status);
  hide(intake);
  meta.textContent = '';
  const chip = (t: string) => {
    const el = document.createElement('span');
    el.className = 'chip';
    el.textContent = t;
    return el;
  };
  meta.append(
    chip(`⏱ ${res.durationMs < 1 ? '<1' : res.durationMs.toFixed(0)} ms`),
    chip(`¶ ${result.stats.paragraphs}`),
    chip(`⧉ ${result.stats.tables} 表`),
    chip(`🖼 ${result.stats.images} 图`),
  );
  show(meta);
};

let currentId = 0;

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

const samplesBar = $('#samples-bar');
for (const sample of SAMPLES) {
  const btn = document.createElement('button');
  btn.className = 'sample-btn';
  btn.textContent = sample.label;
  btn.addEventListener('click', () =>
    void openFile(sample.file, fetch(`./samples/${sample.file}`).then(r => r.arrayBuffer())),
  );
  samplesBar.appendChild(btn);
}

async function openFile(name: string, bytes: Promise<ArrayBuffer>): Promise<void> {
  hide(errorPanel);
  hide(meta);
  statusText.textContent = firstConvert
    ? `首次使用:正在加载解析引擎(约 2.6MB)…`
    : `正在解析「${name}」…`;
  firstConvert = false;
  show(status);
  let buffer: ArrayBuffer;
  try {
    buffer = await bytes;
  } catch {
    showError('文件读取失败');
    return;
  }
  currentId = nextId++;
  worker.postMessage({ id: currentId, name, bytes: buffer }, [buffer]);
}

function showError(message: string): void {
  hide(status);
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
