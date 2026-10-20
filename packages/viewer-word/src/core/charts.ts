import type { WordChart } from './types';

const SVG_NS = 'http://www.w3.org/2000/svg';
const COLORS = ['#4472c4', '#ed7d31', '#a5a5a5', '#ffc000', '#5b9bd5', '#70ad47'];
type Point = { x: number; y: number };

function node<K extends keyof SVGElementTagNameMap>(tag: K, attrs: Record<string, string | number> = {}): SVGElementTagNameMap[K] {
  const element = document.createElementNS(SVG_NS, tag);
  for (const [key, value] of Object.entries(attrs)) element.setAttribute(key, String(value));
  return element;
}

function label(svg: SVGElement, text: string, x: number, y: number, anchor = 'middle', size = 11): void {
  const element = node('text', { x, y, 'text-anchor': anchor, 'font-size': size, fill: '#333' });
  element.textContent = text;
  svg.appendChild(element);
}

function describe(element: SVGElement, text: string): void {
  const title = node('title');
  title.textContent = text;
  element.appendChild(title);
}

function color(index: number): string { return COLORS[index % COLORS.length]; }
function finite(value: number | undefined): number { return Number.isFinite(value) ? value! : 0; }
function tick(value: number): string { return Number(value.toPrecision(4)).toLocaleString(); }
function path(points: Point[], close = false): string {
  return points.map((p, i) => `${i ? 'L' : 'M'} ${p.x} ${p.y}`).join(' ') + (close ? ' Z' : '');
}

function smoothPath(points: Point[]): string {
  return points.map((point, i) => {
    if (!i) return `M ${point.x} ${point.y}`;
    const previous = points[i - 1], middle = (previous.x + point.x) / 2;
    return `C ${middle} ${previous.y} ${middle} ${point.y} ${point.x} ${point.y}`;
  }).join(' ');
}

/** Preserve the source drawing size; unsupported chart families retain their data. */
export function renderWordChart(chart: WordChart): HTMLElement {
  const figure = document.createElement('figure');
  figure.className = 'word-chart';
  const width = Math.max(1, chart.widthPx ?? 480);
  const height = Math.max(1, chart.heightPx ?? 320);
  figure.style.cssText = `margin:0;width:${width}px;max-width:100%;min-height:0;break-inside:avoid`;
  const kind = chart.kind.toLowerCase();
  const pie = /^(exploded)?(pie|doughnut)(3d)?$/.test(kind);
  const radar = kind.includes('radar');
  const scatter = kind.startsWith('scatter') || kind.startsWith('bubble');
  const cartesian = /^(percentstacked|stacked)?(bar|column|line|area)(3d|nomarkers)?$/.test(kind) || kind === 'smoothline';
  if (!chart.series.length || (!pie && !radar && !scatter && !cartesian)) {
    appendDataTable(figure, chart, height);
    return figure;
  }
  const svg = node('svg', { viewBox: `0 0 ${width} ${height}`, role: 'img', width, height });
  svg.classList.add('word-chart-svg');
  svg.style.cssText = 'display:block;width:100%;height:auto;overflow:hidden;font-family:Arial,"PingFang SC",sans-serif';
  describe(svg, chart.alt || chart.title || '图表');
  if (chart.title) label(svg, chart.title, width / 2, 23, 'middle', 15);
  if (kind.includes('3d')) label(svg, '二维预览', width - 8, 13, 'end', 10);
  const names = pie ? chart.categories : chart.series.map(series => series.name);
  const legendCols = Math.max(1, Math.floor(width / 125));
  const legendRows = Math.ceil(names.length / legendCols);
  const top = chart.title ? 40 : 20;
  const bottom = Math.max(top + 10, height - 36 - legendRows * 18);
  const left = Math.min(70, width * 0.2), right = Math.max(left + 10, width - 22);
  const bounds = { left, right, top, bottom };
  if (pie) renderPie(svg, chart, bounds, kind.includes('doughnut'), kind.startsWith('exploded'));
  else if (radar) renderRadar(svg, chart, bounds, kind);
  else if (scatter) renderScatter(svg, chart, bounds, kind);
  else renderCartesian(svg, chart, bounds, kind);
  names.forEach((name, i) => {
    const x = 12 + (i % legendCols) * (width / legendCols);
    const y = height - (legendRows - Math.floor(i / legendCols)) * 18 + 12;
    svg.appendChild(node('rect', { x, y: y - 9, width: 10, height: 10, fill: color(i) }));
    const short = name.length > 14 ? `${name.slice(0, 13)}…` : name;
    label(svg, short, x + 15, y, 'start');
  });
  figure.appendChild(svg);
  return figure;
}

