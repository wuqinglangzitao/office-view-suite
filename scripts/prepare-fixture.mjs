import { cp, mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
const samples = {word:'styles-word.docx',excel:'styles-excel.xlsx',ppt:'styles-ppt.pptx',pdf:'handmade-mixed.pdf'};
await mkdir('public/samples', { recursive: true });
for (const [format, file] of Object.entries(samples)) {
  await cp(resolve(`../../apps/${format}-view/public/samples/${file}`), resolve('public/samples', file));
}
