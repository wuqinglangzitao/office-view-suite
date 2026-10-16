import { SHADOW_CSS } from './styles';
import type { PptDoc, PptPara, PptShadow, PptShape, PptSlide, PptTable } from './types';

const SVG_NS = 'http://www.w3.org/2000/svg';
let markerId = 0;

/**
 * 布局感知渲染:幻灯片按真实宽高比(aspect-ratio)排版,
 * 形状按 EMU→百分比绝对定位,文本映射字号/颜色/粗斜。
 * Shadow DOM 隔离;幻灯片逐个挂载(单页形状量小,无需分批)。
 */
export function renderPptInto(host: HTMLElement, doc: PptDoc): {
  stats: { slides: number; hiddenSlides: number; shapes: number; tables: number };
  dispose(): void;
} {
  const shadow = host.shadowRoot ?? host.attachShadow({ mode: 'open' });
  shadow.replaceChildren();

  const style = document.createElement('style');
  style.textContent = SHADOW_CSS;
  shadow.appendChild(style);

  const deck = document.createElement('div');
  deck.className = 'deck';
  shadow.appendChild(deck);

  let shapes = 0;
  let tables = 0;
  const hiddenSlides = doc.slides.filter(slide => slide.hidden).length;
  for (const slide of doc.slides) {
    deck.appendChild(renderSlide(slide, doc));
    shapes += slide.shapes.length;
    tables += slide.tables.length;
  }

  return {
    stats: {
      slides: doc.slides.length,
      hiddenSlides,
      shapes,
      tables,
    },
    dispose: () => shadow.replaceChildren(),
  };
}

function renderSlide(slide: PptSlide, doc: PptDoc): HTMLElement {
  const el = document.createElement('div');
  el.className = 'slide';
  el.style.aspectRatio = `${doc.slideWidthEmu} / ${doc.slideHeightEmu}`;
  if (slide.background) el.style.background = slide.background;

  for (const shape of slide.shapes) {
    el.appendChild(renderShape(shape, doc));
  }

  for (const table of slide.tables) {
    el.appendChild(renderTable(table, doc));
  }
  return el;
}

function renderShape(shape: PptShape, doc: PptDoc): HTMLElement | SVGSVGElement {
  if (isConnector(shape)) return renderLine(shape, doc);

  const box = document.createElement('div');
  box.className = `shape shape-${shape.shapeType ?? 'rect'}`;
  position(box, shape, doc);
  box.style.transform = transform(shape);
  if (shape.kind === 'shape') appendShapeGeometry(box, shape, doc);
  const padding = [
    cqw(shape.marginTopEmu, doc.slideWidthEmu),
    cqw(shape.marginRightEmu, doc.slideWidthEmu),
    cqw(shape.marginBottomEmu, doc.slideWidthEmu),
    cqw(shape.marginLeftEmu, doc.slideWidthEmu),
  ].join(' ');
  box.style.justifyContent = verticalAlignment(shape.verticalAlign);
  if (shape.autoFit === 'resize') {
    box.classList.add('shape-autofit-resize');
    box.style.minHeight = box.style.height;
    box.style.height = 'auto';
  }

  if (shape.kind === 'picture' && shape.image) {
    if (shape.fill) box.style.background = shape.fill;
    applyStroke(box, shape);
    const image = document.createElement('img');
    image.className = 'picture';
    image.src = shape.image;
    image.alt = '';
    image.draggable = false;
    if (shape.shadow) image.style.filter = shadowFilter(shape.shadow, doc);
    box.appendChild(image);
  }
  const text = document.createElement('div');
  text.className = 'shape-text';
  text.style.padding = padding;
  text.style.setProperty('--font-scale', `${shape.fontScale || 1}`);
  text.style.setProperty('--line-height', `${Math.max(0.1, 1 - (shape.lineSpacingReduction || 0))}`);
  for (const para of shape.paragraphs) {
    text.appendChild(renderPara(para, doc, shape.lineSpacingReduction, shape.wrapText));
  }
  box.appendChild(text);
  if (shape.autoFit === 'shrink') scheduleTextFit(box, text, shape.fontScale || 1);
  return box;
}