type Bounds = { left: number; right: number; top: number; bottom: number };

function renderCartesian(svg: SVGElement, chart: WordChart, bounds: Bounds, kind: string): void {
  const { left, right, top, bottom } = bounds;
  const count = Math.max(chart.categories.length, ...chart.series.map(series => series.values.length));
  if (!count) return;
  const horizontal = kind.includes('bar');
  const bars = horizontal || kind.includes('column');
  const stacked = kind.includes('stacked'), percent = kind.startsWith('percent');
  const positive = new Array<number>(count).fill(0), negative = new Array<number>(count).fill(0);
  const totals = Array.from({ length: count }, (_, i) => chart.series.reduce((sum, series) => sum + Math.abs(finite(series.values[i])), 0));
  const layers = chart.series.map(series => Array.from({ length: count }, (_, i) => {
    const raw = finite(series.values[i]);
    const value = percent ? (totals[i] ? raw / totals[i] * 100 : 0) : raw;
    const stack = value < 0 ? negative : positive;
    const start = stacked ? stack[i] : 0;
    const end = start + value;
    if (stacked) stack[i] = end;
    return { raw, value, start, end };
  }));
  let min = 0, max = 0;
  for (const layer of layers) for (const value of layer) { min = Math.min(min, value.start, value.end); max = Math.max(max, value.start, value.end); }
  if (min === max) max = min + 1;
  const position = (value: number) => horizontal
    ? left + (value - min) / (max - min) * (right - left)
    : bottom - (value - min) / (max - min) * (bottom - top);
  for (let i = 0; i <= 4; i++) {
    const value = min + (max - min) * i / 4;
    const at = position(value);
    svg.appendChild(node('line', horizontal
      ? { x1: at, x2: at, y1: top, y2: bottom, stroke: '#ddd' }
      : { x1: left, x2: right, y1: at, y2: at, stroke: '#ddd' }));
    label(svg, `${tick(value)}${percent ? '%' : ''}`, horizontal ? at : left - 7, horizontal ? bottom + 15 : at + 4, horizontal ? 'middle' : 'end');
  }
  svg.appendChild(node('line', horizontal
    ? { x1: position(0), x2: position(0), y1: top, y2: bottom, stroke: '#777' }
    : { x1: left, x2: right, y1: position(0), y2: position(0), stroke: '#777' }));
  const band = (horizontal ? bottom - top : right - left) / count;
  const categoryAt = (i: number) => (horizontal ? top : left) + band * (i + 0.5);
  const step = Math.max(1, Math.ceil(count / Math.max(1, Math.floor((horizontal ? bottom - top : right - left) / (horizontal ? 18 : 60)))));
  for (let i = 0; i < count; i += step) {
    const name = chart.categories[i] ?? String(i + 1);
    label(svg, name.length > 10 ? `${name.slice(0, 9)}…` : name, horizontal ? left - 7 : categoryAt(i), horizontal ? categoryAt(i) + 4 : bottom + 16, horizontal ? 'end' : 'middle');
  }
  layers.forEach((layer, s) => {
    const points: Point[] = [], bases: Point[] = [];
    layer.forEach((value, i) => {
      const center = categoryAt(i);
      if (bars) {
        const size = band * 0.72 / (stacked ? 1 : layers.length);
        const start = center - band * 0.36 + (stacked ? 0 : s * size);
        const from = position(value.start), to = position(value.end);
        const rect = node('rect', horizontal
          ? { x: Math.min(from, to), y: start, width: Math.abs(to - from), height: size, fill: color(s) }
          : { x: start, y: Math.min(from, to), width: size, height: Math.abs(to - from), fill: color(s) });
        rect.dataset.chartValue = String(value.raw);
        describe(rect, `${chart.series[s].name} · ${chart.categories[i] ?? i + 1}: ${value.raw}`);
        svg.appendChild(rect);
      } else {
        points.push({ x: center, y: position(value.end) });
        bases.push({ x: center, y: position(value.start) });
      }
    });
    if (points.length) {
      const area = kind.includes('area');
      const d = kind === 'smoothline' ? smoothPath(points) : path(area ? [...points, ...bases.reverse()] : points, area);
      const line = node('path', { d, fill: area ? color(s) : 'none', 'fill-opacity': area ? 0.6 : 1, stroke: color(s), 'stroke-width': 2 });
      describe(line, chart.series[s].name);
      svg.appendChild(line);
      if (!area && !kind.includes('nomarkers')) for (const [i, point] of points.entries()) {
        const dot = node('circle', { cx: point.x, cy: point.y, r: 3, fill: color(s) });
        describe(dot, `${chart.series[s].name} · ${chart.categories[i] ?? i + 1}: ${layer[i].raw}`);
        svg.appendChild(dot);
      }
    }
  });
}

