import { renderPptInto } from '../core/render';
import type { PptResponse } from '../worker/worker';

const $ = <T extends HTMLElement = HTMLElement>(sel: string): T => {
  const el = document.querySelector<T>(sel);
  if (!el) throw new Error(`missing element: ${sel}`);
  return el;
};

const SAMPLES: { file: string; label: string }[] = [
  { file: 'arch.pptx', label: '架构评审(真实文档)' },
  { file: 'styles-ppt.pptx', label: '全样式矩阵' },
  { file: 'pres.pptx', label: '演示文稿(表格+层级)' },
  { file: 'handmade-math.pptx', label: '公式(降级展示)' },
];

const worker = new Worker(new URL('../worker/worker.ts', import.meta.url), { type: 'module' });
let nextId = 1;
let currentId = 0;

const drop = $('#drop');
const fileInput = $<HTMLInputElement>('#file');
const status = $('#status');
const statusText = $('#status-text');
const errorPanel = $('#error');
const errorText = $('#error-text');
const viewer = $('#viewer');
const meta = $('#meta');
const intake = $('#intake');
const warn = $('#warn');

let firstConvert = true;

worker.onmessage = (ev: MessageEvent<PptResponse>) => {
  const res = ev.data;
  if (res.id !== currentId) return;
  if (!res.ok) {
    showError(`解析失败:${res.message}`);
    return;
  }
  viewer.textContent = '';
  const result = renderPptInto(viewer, res.document);
  hide(status);
  hide(intake);
  show(viewer);
  warn.textContent = (res.document.warnings ?? []).join('\n');
  meta.textContent = '';
  const chip = (t: string) => {
    const el = document.createElement('span');
    el.className = 'chip';
    el.textContent = t;
    return el;
  };
  meta.append(
    chip(`⏱ ${res.durationMs < 1 ? '<1' : res.durationMs.toFixed(0)} ms`),
    chip(` ▶ ${result.stats.slides} 页`),
    ...(result.stats.hiddenSlides ? [chip(`◌ ${result.stats.hiddenSlides} 隐藏页`)] : []),
    chip(`◧ ${result.stats.shapes} 形状`),
    ...(result.stats.tables ? [chip(`⧉ ${result.stats.tables} 表`)] : []),
  );
  show(meta);
};

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
  warn.textContent = '';
  statusText.textContent = firstConvert
    ? '首次使用:正在加载解析引擎(约 420KB)…'
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