function scheduleTextFit(box: HTMLElement, text: HTMLElement, initialScale: number): void {
  requestAnimationFrame(() => {
    let scale = Math.min(1, Math.max(0.1, initialScale));
    const overflows = () => text.scrollHeight > box.clientHeight + 1 || text.scrollWidth > box.clientWidth + 1;
    while (overflows() && scale > 0.4) {
      scale = Math.max(0.4, scale - 0.025);
      text.style.setProperty('--font-scale', `${scale}`);
    }
  });
}

function renderLine(shape: PptShape, doc: PptDoc): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.classList.add('shape', 'connector');
  svg.style.inset = '0';
  svg.style.width = '100%';
  svg.style.height = '100%';
  svg.setAttribute('viewBox', `0 0 ${doc.slideWidthEmu} ${doc.slideHeightEmu}`);
  svg.setAttribute('preserveAspectRatio', 'none');
  const path = document.createElementNS(SVG_NS, 'path');
  path.setAttribute('d', connectorPath(shape));
  path.setAttribute('fill', 'none');
  path.setAttribute('vector-effect', 'non-scaling-stroke');
  path.setAttribute('stroke', shape.stroke ?? '#000');
  path.setAttribute('stroke-width', `${strokePixels(shape)}`);
  path.setAttribute('stroke-linecap', 'round');
  path.setAttribute('stroke-linejoin', 'round');
  if (shape.shadow) path.style.filter = shadowFilter(shape.shadow, doc);
  if (shape.dash && shape.dash !== 'solid') path.setAttribute('stroke-dasharray', '5 4');
  if (shape.rotation) {
    path.setAttribute(
      'transform',
      `rotate(${shape.rotation} ${shape.x + shape.w / 2} ${shape.y + shape.h / 2})`,
    );
  }

  const defs = document.createElementNS(SVG_NS, 'defs');
  const id = ++markerId;
  if (shape.headEnd) {
    const marker = createMarker(`ppt-head-${id}`, shape.headEnd, shape.stroke ?? '#000');
    defs.appendChild(marker);
    path.setAttribute('marker-start', `url(#ppt-head-${id})`);
  }
  if (shape.tailEnd) {
    const marker = createMarker(`ppt-tail-${id}`, shape.tailEnd, shape.stroke ?? '#000');
    defs.appendChild(marker);
    path.setAttribute('marker-end', `url(#ppt-tail-${id})`);
  }
  if (defs.childNodes.length) svg.appendChild(defs);
  svg.appendChild(path);
  return svg;
}

function isConnector(shape: PptShape): boolean {
  return shape.kind === 'line' || shape.shapeType === 'line' || !!shape.shapeType?.includes('Connector');
}

function connectorPath(shape: PptShape): string {
  const startX = shape.x + (shape.flipH ? shape.w : 0);
  const startY = shape.y + (shape.flipV ? shape.h : 0);
  const endX = shape.x + (shape.flipH ? 0 : shape.w);
  const endY = shape.y + (shape.flipV ? 0 : shape.h);
  const midX = (startX + endX) / 2;
  const midY = (startY + endY) / 2;
  switch (shape.shapeType) {
    case 'bentConnector2':
      return `M ${startX} ${startY} L ${endX} ${startY} L ${endX} ${endY}`;
    case 'bentConnector3':
      return `M ${startX} ${startY} L ${midX} ${startY} L ${midX} ${endY} L ${endX} ${endY}`;
    case 'bentConnector4':
      return `M ${startX} ${startY} L ${midX} ${startY} L ${midX} ${midY} L ${endX} ${midY} L ${endX} ${endY}`;
    case 'curvedConnector2':
      return `M ${startX} ${startY} Q ${endX} ${startY} ${endX} ${endY}`;
    case 'curvedConnector3':
    case 'curvedConnector4':
      return `M ${startX} ${startY} C ${midX} ${startY} ${midX} ${endY} ${endX} ${endY}`;
    default:
      return `M ${startX} ${startY} L ${endX} ${endY}`;
  }
}