function renderPie(svg: SVGElement, chart: WordChart, b: Bounds, doughnut: boolean, exploded: boolean): void {
  const values = chart.series[0].values.map(value => Math.abs(finite(value)));
  const total = values.reduce((sum, value) => sum + value, 0);
  if (!total) return;
  const singleSlice = values.filter(value => value > 0).length === 1;
  const radius = Math.max(1, Math.min(b.right - b.left, b.bottom - b.top) / 2 - 12);
  const cx = (b.left + b.right) / 2, cy = (b.top + b.bottom) / 2;
  let angle = -Math.PI / 2;
  values.forEach((value, i) => {
    if (!value) return;
    const delta = value / total * Math.PI * 2, end = angle + delta;
    const mid = (angle + end) / 2;
    const x = cx + (exploded ? Math.cos(mid) * radius * 0.07 : 0);
    const y = cy + (exploded ? Math.sin(mid) * radius * 0.07 : 0);
    // Two arcs also handle a single nonzero slice (a full circle).
    const polar = (a: number, r: number) => `${x + Math.cos(a) * r} ${y + Math.sin(a) * r}`;
    const inner = radius * 0.5;
    let d = `M ${polar(angle, radius)} A ${radius} ${radius} 0 0 1 ${polar(mid, radius)} A ${radius} ${radius} 0 0 1 ${polar(end, radius)}`;
    d += doughnut
      ? `${singleSlice ? ' Z M' : ' L'} ${polar(end, inner)} A ${inner} ${inner} 0 0 0 ${polar(mid, inner)} A ${inner} ${inner} 0 0 0 ${polar(angle, inner)} Z`
      : singleSlice ? ' Z' : ` L ${x} ${y} Z`;
    const slice = node('path', { d, fill: color(i), stroke: '#fff', 'stroke-width': 1 });
    describe(slice, `${chart.categories[i] ?? i + 1}: ${value} (${tick(value / total * 100)}%)`);
    svg.appendChild(slice);
    angle = end;
  });
}

function renderRadar(svg: SVGElement, chart: WordChart, b: Bounds, kind: string): void {
  const count = Math.max(chart.categories.length, ...chart.series.map(series => series.values.length));
  if (!count) return;
  let minimum = 0, maximum = 0;
  for (const series of chart.series) for (const value of series.values) {
    minimum = Math.min(minimum, finite(value));
    maximum = Math.max(maximum, finite(value));
  }
  if (minimum === maximum) maximum = minimum + 1;
  const cx = (b.left + b.right) / 2, cy = (b.top + b.bottom) / 2;
  const radius = Math.max(1, Math.min(b.right - b.left, b.bottom - b.top) / 2 - 20);
  const point = (i: number, scale: number) => ({ x: cx + Math.sin(i / count * Math.PI * 2) * radius * scale, y: cy - Math.cos(i / count * Math.PI * 2) * radius * scale });
  for (let ring = 1; ring <= 4; ring++) {
    svg.appendChild(node('path', { d: path(Array.from({ length: count }, (_, i) => point(i, ring / 4)), true), fill: 'none', stroke: '#ddd' }));
    label(svg, tick(minimum + (maximum - minimum) * ring / 4), cx + 4, cy - radius * ring / 4, 'start', 9);
  }
  if (minimum < 0) label(svg, tick(minimum), cx + 4, cy + 4, 'start', 9);
  for (let i = 0; i < count; i++) { const p = point(i, 1.15); label(svg, chart.categories[i] ?? String(i + 1), p.x, p.y + 4); }
  chart.series.forEach((series, s) => {
    const points = Array.from({ length: count }, (_, i) => point(i, (finite(series.values[i]) - minimum) / (maximum - minimum)));
    const outline = node('path', { d: path(points, true), fill: kind === 'filledradar' ? color(s) : 'none', 'fill-opacity': 0.25, stroke: color(s), 'stroke-width': 2 });
    describe(outline, `${series.name}: ${series.values.join(', ')}`);
    svg.appendChild(outline);
    if (kind === 'radarwithmarkers') points.forEach((point, i) => {
      const dot = node('circle', { cx: point.x, cy: point.y, r: 3, fill: color(s) });
      describe(dot, `${series.name} · ${chart.categories[i] ?? i + 1}: ${finite(series.values[i])}`);
      svg.appendChild(dot);
    });
  });
}

