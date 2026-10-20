import type { PptGradient } from './types';

const SVG_NS = 'http://www.w3.org/2000/svg';
let paintId = 0;

/** Accept older CSS-string models while current parsers preserve structured stops. */
function parseGradient(value?: string): PptGradient | undefined {
  if (!value?.startsWith('linear-gradient(')) return undefined;
  const angle = Number(value.match(/^linear-gradient\(([-\d.]+)deg/)?.[1] ?? 180);
  const stops = [...value.matchAll(/(#[\da-f]{3,8}|rgba?\([^)]*\))\s+([-\d.]+)%/gi)]
    .map(match => ({ color: match[1], position: Number(match[2]) / 100 }));
  return stops.length ? { type: 'linear', angle, stops } : undefined;
}

export function gradientCss(gradient: PptGradient): string {
  const stops = gradient.stops.map(stop => `${stop.color} ${stop.position * 100}%`).join(',');
  return gradient.type === 'radial'
    ? `radial-gradient(ellipse at ${(gradient.centerX ?? 0.5) * 100}% ${(gradient.centerY ?? 0.5) * 100}%,${stops})`
    : `linear-gradient(${gradient.angle}deg,${stops})`;
}

export function applyTextPaint(element: HTMLElement, color?: string, gradient?: PptGradient): void {
  const resolved = gradient ?? parseGradient(color);
  if (resolved) {
    element.style.backgroundImage = gradientCss(resolved);
    element.style.backgroundClip = 'text';
    element.style.setProperty('-webkit-background-clip', 'text');
    element.style.color = 'transparent';
    element.style.boxDecorationBreak = 'clone';
  } else if (color) {
    element.style.color = color;
  }
}

export function paintEndpoint(color: string | undefined, gradient: PptGradient | undefined, end: boolean): string {
  const resolved = gradient ?? parseGradient(color);
  const stop = end ? resolved?.stops.at(-1) : resolved?.stops[0];
  return stop?.color ?? color ?? '#000';
}

export function svgPaint(
  svg: SVGSVGElement,
  color: string | undefined,
  gradient: PptGradient | undefined,
  width: number,
  height: number,
  userSpace = false,
): string {
  const resolved = gradient ?? parseGradient(color);
  if (!resolved) return !color || color === 'transparent' ? 'none' : color;
  if (!resolved.stops.length) return 'none';
  const id = `ppt-paint-${++paintId}`;
  const paint = document.createElementNS(SVG_NS, resolved.type === 'radial' ? 'radialGradient' : 'linearGradient');
  paint.id = id;
  // userSpaceOnUse is necessary for horizontal/vertical connectors, whose
  // bounding box has zero area and cannot host an objectBoundingBox gradient.
  paint.setAttribute('gradientUnits', userSpace ? 'userSpaceOnUse' : 'objectBoundingBox');
  const w = Math.max(1, Math.abs(width)), h = Math.max(1, Math.abs(height));
  if (resolved.type === 'radial') {
    paint.setAttribute('cx', String((resolved.centerX ?? 0.5) * (userSpace ? w : 1)));
    paint.setAttribute('cy', String((resolved.centerY ?? 0.5) * (userSpace ? h : 1)));
    paint.setAttribute('r', String(userSpace ? Math.max(w, h) / 2 : 0.5));
  } else {
    const angle = resolved.angle * Math.PI / 180;
    const dx = Math.sin(angle), dy = -Math.cos(angle);
    const extent = Math.abs(w * dx) + Math.abs(h * dy);
    for (const [key, value] of Object.entries({
      x1: 0.5 - dx * extent / (2 * w), y1: 0.5 - dy * extent / (2 * h),
      x2: 0.5 + dx * extent / (2 * w), y2: 0.5 + dy * extent / (2 * h),
    })) paint.setAttribute(key, String(value * (userSpace ? (key.startsWith('x') ? w : h) : 1)));
  }
  for (const stop of resolved.stops) {
    const node = document.createElementNS(SVG_NS, 'stop');
    node.setAttribute('offset', String(Math.min(1, Math.max(0, stop.position))));
    node.setAttribute('stop-color', stop.color);
    paint.appendChild(node);
  }
  let defs = svg.querySelector('defs');
  if (!defs) { defs = document.createElementNS(SVG_NS, 'defs'); svg.prepend(defs); }
  defs.appendChild(paint);
  return `url(#${id})`;
}

export function dashArray(dash?: string, width = 1): string | undefined {
  const patterns: Record<string, number[]> = {
    dot: [1, 3], sysDot: [1, 1], dash: [4, 3], sysDash: [3, 1],
    lgDash: [8, 3], dashDot: [4, 3, 1, 3], sysDashDot: [3, 1, 1, 1],
    lgDashDot: [8, 3, 1, 3], lgDashDotDot: [8, 3, 1, 3, 1, 3],
    sysDashDotDot: [3, 1, 1, 1, 1, 1],
  };
  return dash && patterns[dash]?.map(value => value * width).join(' ');
}

export function cssLineStyle(dash?: string): string {
  if (!dash || dash === 'solid') return 'solid';
  return dash === 'dot' || dash === 'sysDot' ? 'dotted' : 'dashed';
}
