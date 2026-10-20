import assert from 'node:assert/strict';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = resolve(root, 'output/playwright/ppt-display');
const { build } = await import(pathToFileURL(resolve(root, 'packages/viewer/node_modules/vite/dist/node/index.js')).href);
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const built = await build({ configFile: false, logLevel: 'silent', build: {
  write: false, minify: false,
  lib: { entry: resolve(root, 'packages/viewer-ppt/src/core/render.ts'), name: 'pptDisplay', formats: ['iife'] },
} });
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const page = await browser.newPage({ viewport: { width: 1000, height: 900 }, deviceScaleFactor: 1 });
const results = [], errors = [];
page.on('pageerror', error => errors.push(error.message));
const run = (text, properties = {}) => ({ text, bold: false, italic: false, underline: false, sizePt: 18, ...properties });
const para = (runs = [run('Text')], properties = {}) => ({ level: 0, runs, ...properties });
const shape = (properties = {}) => ({
  kind: 'shape', name: 'shape', x: 100000, y: 100000, w: 2000000, h: 1000000,
  rotation: 0, flipH: false, flipV: false, shapeType: 'rect', fill: '#ffffff',
  adjustments: [], customPaths: [], paragraphs: [para()], marginLeftEmu: 0,
  marginRightEmu: 0, marginTopEmu: 0, marginBottomEmu: 0,
  wrapText: true, fontScale: 1, lineSpacingReduction: 0, ...properties,
});
const cell = (properties = {}) => ({ text: 'Text', colSpan: 1, rowSpan: 1,
  marginLeftEmu: 0, marginRightEmu: 0, marginTopEmu: 0, marginBottomEmu: 0,
  ...properties });
const table = (properties = {}) => ({ x: 100000, y: 100000, w: 2000000, h: 1000000,
  columns: [2000000], rows: [{ heightEmu: 1000000, cells: [cell()] }], ...properties });
const doc = (shapes = [], tables = []) => ({ slideWidthEmu: 9144000, slideHeightEmu: 6858000,
  slides: [{ hidden: false, shapes, tables }], warnings: [] });
const render = document => page.evaluate(document => {
  window.rendered?.dispose();
  window.rendered = window.pptDisplay.renderPptInto(window.host, document);
}, document);
const check = async (name, fn) => {
  try { const details = await fn(); results.push({ name, passed: true, ...details }); console.log(`PASS ${name}`); }
  catch (error) { results.push({ name, passed: false, error: error.message }); console.error(`FAIL ${name}: ${error.message}`); }
};