function renderScatter(svg: SVGElement, chart: WordChart, b: Bounds, kind: string): void {
  const values = chart.series.flatMap(series => series.values).filter(Number.isFinite);
  if (!values.length) return;
  const count = Math.max(...chart.series.map(series => series.values.length));
  const numericCategories = chart.categories.length >= count
    && chart.categories.slice(0, count).every(category => category.trim() !== '' && Number.isFinite(Number(category)));
  // The parser's chart model can lack the source xVal cache. Keep every point
  // in view and disclose an ordinal axis instead of inventing source values.
  const xValues = Array.from({ length: count }, (_, i) => numericCategories ? Number(chart.categories[i]) : i + 1);
  if (!numericCategories) label(svg, 'X轴按序号预览', b.right, b.top + 10, 'end', 10);
  let xMin = 0, xMax = 1, yMin = 0, yMax = 1, bubbleMax = 1;
  for (const x of xValues) { xMin = Math.min(xMin, x); xMax = Math.max(xMax, x); }
  for (const y of values) { yMin = Math.min(yMin, y); yMax = Math.max(yMax, y); }
  for (const series of chart.series) for (const size of series.bubbleSizes ?? []) bubbleMax = Math.max(bubbleMax, finite(size));
  const x = (value: number) => b.left + (value - xMin) / (xMax - xMin) * (b.right - b.left);
  const y = (value: number) => b.bottom - (value - yMin) / (yMax - yMin) * (b.bottom - b.top);
  for (let i = 0; i <= 4; i++) {
    const xv = xMin + (xMax - xMin) * i / 4, yv = yMin + (yMax - yMin) * i / 4;
    svg.appendChild(node('line', { x1: b.left, x2: b.right, y1: y(yv), y2: y(yv), stroke: '#ddd' }));
    label(svg, tick(xv), x(xv), b.bottom + 16);
    label(svg, tick(yv), b.left - 7, y(yv) + 4, 'end');
  }
  chart.series.forEach((series, s) => {
    const points = series.values.map((value, i) => ({ x: x(xValues[i] ?? i), y: y(finite(value)) }));
    if (kind.includes('lines') || kind.includes('smooth')) svg.appendChild(node('path', { d: kind.includes('smooth') ? smoothPath(points) : path(points), fill: 'none', stroke: color(s), 'stroke-width': 2 }));
    if (kind === 'scatterlines' || kind === 'scattersmoothnomarkers') return;
    points.forEach((point, i) => {
      const radius = kind.startsWith('bubble') ? Math.sqrt(Math.max(0, finite(series.bubbleSizes?.[i])) / bubbleMax) * 18 : 3;
      const dot = node('circle', { cx: point.x, cy: point.y, r: radius, fill: color(s), 'fill-opacity': 0.7 });
      describe(dot, `${series.name}: ${xValues[i] ?? i}, ${series.values[i]}`);
      svg.appendChild(dot);
    });
  });
}

function appendDataTable(figure: HTMLElement, chart: WordChart, height: number): void {
  figure.style.height = `${height}px`;
  figure.style.overflow = 'auto';
  const caption = document.createElement('figcaption');
  caption.textContent = `${chart.title || chart.alt || '图表'}（数据预览）`;
  const table = document.createElement('table');
  table.style.cssText = 'border-collapse:collapse;width:100%;font-size:12px';
  const headings = document.createElement('tr');
  for (const name of ['', ...chart.series.map(series => series.name)]) {
    const th = document.createElement('th'); th.textContent = name; headings.appendChild(th);
  }
  const head = document.createElement('thead'); head.appendChild(headings); table.appendChild(head);
  const body = document.createElement('tbody');
  const count = Math.max(chart.categories.length, 0, ...chart.series.map(series => series.values.length));
  for (let i = 0; i < count; i++) {
    const row = document.createElement('tr');
    for (const value of [chart.categories[i] ?? String(i + 1), ...chart.series.map(series => series.values[i] === undefined ? '' : String(series.values[i]))]) {
      const cell = document.createElement('td'); cell.textContent = value; cell.style.border = '1px solid #ddd'; row.appendChild(cell);
    }
    body.appendChild(row);
  }
  table.appendChild(body);
  figure.append(caption, table);
}