function createMarker(id: string, type: string, color: string): SVGMarkerElement {
  const marker = document.createElementNS(SVG_NS, 'marker');
  marker.id = id;
  marker.setAttribute('viewBox', '0 0 10 10');
  marker.setAttribute('refX', '9');
  marker.setAttribute('refY', '5');
  marker.setAttribute('markerWidth', '7');
  marker.setAttribute('markerHeight', '7');
  marker.setAttribute('orient', 'auto-start-reverse');
  marker.setAttribute('markerUnits', 'strokeWidth');
  if (type === 'oval') {
    const ellipse = document.createElementNS(SVG_NS, 'ellipse');
    ellipse.setAttribute('cx', '5');
    ellipse.setAttribute('cy', '5');
    ellipse.setAttribute('rx', '4');
    ellipse.setAttribute('ry', '3');
    ellipse.setAttribute('fill', color);
    marker.appendChild(ellipse);
  } else {
    const end = document.createElementNS(SVG_NS, 'path');
    if (type === 'arrow') {
      end.setAttribute('d', 'M 9 1 L 1 5 L 9 9');
      end.setAttribute('fill', 'none');
      end.setAttribute('stroke', color);
      end.setAttribute('stroke-width', '1.5');
    } else if (type === 'diamond') {
      end.setAttribute('d', 'M 1 5 L 5 1 L 9 5 L 5 9 Z');
      end.setAttribute('fill', color);
    } else {
      end.setAttribute('d', type === 'stealth' ? 'M 1 1 L 9 5 L 1 9 L 4 5 Z' : 'M 1 1 L 9 5 L 1 9 Z');
      end.setAttribute('fill', color);
    }
    marker.appendChild(end);
  }
  return marker;
}

function renderPara(
  para: PptPara,
  doc: PptDoc,
  lineSpacingReduction: number,
  wrapText: boolean,
): HTMLElement {
  const p = document.createElement('p');
  const firstRun = para.runs.find(run => run.text !== '\n');
  if (firstRun?.sizePt)
    p.style.fontSize = `calc(${firstRun.sizePt / 9.6}cqw * var(--font-scale, 1))`;
  if (firstRun?.font && !firstRun.font.startsWith('+')) p.style.fontFamily = fontFamily(firstRun.font);
  if (firstRun?.color) p.style.color = firstRun.color;
  if (para.marginLeftEmu !== undefined) p.style.marginLeft = cqw(para.marginLeftEmu, doc.slideWidthEmu);
  else if (para.level) p.style.paddingLeft = `${para.level * 1.4}em`;
  if (para.marginRightEmu !== undefined) p.style.marginRight = cqw(para.marginRightEmu, doc.slideWidthEmu);
  if (para.indentEmu !== undefined) p.style.textIndent = cqw(para.indentEmu, doc.slideWidthEmu);
  if (para.align) p.style.textAlign = para.align;
  if (para.lineSpacingPt !== undefined) p.style.lineHeight = ptCqw(para.lineSpacingPt);
  else if (para.lineSpacing !== undefined)
    p.style.lineHeight = `${Math.max(0.1, para.lineSpacing * (1 - lineSpacingReduction))}`;
  if (para.spaceBeforePt !== undefined) p.style.marginTop = ptCqw(para.spaceBeforePt);
  else if (para.spaceBefore !== undefined) p.style.marginTop = `${para.spaceBefore}em`;
  if (para.spaceAfterPt !== undefined) p.style.marginBottom = ptCqw(para.spaceAfterPt);
  else if (para.spaceAfter !== undefined) p.style.marginBottom = `${para.spaceAfter}em`;
  p.style.whiteSpace = wrapText ? 'pre-wrap' : 'pre';
  if (para.bullet) {
    const bullet = document.createElement('span');
    bullet.className = 'shape-bullet';
    bullet.textContent = `${para.bullet} `;
    const bulletFont = para.bulletFont ?? firstRun?.font;
    if (bulletFont && !bulletFont.startsWith('+')) bullet.style.fontFamily = fontFamily(bulletFont);
    if (firstRun?.sizePt)
      bullet.style.fontSize = `calc(${firstRun.sizePt / 9.6}cqw * var(--font-scale, 1))`;
    if (firstRun?.color) bullet.style.color = firstRun.color;
    if (firstRun?.bold) bullet.style.fontWeight = '700';
    if (firstRun?.italic) bullet.style.fontStyle = 'italic';
    p.appendChild(bullet);
  }
  for (const run of para.runs) {
    const span = document.createElement('span');
    span.textContent = run.text;
    if (run.font && !run.font.startsWith('+')) span.style.fontFamily = fontFamily(run.font);
    if (run.bold) span.style.fontWeight = '700';
    if (run.italic) span.style.fontStyle = 'italic';
    if (run.underline) span.style.textDecoration = 'underline';
    if (run.sizePt) span.style.fontSize = `calc(${run.sizePt / 9.6}cqw * var(--font-scale, 1))`;
    if (run.color) span.style.color = run.color;
    if (run.shadow) span.style.textShadow = textShadow(run.shadow, doc);
    p.appendChild(span);
  }
  return p;
}

