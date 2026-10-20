import { cp, mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';
import './prepare-fixture.mjs';

await mkdir('public/office-assets', { recursive: true });
await mkdir('public/office-licenses', { recursive: true });
await cp(resolve('../../LICENSE'), resolve('public/office-licenses/LICENSE'));
for (const format of ['word', 'excel', 'ppt', 'pdf']) {
  const dist = resolve(`../../packages/viewer-${format}/dist`);
  await cp(resolve(dist, 'assets'), resolve('public/office-assets'), { recursive: true });
  const licenses = resolve(`public/office-licenses/viewer-${format}`);
  await mkdir(licenses, { recursive: true });
  await cp(resolve(dist, 'THIRD_PARTY_NOTICES.md'), resolve(licenses, 'THIRD_PARTY_NOTICES.md'));
  await cp(resolve(dist, 'licenses'), resolve(licenses, 'licenses'), { recursive: true });
}
