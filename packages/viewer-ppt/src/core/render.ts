import { SHADOW_CSS } from './styles';
import { presetPath } from './geometry';
import { applyTextPaint, cssLineStyle, dashArray, gradientCss, paintEndpoint, svgPaint } from './paint';
import type { PptDoc, PptPara, PptShadow, PptShape, PptSlide, PptTable, PptTransform } from './types';

const SVG_NS = 'http://www.w3.org/2000/svg';
let markerId = 0;

/**
 * 布局感知渲染:幻灯片按真实宽高比(aspect-ratio)排版,
 * 形状按 EMU→百分比绝对定位,文本映射字号/颜色/粗斜。
 * Shadow DOM 隔离;幻灯片逐个挂载(单页形状量小,无需分批)。
 */
export function renderPptInto(host: HTMLElement, doc: PptDoc): {
  stats: { slides: number; hiddenSlides: number; shapes: number; tables: number };
  resize(): void;
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
  let disposed = false;
  let fitFrame: number | undefined;
  const textFits: (() => void)[] = [];
  const hiddenSlides = doc.slides.filter(slide => slide.hidden).length;
  for (const slide of doc.slides) {
    deck.appendChild(renderSlide(slide, doc, textFits));
    shapes += slide.shapes.length;
    tables += slide.tables.length;
  }

  const resize = () => {
    if (disposed || fitFrame !== undefined || textFits.length === 0) return;
    fitFrame = requestAnimationFrame(() => {
      fitFrame = undefined;
      if (disposed) return;
      for (const fit of textFits) fit();
    });
  };
  resize();

  return {
    stats: {
      slides: doc.slides.length,
      hiddenSlides,
      shapes,
      tables,
    },
    resize,
    dispose() {
      if (disposed) return;
      disposed = true;
      if (fitFrame !== undefined) cancelAnimationFrame(fitFrame);
      fitFrame = undefined;
      textFits.length = 0;
      deck.remove();
      style.remove();
    },
  };
}

function renderSlide(slide: PptSlide, doc: PptDoc, textFits: (() => void)[]): HTMLElement {
  const el = document.createElement('div');
  el.className = 'slide';
  el.style.aspectRatio = `${doc.slideWidthEmu} / ${doc.slideHeightEmu}`;
  el.style.setProperty('--default-font-size', ptCqw(18, doc));
  if (slide.background) el.style.background = slide.background;

  const nodes = [
    ...slide.shapes.map((shape, index) => ({ order: shape.zOrder ?? index, render: () => renderShape(shape, doc, textFits) })),
    ...slide.tables.map((table, index) => ({ order: table.zOrder ?? slide.shapes.length + index, render: () => renderTable(table, doc) })),
  ];
  for (const node of nodes.sort((a, b) => a.order - b.order)) {
    el.appendChild(node.render());
  }
  return el;
}

function renderShape(shape: PptShape, doc: PptDoc, textFits: (() => void)[]): HTMLElement | SVGSVGElement {
  shape = physicalBox(shape);
  if (isConnector(shape)) return renderLine(shape, doc);

  const box = document.createElement('div');
  box.className = `shape shape-${shape.shapeType ?? 'rect'}`;
  position(box, shape, doc);
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
    const clip = pictureClip(shape);
    if (shape.fillGradient) clip.style.background = gradientCss(shape.fillGradient);
    else if (shape.fill) clip.style.background = shape.fill;
    const image = document.createElement('img');
    image.className = 'picture';
    image.src = shape.image;
    image.alt = '';
    image.draggable = false;
    if (shape.crop) {
      const width = Math.max(0.0001, 1 - shape.crop.left - shape.crop.right);
      const height = Math.max(0.0001, 1 - shape.crop.top - shape.crop.bottom);
      image.style.width = `${100 / width}%`;
      image.style.height = `${100 / height}%`;
      image.style.left = `${-shape.crop.left * 100 / width}%`;
      image.style.top = `${-shape.crop.top * 100 / height}%`;
    }
    clip.appendChild(image);
    if (shape.shadow) box.style.filter = shadowFilter(shape.shadow, doc);
    box.appendChild(clip);
    if (shape.stroke && shape.stroke !== 'transparent') appendShapeGeometry(box, { ...shape, fill: 'transparent', fillGradient: undefined, shadow: undefined }, doc);
  }
  const text = document.createElement('div');
  text.className = 'shape-text';
  text.style.padding = padding;
  applyWritingMode(text, shape.verticalText);
  text.style.setProperty('--font-scale', `${shape.fontScale || 1}`);
  text.style.setProperty('--line-height', `${Math.max(0.1, 1 - (shape.lineSpacingReduction || 0))}`);
  for (const para of shape.paragraphs) {
    text.appendChild(renderPara(para, doc, shape.lineSpacingReduction, shape.wrapText));
  }
  box.appendChild(text);
  if (shape.autoFit === 'shrink') textFits.push(() => fitText(box, text, shape.fontScale || 1));
  return box;
}