function renderTable(table: PptTable, doc: PptDoc): HTMLElement {
  const tbl = document.createElement('table');
  tbl.className = 'tbl';
  tbl.style.left = pct(table.x, doc.slideWidthEmu);
  tbl.style.top = pct(table.y, doc.slideHeightEmu);
  tbl.style.width = pct(table.w, doc.slideWidthEmu);
  tbl.style.height = pct(table.h, doc.slideHeightEmu);
  if (table.columns.length) {
    const colgroup = document.createElement('colgroup');
    const total = table.columns.reduce((sum, value) => sum + value, 0);
    for (const width of table.columns) {
      const col = document.createElement('col');
      col.style.width = pct(width, total);
      colgroup.appendChild(col);
    }
    tbl.appendChild(colgroup);
  }
  table.rows.forEach((row, ri) => {
    const tr = document.createElement('tr');
    if (row.heightEmu) tr.style.height = pct(row.heightEmu, table.h);
    row.cells.forEach(cell => {
      const td = document.createElement(ri === 0 ? 'th' : 'td');
      td.textContent = cell.text;
      if (cell.colSpan > 1) td.colSpan = cell.colSpan;
      if (cell.rowSpan > 1) td.rowSpan = cell.rowSpan;
      if (cell.fill) td.style.background = cell.fill;
      if (cell.color) td.style.color = cell.color;
      if (cell.fontSizePt) td.style.fontSize = `${cell.fontSizePt / 9.6}cqw`;
      if (cell.align) td.style.textAlign = cell.align;
      td.style.padding = [
        cqw(cell.marginTopEmu, doc.slideWidthEmu),
        cqw(cell.marginRightEmu, doc.slideWidthEmu),
        cqw(cell.marginBottomEmu, doc.slideWidthEmu),
        cqw(cell.marginLeftEmu, doc.slideWidthEmu),
      ].join(' ');
      tr.appendChild(td);
    });
    tbl.appendChild(tr);
  });
  return tbl;
}

function position(el: HTMLElement | SVGSVGElement, shape: PptShape, doc: PptDoc): void {
  el.style.left = pct(shape.x, doc.slideWidthEmu);
  el.style.top = pct(shape.y, doc.slideHeightEmu);
  el.style.width = pct(shape.w, doc.slideWidthEmu);
  el.style.height = pct(shape.h, doc.slideHeightEmu);
}

function transform(shape: PptShape): string {
  const transforms = [`rotate(${shape.rotation}deg)`];
  if (shape.flipH || shape.flipV) transforms.push(`scale(${shape.flipH ? -1 : 1},${shape.flipV ? -1 : 1})`);
  return transforms.join(' ');
}

function applyStroke(box: HTMLElement, shape: PptShape): void {
  if (!shape.stroke || shape.stroke === 'transparent') return;
  box.style.borderColor = shape.stroke;
  box.style.borderWidth = `${Math.max(0.5, (shape.strokeWidthEmu ?? 12_700) / 12_700)}px`;
  box.style.borderStyle = shape.dash && shape.dash !== 'solid' ? 'dashed' : 'solid';
}

