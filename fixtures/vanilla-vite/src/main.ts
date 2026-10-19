import { createOfficeViewer, type OfficeViewerOptions } from '@office-view-suite/viewer';

const input = document.querySelector<HTMLInputElement>('#file');
const host = document.querySelector<HTMLElement>('#viewer');
if (!input || !host) throw new Error('fixture DOM 初始化失败');

const viewer = createOfficeViewer(host);
const secondHost = document.querySelector<HTMLElement>('#second-viewer')!;
const second = createOfficeViewer(secondHost);
const status = document.querySelector<HTMLOutputElement>('#status')!;
input.addEventListener('change', async () => {
  const file = input.files?.[0];
  if (!file) return;
  try {
    status.textContent = '正在打开…';
    const result = await viewer.open({ bytes: file, name: file.name });
    status.textContent = JSON.stringify(result);
  } catch (error) { status.textContent = String(error); }
});

const samples = { word: 'styles-word.docx', excel: 'styles-excel.xlsx', ppt: 'styles-ppt.pptx', pdf: 'handmade-mixed.pdf' };
async function sample(kind: keyof typeof samples) {
  const name = samples[kind];
  const response = await fetch(`${import.meta.env.BASE_URL}samples/${name}`);
  if (!response.ok) throw new Error(`Sample request failed: ${response.status}`);
  return { bytes: await response.arrayBuffer(), name };
}
for (const button of document.querySelectorAll<HTMLButtonElement>('[data-sample]')) {
  button.onclick = async () => {
    try {
      status.textContent = '正在打开…';
      status.textContent = JSON.stringify(await viewer.open(await sample(button.dataset.sample as keyof typeof samples)));
    } catch (error) { status.textContent = String(error); }
  };
}
document.querySelector<HTMLButtonElement>('#clear')!.onclick = () => viewer.clear();

// This fixture-only surface lets the packed-consumer smoke test exercise the public API.
const fixture = { viewer, second, sample, create: (element: HTMLElement, options?: OfficeViewerOptions) => createOfficeViewer(element, options) };
declare global { interface Window { officeFixture: typeof fixture } }
window.officeFixture = fixture;
window.addEventListener('pagehide', () => { viewer.destroy(); second.destroy(); });
if (import.meta.hot) import.meta.hot.dispose(() => { viewer.destroy(); second.destroy(); });