function fitText(box: HTMLElement, text: HTMLElement, initialScale: number): void {
  if (box.clientWidth === 0 || box.clientHeight === 0) return;
  let scale = Math.min(1, Math.max(0.1, initialScale));
  text.style.setProperty('--font-scale', `${scale}`);
  const overflows = () => text.scrollHeight > box.clientHeight + 1 || text.scrollWidth > box.clientWidth + 1;
  while (overflows() && scale > 0.4) {
    scale = Math.max(0.4, scale - 0.025);
    text.style.setProperty('--font-scale', `${scale}`);
  }
}

function renderLine(shape: PptShape, doc: PptDoc): SVGSVGElement {
  const svg = document.createElementNS(SVG_NS, 'svg');
  svg.classList.add('shape', 'connector');
  const local = !!shape.transform;
  // Use the slide coordinate system for connectors, including zero-height or
  // zero-width lines. Both line widths and arrowheads then scale uniformly.
  svg.style.inset = '0'; svg.style.width = '100%'; svg.style.height = '100%';
  svg.setAttribute('viewBox', `0 0 ${doc.slideWidthEmu} ${doc.slideHeightEmu}`);
  svg.setAttribute('preserveAspectRatio', 'none');
  const path = document.createElementNS(SVG_NS, 'path');
  path.setAttribute('d', connectorPath({ ...shape, x: 0, y: 0, flipH: local ? false : shape.flipH, flipV: local ? false : shape.flipV }));
  path.setAttribute('transform', shape.transform ? `matrix(${shape.transform.join(' ')})`
    : `translate(${shape.x} ${shape.y}) rotate(${shape.rotation} ${shape.w / 2} ${shape.h / 2})`);
  path.setAttribute('fill', 'none');
  path.setAttribute('stroke', svgPaint(svg, shape.stroke ?? '#000', shape.strokeGradient, shape.w, shape.h, true));
  path.setAttribute('stroke-width', String(shape.strokeWidthEmu ?? 12700));
  path.setAttribute('stroke-linecap', 'round');
  path.setAttribute('stroke-linejoin', 'round');
  if (shape.shadow) path.style.filter = shadowFilter(shape.shadow, doc);
  const pattern = dashArray(shape.dash, shape.strokeWidthEmu ?? 12700);
  if (pattern) path.setAttribute('stroke-dasharray', pattern);

  const defs = document.createElementNS(SVG_NS, 'defs');
  const id = ++markerId;
  if (shape.headEnd) {
    const marker = createMarker(`ppt-head-${id}`, shape.headEnd, paintEndpoint(shape.stroke, shape.strokeGradient, false), shape.strokeWidthEmu ?? 12700);
    defs.appendChild(marker);
    path.setAttribute('marker-start', `url(#ppt-head-${id})`);
  }
  if (shape.tailEnd) {
    const marker = createMarker(`ppt-tail-${id}`, shape.tailEnd, paintEndpoint(shape.stroke, shape.strokeGradient, true), shape.strokeWidthEmu ?? 12700);
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

function createMarker(id: string, type: string, color: string, strokeWidthEmu: number): SVGMarkerElement {
  const marker = document.createElementNS(SVG_NS, 'marker');
  marker.id = id;
  marker.setAttribute('viewBox', '0 0 10 10');
  marker.setAttribute('refX', '9');
  marker.setAttribute('refY', '5');
  marker.setAttribute('markerWidth', String(strokeWidthEmu * 7));
  marker.setAttribute('markerHeight', String(strokeWidthEmu * 7));
  marker.setAttribute('orient', 'auto-start-reverse');
  marker.setAttribute('markerUnits', 'userSpaceOnUse');
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
    p.style.fontSize = `calc(${ptCqw(firstRun.sizePt, doc)} * var(--font-scale, 1))`;
  if (firstRun?.font && !firstRun.font.startsWith('+')) p.style.fontFamily = fontFamily(firstRun.font);
  if (firstRun?.color && !firstRun.color.includes('gradient(')) p.style.color = firstRun.color;
  if (para.marginLeftEmu !== undefined) p.style.marginLeft = cqw(para.marginLeftEmu, doc.slideWidthEmu);
  else if (para.level) p.style.paddingLeft = `${para.level * 1.4}em`;
  if (para.marginRightEmu !== undefined) p.style.marginRight = cqw(para.marginRightEmu, doc.slideWidthEmu);
  if (para.indentEmu !== undefined) p.style.textIndent = cqw(para.indentEmu, doc.slideWidthEmu);
  if (para.align) p.style.textAlign = para.align;
  if (para.lineSpacingPt !== undefined) p.style.lineHeight = ptCqw(para.lineSpacingPt, doc);
  else if (para.lineSpacing !== undefined)
    p.style.lineHeight = `${Math.max(0.1, para.lineSpacing * (1 - lineSpacingReduction))}`;
  if (para.spaceBeforePt !== undefined) p.style.marginTop = ptCqw(para.spaceBeforePt, doc);
  else if (para.spaceBefore !== undefined) p.style.marginTop = `${para.spaceBefore}em`;
  if (para.spaceAfterPt !== undefined) p.style.marginBottom = ptCqw(para.spaceAfterPt, doc);
  else if (para.spaceAfter !== undefined) p.style.marginBottom = `${para.spaceAfter}em`;
  p.style.whiteSpace = wrapText ? 'pre-wrap' : 'pre';
  if (para.bullet) {
    const bullet = document.createElement('span');
    bullet.className = 'shape-bullet';
    bullet.textContent = `${para.bullet} `;
    const bulletFont = para.bulletFont ?? firstRun?.font;
    if (bulletFont && !bulletFont.startsWith('+')) bullet.style.fontFamily = fontFamily(bulletFont);
    if (firstRun?.sizePt)
      bullet.style.fontSize = `calc(${ptCqw(firstRun.sizePt, doc)} * var(--font-scale, 1))`;
    applyTextPaint(bullet, firstRun?.color, firstRun?.colorGradient);
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
    if (run.sizePt) span.style.fontSize = `calc(${ptCqw(run.sizePt, doc)} * var(--font-scale, 1))`;
    applyTextPaint(span, run.color, run.colorGradient);
    if (run.shadow) span.style.textShadow = textShadow(run.shadow, doc);
    p.appendChild(span);
  }
  return p;
}

function renderTable(table: PptTable, doc: PptDoc): HTMLElement {
  const sourceHeight = table.transform ? table.localHeightEmu ?? table.h : table.h;
  table = physicalBox(table);
  const tbl = document.createElement('table');
  tbl.className = 'tbl';
  position(tbl, table, doc);
  tbl.style.fontSize = ptCqw(14, doc);
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
  table.rows.forEach(row => {
    const tr = document.createElement('tr');
    if (row.heightEmu) tr.style.height = pct(row.heightEmu, sourceHeight);
    row.cells.forEach(cell => {
      const td = document.createElement(cell.isHeader ? 'th' : 'td');
      if (cell.paragraphs?.length) {
        for (const paragraph of cell.paragraphs) td.appendChild(renderPara(paragraph, doc, 0, true));
      } else td.textContent = cell.text;
      if (cell.colSpan > 1) td.colSpan = cell.colSpan;
      if (cell.rowSpan > 1) td.rowSpan = cell.rowSpan;
      if (cell.fill) td.style.background = cell.fill;
      if (cell.color) td.style.color = cell.color;
      if (cell.fontSizePt) td.style.fontSize = ptCqw(cell.fontSizePt, doc);
      if (cell.align) td.style.textAlign = cell.align;
      if (cell.verticalAlign) td.style.verticalAlign = cell.verticalAlign === 'center' ? 'middle' : cell.verticalAlign;
      applyWritingMode(td, cell.verticalText);
      for (const side of ['top', 'right', 'bottom', 'left'] as const) {
        const border = cell.borders?.[side];
        td.style.setProperty(`border-${side}`, border && border.widthEmu > 0 && border.color !== 'transparent'
          ? `${cqw(border.widthEmu, doc.slideWidthEmu)} ${cssLineStyle(border.dash)} ${border.color ?? '#000'}` : 'none');
      }
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

function physicalBox<T extends { w: number; h: number; transform?: PptTransform; localWidthEmu?: number; localHeightEmu?: number }>(value: T): T {
  if (!value.transform) return value;
  const [a, b, c, d, e, f] = value.transform;
  const scaleX = Math.hypot(a, b) || 1;
  const scaleY = Math.hypot(c, d) || 1;
  // Group child coordinates can use arbitrary units (the real deck uses 635
  // units of slide EMU per child unit). Absorb that scale into the box only:
  // font sizes, insets, line widths and shadow distances are already physical.
  // Normalized columns retain rotation, reflection and nested-group shear.
  return {
    ...value,
    w: (value.localWidthEmu ?? value.w) * scaleX,
    h: (value.localHeightEmu ?? value.h) * scaleY,
    transform: [a / scaleX, b / scaleX, c / scaleY, d / scaleY, e, f],
  };
}

function position(el: HTMLElement | SVGSVGElement, shape: Pick<PptShape, 'x' | 'y' | 'w' | 'h'> & { transform?: PptTransform; rotation?: number; flipH?: boolean; flipV?: boolean }, doc: PptDoc): void {
  el.style.left = shape.transform ? '0' : pct(shape.x, doc.slideWidthEmu);
  el.style.top = shape.transform ? '0' : pct(shape.y, doc.slideHeightEmu);
  el.style.width = pct(shape.w, doc.slideWidthEmu);
  el.style.height = pct(shape.h, doc.slideHeightEmu);
  if (shape.transform) {
    const [a, b, c, d, e, f] = shape.transform;
    el.style.transformOrigin = '0 0';
    el.style.transform = `translate(${cqw(e, doc.slideWidthEmu)},${cqw(f, doc.slideWidthEmu)}) matrix(${a},${b},${c},${d},0,0)`;
  } else el.style.transform = transform(shape);
}

function transform(shape: { rotation?: number; flipH?: boolean; flipV?: boolean }): string {
  const transforms = [`rotate(${shape.rotation ?? 0}deg)`];
  if (shape.flipH || shape.flipV) transforms.push(`scale(${shape.flipH ? -1 : 1},${shape.flipV ? -1 : 1})`);
  return transforms.join(' ');
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
      styleGeometry(svg, path, shape, doc, customPath.fill, customPath.stroke);
      svg.appendChild(path);
      layer.appendChild(svg);
    }
    box.appendChild(layer);
    return;
  }

  const svg = geometrySvg(100, 100);
  if (shape.shapeType === 'ellipse') {
    const ellipse = document.createElementNS(SVG_NS, 'ellipse');
    ellipse.setAttribute('cx', '50');
    ellipse.setAttribute('cy', '50');
    ellipse.setAttribute('rx', '50');
    ellipse.setAttribute('ry', '50');
    styleGeometry(svg, ellipse, shape, doc, true, true);
    svg.appendChild(ellipse);
  } else {
    const path = document.createElementNS(SVG_NS, 'path');
    path.setAttribute('d', presetPath(shape));
    styleGeometry(svg, path, shape, doc, !/Bracket|Brace/.test(shape.shapeType ?? ''), true);
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
  svg: SVGSVGElement,
  geometry: SVGGeometryElement,
  shape: PptShape,
  doc: PptDoc,
  useFill: boolean,
  useStroke: boolean,
): void {
  geometry.setAttribute('fill', useFill ? svgPaint(svg, shape.fill, shape.fillGradient, shape.w, shape.h) : 'none');
  geometry.setAttribute('stroke', useStroke ? svgPaint(svg, shape.stroke, shape.strokeGradient, shape.w, shape.h) : 'none');
  geometry.style.strokeWidth = cqw(shape.strokeWidthEmu ?? 12700, doc.slideWidthEmu);
  geometry.setAttribute('vector-effect', 'non-scaling-stroke');
  geometry.setAttribute('stroke-linejoin', 'round');
  applyDash(geometry, shape, doc);
}

function applyDash(path: SVGGeometryElement, shape: PptShape, doc: PptDoc): void {
  const pattern = dashArray(shape.dash);
  if (pattern) path.setAttribute('stroke-dasharray', pattern.split(' ')
    .map(value => cqw(Number(value) * (shape.strokeWidthEmu ?? 12700), doc.slideWidthEmu)).join(' '));
}

function pictureClip(shape: PptShape): HTMLElement {
  const clip = document.createElement('div');
  clip.className = 'picture-clip';
  if (shape.shapeType === 'ellipse') {
    clip.style.clipPath = 'ellipse(50% 50% at 50% 50%)';
  } else if (shape.customPaths.length || (shape.shapeType && shape.shapeType !== 'rect')) {
    const svg = document.createElementNS(SVG_NS, 'svg');
    svg.style.position = 'absolute';
    svg.style.width = svg.style.height = '0';
    const defs = document.createElementNS(SVG_NS, 'defs');
    const pathClip = document.createElementNS(SVG_NS, 'clipPath');
    pathClip.id = `ppt-picture-${++markerId}`;
    pathClip.setAttribute('clipPathUnits', 'objectBoundingBox');
    const paths = shape.customPaths.length ? shape.customPaths : [{ data: presetPath(shape), width: 100, height: 100 }];
    for (const item of paths) {
      const path = document.createElementNS(SVG_NS, 'path');
      path.setAttribute('d', item.data);
      path.setAttribute('transform', `scale(${1 / Math.max(1, item.width)} ${1 / Math.max(1, item.height)})`);
      pathClip.appendChild(path);
    }
    defs.appendChild(pathClip); svg.appendChild(defs); clip.appendChild(svg);
    clip.style.clipPath = `url(#${pathClip.id})`;
  }
  return clip;
}

function applyWritingMode(element: HTMLElement, direction?: string): void {
  if (!direction || direction === 'horz') return;
  element.style.writingMode = direction === 'mongolianVert' ? 'vertical-lr' : 'vertical-rl';
  if (direction === 'eaVert' || direction.startsWith('wordArtVert')) element.style.textOrientation = 'upright';
  if (direction === 'vert270') element.style.transform = 'rotate(180deg)';
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

function ptCqw(points: number, doc: PptDoc): string {
  return cqw(points * 12700, doc.slideWidthEmu);
}

function pct(emu: number, total: number): string {
  return `${(emu / total) * 100}%`;
}

function cqw(emu: number, slideWidthEmu: number): string {
  return `${(emu / slideWidthEmu) * 100}cqw`;
}