function appendShapeGeometry(box: HTMLElement, shape: PptShape, doc: PptDoc): void {
  const layer = document.createElement('div');
  layer.className = 'shape-geometry-layer';
  if (shape.shadow) layer.style.filter = shadowFilter(shape.shadow, doc);
  if (shape.customPaths?.length) {
    for (const customPath of shape.customPaths) {
      const svg = geometrySvg(customPath.width, customPath.height);
      const path = document.createElementNS(SVG_NS, 'path');
      path.setAttribute('d', customPath.data);
      styleGeometry(path, shape, customPath.fill, customPath.stroke);
      svg.appendChild(path);
      layer.appendChild(svg);
    }
    box.appendChild(layer);
    return;
  }

  const rounded = shape.shapeType === 'roundRect' || shape.shapeType === 'round2SameRect';
  const svg = rounded ? geometrySvg(shape.w, shape.h) : geometrySvg(100, 100);
  if (shape.shapeType === 'ellipse') {
    const ellipse = document.createElementNS(SVG_NS, 'ellipse');
    ellipse.setAttribute('cx', '50');
    ellipse.setAttribute('cy', '50');
    ellipse.setAttribute('rx', '49');
    ellipse.setAttribute('ry', '49');
    styleGeometry(ellipse, shape, true, true);
    svg.appendChild(ellipse);
  } else {
    const path = document.createElementNS(SVG_NS, 'path');
    path.setAttribute('d', rounded ? roundedRectPath(shape) : presetPath(shape.shapeType));
    styleGeometry(path, shape, shape.shapeType !== 'leftBracket', true);
    svg.appendChild(path);
  }
  layer.appendChild(svg);
  box.appendChild(layer);
}

function geometrySvg(width: number, height: number): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.classList.add('shape-geometry');
  svg.setAttribute('viewBox', `0 0 ${Math.max(1, width)} ${Math.max(1, height)}`);
  svg.setAttribute('preserveAspectRatio', 'none');
  return svg;
}

function styleGeometry(
  geometry: SVGGeometryElement,
  shape: PptShape,
  useFill: boolean,
  useStroke: boolean,
): void {
  geometry.setAttribute('fill', useFill ? svgFill(shape.fill) : 'none');
  geometry.setAttribute('stroke', useStroke ? (shape.stroke ?? 'none') : 'none');
  geometry.setAttribute('stroke-width', `${strokePixels(shape)}`);
  geometry.setAttribute('vector-effect', 'non-scaling-stroke');
  geometry.setAttribute('stroke-linejoin', 'round');
  if (shape.dash && shape.dash !== 'solid') geometry.setAttribute('stroke-dasharray', '5 4');
}