try {
  await page.setContent('<!doctype html><meta charset="utf-8"><style>body{margin:0}#host{width:960px}</style><div id="host"></div>');
  await page.addScriptTag({ content: built[0].output.find(item => item.type === 'chunk').code });
  await check('physical-points-follow-source-slide-width', async () => {
    await render(doc([shape({ paragraphs: [para([run('18pt')], { lineSpacingPt: 24, spaceAfterPt: 6 })] })]));
    const metrics = await page.evaluate(() => {
      const p = host.shadowRoot.querySelector('.shape p'), span = p.querySelector('span');
      return { font: parseFloat(getComputedStyle(span).fontSize), line: parseFloat(getComputedStyle(p).lineHeight), after: parseFloat(getComputedStyle(p).marginBottom) };
    });
    assert.equal(metrics.font, 24); assert.equal(metrics.line, 32); assert.equal(metrics.after, 8);
    return metrics;
  });
  await check('transparent-gradients-survive-svg-and-text', async () => {
    const gradient = 'linear-gradient(90deg,rgba(255,0,0,0.5) 0%,rgba(0,0,255,0.7) 100%)';
    await render(doc([shape({ fill: gradient, paragraphs: [para([run('Gradient', { color: gradient })])] }),
      shape({ kind: 'line', y: 2000000, h: 0, stroke: gradient, headEnd: 'triangle', tailEnd: 'triangle', paragraphs: [] })]));
    const paint = await page.evaluate(() => {
      const root = host.shadowRoot, fill = root.querySelector('.shape-geometry path'), line = root.querySelector('.connector > path'), span = root.querySelector('.shape p span');
      return { fill: fill.getAttribute('fill'), line: line.getAttribute('stroke'), stops: root.querySelectorAll('stop').length,
        background: getComputedStyle(span).backgroundImage, clip: getComputedStyle(span).backgroundClip, color: getComputedStyle(span).color,
        markerFills: [...root.querySelectorAll('marker path')].map(path => getComputedStyle(path).fill) };
    });
    assert.match(paint.fill, /^url\(#/); assert.match(paint.line, /^url\(#/); assert.equal(paint.stops, 4);
    assert.match(paint.background, /linear-gradient/); assert.equal(paint.clip, 'text'); assert.equal(paint.color, 'rgba(0, 0, 0, 0)');
    assert.deepEqual(paint.markerFills, ['rgba(255, 0, 0, 0.5)', 'rgba(0, 0, 255, 0.7)']);
  });
  await check('table-retains-rich-runs-bullets-and-source-borders', async () => {
    await render(doc([], [table({ rows: [{ heightEmu: 1000000, cells: [cell({ paragraphs: [para([run('Bold', { bold: true }), run('Italic', { italic: true, color: '#ff0000' })], { bullet: '•' })],
      verticalAlign: 'bottom', borders: { bottom: { color: '#336699', widthEmu: 25400, dash: 'dot' } } })] }] })]));
    const styles = await page.evaluate(() => {
      const td = host.shadowRoot.querySelector('td'), s = td && getComputedStyle(td);
      return { exists: !!td, bold: td?.querySelector('p span:not(.shape-bullet)')?.style.fontWeight,
        italic: td?.querySelector('p span:last-child')?.style.fontStyle, bullet: td?.querySelector('.shape-bullet')?.textContent.trim(),
        top: s?.borderTopStyle, bottom: s?.borderBottomStyle, bottomWidth: parseFloat(s?.borderBottomWidth), valign: s?.verticalAlign, background: s?.backgroundColor };
    });
    assert.equal(styles.exists, true); assert.equal(styles.bold, '700'); assert.equal(styles.italic, 'italic'); assert.equal(styles.bullet, '•');
    assert.equal(styles.top, 'none'); assert.equal(styles.bottom, 'dotted'); assert.ok(Math.abs(styles.bottomWidth - 2.66667) < 1); assert.equal(styles.valign, 'bottom');
    assert.equal(styles.background, 'rgba(0, 0, 0, 0)');
  });
  await check('paint-order-interleaves-shapes-and-tables', async () => {
    await render(doc([shape({ zOrder: 2 }), shape({ zOrder: 0 })], [table({ zOrder: 1 })]));
    const order = await page.evaluate(() => [...host.shadowRoot.querySelector('.slide').children].map(node => node.tagName));
    assert.deepEqual(order, ['DIV', 'TABLE', 'DIV']);
  });
  await check('affine-transform-retains-group-rotation-and-table-mirroring', async () => {
    await render(doc([shape({ transform: [0, 1, -1, 0, 3000000, 2000000] })], [table({ transform: [-1, 0, 0, 1, 5000000, 2000000] })]));
    const transforms = await page.evaluate(() => [...host.shadowRoot.querySelector('.slide').children].map(node => {
      const m = new DOMMatrix(getComputedStyle(node).transform); return [m.a, m.b, m.c, m.d, m.e, m.f];
    }));
    assert.deepEqual(transforms[0].slice(0, 4), [0, 1, -1, 0]);
    assert.deepEqual(transforms[1].slice(0, 4), [-1, 0, 0, 1]);
    assert.ok(Math.abs(transforms[0][4] - 3000000 * 960 / 9144000) < 0.01);
  });
  await check('group-coordinates-scale-geometry-without-scaling-physical-fonts', async () => {
    const unit = 9144000 / 960;
    await render(doc([shape({ w: 200 * unit, h: 100 * unit, localWidthEmu: 100 * unit, localHeightEmu: 50 * unit,
      transform: [2, 0, 0, 2, 0, 0] }),
    shape({ w: 100 * unit, h: 50 * unit, localWidthEmu: 100 * unit, localHeightEmu: 50 * unit,
      transform: [1, 0, 0, 1, 0, 200 * unit] })]));
    const sizes = await page.evaluate(() => [...host.shadowRoot.querySelectorAll('.slide > .shape')].map(node => ({
      width: node.getBoundingClientRect().width, textHeight: node.querySelector('p span').getBoundingClientRect().height,
    })));
    assert.equal(sizes[0].width, 200); assert.equal(sizes[1].width, 100);
    assert.equal(sizes[0].textHeight, sizes[1].textHeight);
  });
  await check('picture-crop-and-ellipse-clip', async () => {
    const image = 'data:image/svg+xml,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect width="100" height="100" fill="red"/></svg>');
    await render(doc([shape({ kind: 'picture', shapeType: 'ellipse', image, crop: { left: 0.25, right: 0.25, top: 0, bottom: 0 }, paragraphs: [] })]));
    const actual = await page.evaluate(() => {
      const root = host.shadowRoot, image = root.querySelector('.picture'), clip = image.closest('.picture-clip');
      return { width: image.style.width, left: image.style.left, clip: clip?.style.clipPath || '', overflow: clip && getComputedStyle(clip).overflow };
    });
    assert.equal(actual.width, '200%'); assert.equal(actual.left, '-50%'); assert.match(actual.clip, /ellipse/); assert.equal(actual.overflow, 'hidden');
  });
  await check('vertical-text-follows-source-direction', async () => {
    await render(doc([shape({ verticalText: 'eaVert', w: 571500, h: 1905000, paragraphs: [para([run('竖排文字')])] })]));
    const actual = await page.evaluate(() => {
      const text = host.shadowRoot.querySelector('.shape-text'), node = text.querySelector('p span').firstChild;
      const range = document.createRange();
      range.setStart(node, 0); range.setEnd(node, 1); const first = range.getBoundingClientRect();
      range.setStart(node, 3); range.setEnd(node, 4); const last = range.getBoundingClientRect();
      return { mode: getComputedStyle(text).writingMode, first: { x: first.x, y: first.y }, last: { x: last.x, y: last.y } };
    });
    assert.equal(actual.mode, 'vertical-rl'); assert.ok(actual.last.y > actual.first.y + 20); assert.ok(Math.abs(actual.first.x - actual.last.x) < 1);
  });
  await check('presets-and-adjustments-change-source-geometry', async () => {
    await render(doc([shape({ shapeType: 'rect' }), shape({ shapeType: 'star5' }), shape({ shapeType: 'rightArrow', adjustments: [20000, 15000] }), shape({ shapeType: 'rightArrow', adjustments: [60000, 40000] })]));
    const paths = await page.evaluate(() => [...host.shadowRoot.querySelectorAll('.shape-geometry path')].map(path => path.getAttribute('d')));
    assert.notEqual(paths[0], paths[1]); assert.notEqual(paths[2], paths[3]);
  });
  await check('dash-variants-remain-distinct', async () => {
    await render(doc([shape({ stroke: '#000', dash: 'dot' }), shape({ stroke: '#000', dash: 'dashDot' }), shape({ stroke: '#000', dash: 'lgDash' })]));
    const values = await page.evaluate(() => [...host.shadowRoot.querySelectorAll('.shape-geometry path')].map(path => getComputedStyle(path).strokeDasharray));
    assert.equal(new Set(values).size, 3);
    assert.ok(values.every(value => value !== 'none' && !value.includes('cqw')));
  });
  await check('two-corner-rounding-uses-both-adjustment-groups', async () => {
    await render(doc([shape({ shapeType: 'round2DiagRect', adjustments: [20000, 0] }),
      shape({ shapeType: 'round2DiagRect', adjustments: [20000, 10000] }),
      shape({ shapeType: 'round2SameRect', adjustments: [20000, 0] }),
      shape({ shapeType: 'round2SameRect', adjustments: [20000, 10000] })]));
    const paths = await page.evaluate(() => [...host.shadowRoot.querySelectorAll('.shape-geometry path')].map(path => path.getAttribute('d')));
    assert.equal((paths[0].match(/ A /g) ?? []).length, 2);
    assert.equal((paths[1].match(/ A /g) ?? []).length, 4);
    assert.equal((paths[2].match(/ A /g) ?? []).length, 2);
    assert.equal((paths[3].match(/ A /g) ?? []).length, 4);
    assert.notEqual(paths[0], paths[2]);
  });
  await check('axis-aligned-connectors-and-arrowheads-paint-pixels', async () => {
    const unit = 9144000 / 960;
    await render(doc([shape({ kind: 'line', w: 200 * unit, h: 0,
      transform: [1, 0, 0, 1, 100 * unit, 100 * unit], stroke: '#000000', strokeWidthEmu: 3 * unit,
      headEnd: 'triangle', tailEnd: 'triangle', paragraphs: [] }),
    shape({ kind: 'line', w: 0, h: 200 * unit,
      transform: [1, 0, 0, 1, 400 * unit, 100 * unit], stroke: '#000000', strokeWidthEmu: 3 * unit,
      headEnd: 'triangle', tailEnd: 'triangle', paragraphs: [] })]));
    const png = await page.locator('#host').screenshot();
    const pixels = await page.evaluate(async source => {
      const image = new Image(); image.src = source; await image.decode();
      const canvas = document.createElement('canvas'); canvas.width = image.width; canvas.height = image.height;
      const context = canvas.getContext('2d'); context.drawImage(image, 0, 0);
      return [[200, 100], [111, 95], [289, 95], [400, 200], [395, 111]].map(([x, y]) => [...context.getImageData(x, y, 1, 1).data]);
    }, `data:image/png;base64,${png.toString('base64')}`);
    assert.ok(pixels.every(pixel => pixel.slice(0, 3).every(channel => channel < 50)), JSON.stringify(pixels));
  });
  await mkdir(output, { recursive: true });
  await check('source-arch-group-units-preserve-physical-lines-and-text', async () => {
    const wasm = resolve(root, 'packages/viewer-ppt/src/wasm');
    const engine = await import(pathToFileURL(resolve(wasm, 'ppt_view_rs.js')).href);
    engine.initSync({ module: await readFile(resolve(wasm, 'ppt_view_rs_bg.wasm')) });
    const source = engine.toDocument(await readFile(resolve(root, 'apps/ppt-view/public/samples/arch.pptx')));
    const measurements = [];
    for (const slideNumber of [29, 33]) {
      await render({ ...source, slides: [source.slides[slideNumber - 1]] });
      await page.evaluate(async () => {
        await document.fonts.ready;
        await Promise.all([...host.shadowRoot.querySelectorAll('img')].map(image => image.decode().catch(() => {})));
        await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
      });
      const png = await page.locator('#host .slide').screenshot({ path: resolve(output, `arch-${String(slideNumber).padStart(3, '0')}-group-units.png`) });
      const pixels = await page.evaluate(async source => {
        const image = new Image(); image.src = source; await image.decode();
        const canvas = document.createElement('canvas'); canvas.width = image.width; canvas.height = image.height;
        const context = canvas.getContext('2d'); context.drawImage(image, 0, 0);
        const data = context.getImageData(0, 0, canvas.width, canvas.height).data;
        const counts = [[98, 240, 199], [237, 113, 0], [84, 91, 100]].map(color => {
          let count = 0;
          for (let i = 0; i < data.length; i += 4) if (color.every((value, channel) => Math.abs(value - data[i + channel]) < 4)) count++;
          return count / (canvas.width * canvas.height);
        });
        const corners = [[10, 10], [canvas.width - 10, canvas.height - 10]].map(([x, y]) => [...context.getImageData(x, y, 1, 1).data]);
        return { counts, corners };
      }, `data:image/png;base64,${png.toString('base64')}`);
      // These source colors belong to thin rules/borders. Group child-coordinate
      // normalization must never turn them into page-sized opaque surfaces.
      assert.ok(pixels.counts.every(fraction => fraction < 0.02), `slide ${slideNumber}: ${JSON.stringify(pixels)}`);
      assert.ok(pixels.corners.every(pixel => pixel.slice(0, 3).every(channel => channel > 245)), `slide ${slideNumber} blank margins were covered`);
      const text = await page.evaluate(() => [...host.shadowRoot.querySelectorAll('.shape-text span')]
        .filter(span => ['远端物联网', '开发', 'MySQL', 'Code'].includes(span.textContent.trim()))
        .map(span => ({ text: span.textContent, height: span.getBoundingClientRect().height })));
      assert.ok(text.length > 0);
      assert.ok(text.every(span => span.height > 5 && span.height < 30), JSON.stringify(text));
      measurements.push({ slideNumber, ...pixels, text });
    }
    return { measurements };
  });
  const gallery = doc([
    shape({ x: 285750, y: 285750, w: 2286000, h: 1143000, fill: 'linear-gradient(90deg,rgba(220,40,70,0.8) 0%,rgba(40,80,220,0.8) 100%)',
      paragraphs: [para([run('Gradient', { color: '#ffffff', bold: true })], { align: 'center' })], verticalAlign: 'center' }),
    shape({ x: 3143250, y: 285750, w: 2286000, h: 1143000, shapeType: 'rightArrow', adjustments: [35000, 45000], fill: '#3894bb', paragraphs: [] }),
    shape({ x: 6286500, y: 285750, w: 1714500, h: 1714500, shapeType: 'star5', fill: '#e8b74a', paragraphs: [] }),
    shape({ kind: 'line', x: 285750, y: 2095500, w: 2286000, h: 0, strokeWidthEmu: 38100,
      transform: [1, 0, 0, 1, 285750, 2095500], headEnd: 'triangle', tailEnd: 'triangle',
      stroke: 'linear-gradient(90deg,#dc2846 0%,#2850dc 100%)', paragraphs: [] }),
    shape({ x: 3143250, y: 2095500, w: 2286000, h: 1143000, shapeType: 'round2DiagRect', adjustments: [25000, 0],
      fill: '#e8f1f5', stroke: '#3894bb', dash: 'dashDot', paragraphs: [para([run('Two corners')], { align: 'center' })], verticalAlign: 'center' }),
    shape({ kind: 'picture', x: 6286500, y: 3048000, w: 1714500, h: 1714500, shapeType: 'ellipse', crop: { left: 0.25, right: 0.25, top: 0, bottom: 0 },
      image: 'data:image/svg+xml,' + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect width="50" height="100" fill="#e8b74a"/><rect x="50" width="50" height="100" fill="#3894bb"/></svg>'), paragraphs: [] }),
  ], [table({ x: 285750, y: 3619500, w: 5143500, h: 1714500, columns: [2571750, 2571750], rows: [
    { heightEmu: 857250, cells: [cell({ isHeader: true, fill: '#17395c', paragraphs: [para([run('Source header', { color: '#fff', bold: true })])] }),
      cell({ fill: '#e8f1f5', paragraphs: [para([run('Rich ', { bold: true }), run('text', { italic: true, color: '#bb3366' })])] })] },
    { heightEmu: 857250, cells: [cell({ paragraphs: [para([run('List item')], { bullet: '•' })] }),
      cell({ verticalAlign: 'bottom', paragraphs: [para([run('Bottom border')])], borders: { bottom: { color: '#3894bb', widthEmu: 25400, dash: 'dot' } } })] },
  ] })]);
  await render(gallery);
  await page.evaluate(async () => { await document.fonts.ready; await Promise.all([...host.shadowRoot.querySelectorAll('img')].map(image => image.decode().catch(() => {}))); });
  await page.locator('#host').screenshot({ path: resolve(output, 'display.png') });
  if (process.env.PPT_DISPLAY_PPTX) await check('external-source-parses-and-renders', async () => {
    const wasm = resolve(root, 'packages/viewer-ppt/src/wasm');
    const engine = await import(pathToFileURL(resolve(wasm, 'ppt_view_rs.js')).href);
    engine.initSync({ module: await readFile(resolve(wasm, 'ppt_view_rs_bg.wasm')) });
    const document = engine.toDocument(await readFile(resolve(process.env.PPT_DISPLAY_PPTX)));
    assert.ok(document.slides.length > 0);
    await render(document);
    const layout = await page.evaluate(() => {
      const slides = [...host.shadowRoot.querySelectorAll('.slide')];
      return { slides: slides.length, invalid: [...host.shadowRoot.querySelectorAll('[style]')].filter(node => /NaN|Infinity/.test(node.getAttribute('style'))).length,
        shapes: host.shadowRoot.querySelectorAll('.slide > .shape').length, tables: host.shadowRoot.querySelectorAll('table').length };
    });
    assert.equal(layout.slides, document.slides.length); assert.equal(layout.invalid, 0);
    await page.locator('#host .slide').first().screenshot({ path: resolve(output, 'external-first-slide.png') });
    return { ...layout, warnings: document.warnings };
  });
  await writeFile(resolve(output, 'results.json'), JSON.stringify({ results, errors }, null, 2));
} finally { await browser.close(); }
assert.deepEqual(errors, [], 'Browser errors');
assert.ok(results.every(result => result.passed), 'PPT display regression failed');
