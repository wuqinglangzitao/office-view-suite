import assert from 'node:assert/strict';
import { mkdir, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../', import.meta.url));
const { build } = await import(new URL('../packages/viewer/node_modules/vite/dist/node/index.js', import.meta.url));
const { chromium } = await import(process.env.PLAYWRIGHT_MODULE || 'playwright');
const bundle = await build({
  configFile: false,
  logLevel: 'silent',
  build: {
    write: false,
    minify: false,
    lib: { entry: resolve(root, 'packages/viewer-word/src/core/charts.ts'), name: 'chartRegression', formats: ['iife'] },
  },
});
const output = resolve(root, 'output/playwright/word-charts');
await mkdir(output, { recursive: true });
const browser = await chromium.launch({ channel: 'chrome', headless: true });
const errors = [], results = [];
try {
  const page = await browser.newPage({ viewport: { width: 900, height: 760 } });
  page.on('pageerror', error => errors.push(error.message));
  await page.setContent('<style>body{margin:20px;background:white}#host{width:480px}</style><div id="host"></div>');
  await page.addScriptTag({ content: bundle[0].output.find(item => item.type === 'chunk').code });
  const base = {
    kind: 'column', title: '运输量', categories: ['一月', '二月', '三月'],
    series: [{ name: '公路', values: [10, -5, 15] }, { name: '铁路', values: [8, 12, 20] }],
    widthPx: 480, heightPx: 320,
  };
  const render = chart => page.evaluate(chart => {
    const figure = chartRegression.renderWordChart(chart);
    document.querySelector('#host').replaceChildren(figure);
    return {
      svg: !!figure.querySelector('svg'),
      viewBox: figure.querySelector('svg')?.getAttribute('viewBox'),
      bars: [...figure.querySelectorAll('rect[data-chart-value]')].map(rect => ({
        value: Number(rect.dataset.chartValue), x: Number(rect.getAttribute('x')), y: Number(rect.getAttribute('y')),
        width: Number(rect.getAttribute('width')), height: Number(rect.getAttribute('height')),
      })),
      paths: [...figure.querySelectorAll('path')].map(path => ({ d: path.getAttribute('d'), fill: path.getAttribute('fill') })),
      circles: figure.querySelectorAll('circle').length,
      circlePositions: [...figure.querySelectorAll('circle')].map(dot => [Number(dot.getAttribute('cx')), Number(dot.getAttribute('cy'))]),
      text: figure.textContent,
      invalidGeometry: /NaN|Infinity/.test([...figure.querySelectorAll('path,rect,circle,line')].map(node => [...node.attributes].map(attr => attr.value).join(' ')).join(' ')),
      tableRows: figure.querySelectorAll('tbody tr').length,
    };
  }, chart);
  let result = await render(base);
  assert.equal(result.svg, true);
  assert.equal(result.viewBox, '0 0 480 320');
  assert.deepEqual(result.bars.map(bar => bar.value), [10, -5, 15, 8, 12, 20]);
  assert.ok(result.bars.every(bar => bar.width > 0 && bar.height > 0));
  const zero = result.bars[0].y + result.bars[0].height;
  assert.ok(Math.abs(result.bars[1].y - zero) < 0.01, 'Negative column must extend below the zero baseline');
  results.push({ name: 'column-positive-negative-and-source-size', passed: true });
  await page.locator('figure').screenshot({ path: resolve(output, 'column.png') });

  result = await render({ ...base, kind: 'stackedColumn', series: [{ name: 'A', values: [10, -5] }, { name: 'B', values: [20, -8] }], categories: ['正值', '负值'] });
  assert.ok(Math.abs(result.bars[0].y - result.bars[2].y - result.bars[2].height) < 0.01);
  assert.ok(Math.abs(result.bars[1].y + result.bars[1].height - result.bars[3].y) < 0.01);
  results.push({ name: 'stacked-positive-negative-bases', passed: true });

  result = await render({ ...base, kind: 'percentStackedColumn', categories: ['A'], series: [{ name: 'one', values: [20] }, { name: 'two', values: [80] }] });
  assert.match(result.text, /100%/);
  assert.ok(Math.abs(result.bars[1].height / result.bars[0].height - 4) < 0.01);
  results.push({ name: 'percent-stacked-values', passed: true });

  for (const kind of ['bar', 'line', 'area', 'scatterMarkers', 'bubble', 'radar', 'filledRadar', 'pie', 'doughnut']) {
    result = await render({ ...base, kind, categories: ['1', '2', '3'], series: [{ name: '数据', values: [5, 10, 20], bubbleSizes: [1, 4, 9] }] });
    assert.ok(result.svg && !result.invalidGeometry, kind);
    assert.ok(result.paths.length || result.bars.length || result.circles, `${kind} has visible geometry`);
    results.push({ name: `chart-family-${kind}`, passed: true });
  }
  result = await render({ ...base, kind: 'pie', categories: ['唯一类别'], series: [{ name: 'A', values: [30] }] });
  assert.equal((result.paths[0].d.match(/ A /g) || []).length, 2, 'Single slice must render a full circle');
  assert.equal(result.paths[0].d.includes(' L '), false, 'A full pie must not have a radial stroke through its center');
  results.push({ name: 'single-slice-circle', passed: true });
  await page.locator('figure').screenshot({ path: resolve(output, 'pie.png') });

  result = await render({ ...base, kind: 'smoothLine' });
  assert.ok(result.paths.some(path => path.d.includes(' C ')));
  results.push({ name: 'smooth-line-retains-curves', passed: true });

  result = await render({ ...base, kind: 'scatter', categories: [], series: [{ name: '原始数值', values: [1, 5, 9, 12, 20] }] });
  assert.equal(result.circles, 5);
  assert.ok(result.circlePositions.every(([x, y]) => x >= 0 && x <= 480 && y >= 0 && y <= 320));
  assert.match(result.text, /X轴按序号预览/);
  results.push({ name: 'scatter-without-x-cache-keeps-every-point-visible', passed: true });

  result = await render({ ...base, kind: 'radarWithMarkers', series: [{ name: '变化', values: [-10, 0, 20] }] });
  assert.equal(result.circles, 3);
  assert.match(result.text, /-10/);
  assert.ok(result.circlePositions[1][0] !== result.circlePositions[0][0] || result.circlePositions[1][1] !== result.circlePositions[0][1]);
  results.push({ name: 'radar-retains-markers-and-negative-values', passed: true });

  result = await render({ ...base, kind: 'surface', title: '<img src=x onerror=alert(1)>', categories: ['A', 'B'], series: [{ name: '原始数据', values: [2, 7] }] });
  assert.equal(result.tableRows, 2);
  assert.match(result.text, /2/);
  assert.equal(await page.locator('figure img').count(), 0);
  assert.match(result.text, /数据预览/);
  results.push({ name: 'unsupported-family-retains-data-safely', passed: true });
  result = await render({ ...base, kind: 'column3D' });
  assert.match(result.text, /二维预览/);
  assert.equal(result.bars.length, 6);
  results.push({ name: 'three-dimensional-chart-explicit-planar-preview', passed: true });

  assert.deepEqual(errors, []);
  console.log(JSON.stringify({ passed: results.length, results, errors }, null, 2));
} finally {
  await writeFile(resolve(output, 'results.json'), JSON.stringify({ results, errors }, null, 2));
  await browser.close();
}