function svgFill(fill?: string): string {
  if (!fill || fill === 'transparent') return 'none';
  if (!fill.includes('gradient(')) return fill;
  return fill.match(/#[0-9a-f]{6,8}/i)?.[0] ?? 'none';
}

function strokePixels(shape: PptShape): number {
  return Math.max(0.6, (shape.strokeWidthEmu ?? 12_700) / 12_700);
}

function roundedRectPath(shape: PptShape): string {
  const width = Math.max(1, Math.abs(shape.w));
  const height = Math.max(1, Math.abs(shape.h));
  const adjustment = Math.min(50_000, Math.max(0, shape.adjustments?.[0] ?? 16_667));
  const radius = Math.min(width, height) * adjustment / 100_000;
  if (radius <= 0) return `M 0 0 H ${width} V ${height} H 0 Z`;
  if (shape.shapeType === 'round2SameRect') {
    return `M ${radius} 0 H ${width - radius} A ${radius} ${radius} 0 0 1 ${width} ${radius} V ${height} H 0 V ${radius} A ${radius} ${radius} 0 0 1 ${radius} 0 Z`;
  }
  return `M ${radius} 0 H ${width - radius} A ${radius} ${radius} 0 0 1 ${width} ${radius} V ${height - radius} A ${radius} ${radius} 0 0 1 ${width - radius} ${height} H ${radius} A ${radius} ${radius} 0 0 1 0 ${height - radius} V ${radius} A ${radius} ${radius} 0 0 1 ${radius} 0 Z`;
}

function presetPath(shapeType?: string): string {
  switch (shapeType) {
    case 'roundRect':
    case 'round2SameRect': return 'M 10 1 H 90 Q 99 1 99 10 V 90 Q 99 99 90 99 H 10 Q 1 99 1 90 V 10 Q 1 1 10 1 Z';
    case 'triangle': return 'M 50 1 L 99 99 L 1 99 Z';
    case 'rtTriangle': return 'M 1 1 L 99 99 L 1 99 Z';
    case 'diamond': return 'M 50 1 L 99 50 L 50 99 L 1 50 Z';
    case 'parallelogram': return 'M 20 1 H 99 L 80 99 H 1 Z';
    case 'trapezoid': return 'M 20 1 H 80 L 99 99 H 1 Z';
    case 'pentagon': return 'M 50 1 L 99 38 L 80 99 H 20 L 1 38 Z';
    case 'hexagon': return 'M 25 1 H 75 L 99 50 L 75 99 H 25 L 1 50 Z';
    case 'octagon': return 'M 30 1 H 70 L 99 30 V 70 L 70 99 H 30 L 1 70 V 30 Z';
    case 'chevron': return 'M 1 1 H 62 L 99 50 L 62 99 H 1 L 38 50 Z';
    case 'homePlate': return 'M 1 1 H 70 L 99 50 L 70 99 H 1 Z';
    case 'leftArrow': return 'M 1 50 L 38 1 V 28 H 99 V 72 H 38 V 99 Z';
    case 'rightArrow': return 'M 99 50 L 62 1 V 28 H 1 V 72 H 62 V 99 Z';
    case 'upArrow': return 'M 50 1 L 99 38 H 72 V 99 H 28 V 38 H 1 Z';
    case 'downArrow': return 'M 50 99 L 99 62 H 72 V 1 H 28 V 62 H 1 Z';
    case 'leftRightArrow': return 'M 1 50 L 28 20 V 35 H 72 V 20 L 99 50 L 72 80 V 65 H 28 V 80 Z';
    case 'upDownArrow': return 'M 50 1 L 80 28 H 65 V 72 H 80 L 50 99 L 20 72 H 35 V 28 H 20 Z';
    case 'leftBracket': return 'M 80 1 H 55 Q 20 1 20 25 V 75 Q 20 99 55 99 H 80';
    default: return 'M 1 1 H 99 V 99 H 1 Z';
  }
}

function verticalAlignment(value?: PptShape['verticalAlign']): string {
  if (value === 'center') return 'center';
  if (value === 'bottom') return 'flex-end';
  return 'flex-start';
}

function fontFamily(font: string): string {
  const quoted = `"${font}"`;
  if (/宋体|SimSun|STSong|Songti/i.test(font))
    return `${quoted}, "Songti SC", "STSong", "SimSun", serif`;
  if (/仿宋|FangSong/i.test(font))
    return `${quoted}, "FangSong SC", "STFangsong", "Songti SC", serif`;
  if (/微软雅黑|Microsoft YaHei|思源黑体|Source Han Sans|等线/i.test(font))
    return `${quoted}, "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", sans-serif`;
  return `${quoted}, "PingFang SC", "Helvetica Neue", Arial, sans-serif`;
}

function shadowOffset(shadow: PptShadow): { x: number; y: number } {
  const radians = shadow.directionDeg * Math.PI / 180;
  return {
    x: Math.cos(radians) * shadow.distanceEmu,
    y: Math.sin(radians) * shadow.distanceEmu,
  };
}

function shadowFilter(shadow: PptShadow, doc: PptDoc): string {
  const offset = shadowOffset(shadow);
  return `drop-shadow(${cqw(offset.x, doc.slideWidthEmu)} ${cqw(offset.y, doc.slideWidthEmu)} ${cqw(shadow.blurRadiusEmu / 2, doc.slideWidthEmu)} ${shadow.color})`;
}

function textShadow(shadow: PptShadow, doc: PptDoc): string {
  const offset = shadowOffset(shadow);
  return `${cqw(offset.x, doc.slideWidthEmu)} ${cqw(offset.y, doc.slideWidthEmu)} ${cqw(shadow.blurRadiusEmu / 2, doc.slideWidthEmu)} ${shadow.color}`;
}

function ptCqw(points: number): string {
  return `${points / 9.6}cqw`;
}

function pct(emu: number, total: number): string {
  return `${(emu / total) * 100}%`;
}

function cqw(emu: number, slideWidthEmu: number): string {
  return `${(emu / slideWidthEmu) * 100}cqw`;
}
