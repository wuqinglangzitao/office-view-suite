import type { PptShape } from './types';

const clamp = (value: number, max = 100) => Math.min(max, Math.max(0, value));
const polygon = (points: number[][]): string => points.map(([x, y], index) => `${index ? 'L' : 'M'} ${x} ${y}`).join(' ') + ' Z';

function star(points: number, inner: number): string {
  return polygon(Array.from({ length: points * 2 }, (_, index) => {
    const angle = index * Math.PI / points - Math.PI / 2;
    const radius = index % 2 ? inner : 50;
    return [50 + radius * Math.cos(angle), 50 + radius * Math.sin(angle)];
  }));
}

function arrow(shape: PptShape): string {
  const vertical = shape.shapeType === 'upArrow' || shape.shapeType === 'downArrow';
  const width = Math.max(1, vertical ? shape.h : shape.w);
  const height = Math.max(1, vertical ? shape.w : shape.h);
  const shaft = clamp((shape.adjustments?.[0] ?? 50000) / 1000);
  const head = clamp((shape.adjustments?.[1] ?? 50000) / 1000 * Math.min(width, height) / width);
  const y = (100 - shaft) / 2;
  const points = [[0, y], [100 - head, y], [100 - head, 0], [100, 50], [100 - head, 100], [100 - head, 100 - y], [0, 100 - y]];
  return polygon(points.map(([x, py]) => {
    if (shape.shapeType === 'leftArrow') return [100 - x, py];
    if (shape.shapeType === 'upArrow') return [py, 100 - x];
    if (shape.shapeType === 'downArrow') return [100 - py, x];
    return [x, py];
  }));
}

export function roundedRectPath(shape: PptShape): string {
  const width = Math.max(1, Math.abs(shape.w)), height = Math.max(1, Math.abs(shape.h));
  const radius = (index: number, fallback: number) => {
    const value = Math.min(width, height) * clamp(shape.adjustments?.[index] ?? fallback, 50000) / 100000;
    return [value / width * 100, value / height * 100];
  };
  const first = radius(0, 16667), second = radius(1, 0), square = [0, 0];
  const kind = shape.shapeType;
  const [tl, tr, br, bl] = kind === 'round1Rect' ? [square, first, square, square]
    : kind === 'round2SameRect' ? [first, first, second, second]
      : kind === 'round2DiagRect' ? [second, first, second, first] : [first, first, first, first];
  return `M ${tl[0]} 0 H ${100 - tr[0]}`
    + (tr[0] ? ` A ${tr[0]} ${tr[1]} 0 0 1 100 ${tr[1]}` : '')
    + ` V ${100 - br[1]}`
    + (br[0] ? ` A ${br[0]} ${br[1]} 0 0 1 ${100 - br[0]} 100` : '')
    + ` H ${bl[0]}`
    + (bl[0] ? ` A ${bl[0]} ${bl[1]} 0 0 1 0 ${100 - bl[1]}` : '')
    + ` V ${tl[1]}`
    + (tl[0] ? ` A ${tl[0]} ${tl[1]} 0 0 1 ${tl[0]} 0` : '') + ' Z';
}

/** Presets use a normalized box; adjustment distances retain the source aspect ratio. */
export function presetPath(shape: PptShape): string {
  const kind = shape.shapeType;
  const adjustment = (fallback: number, max = 50) => clamp((shape.adjustments?.[0] ?? fallback) / 1000
    * Math.min(Math.abs(shape.w), Math.abs(shape.h)) / Math.max(1, Math.abs(shape.w)), max);
  const starMatch = kind?.match(/^star(4|5|6|7|8|10|12|16|24|32)$/);
  if (starMatch) return star(Number(starMatch[1]), clamp((shape.adjustments?.[0] ?? 19098) / 1000, 50));
  switch (kind) {
    case 'roundRect': case 'round1Rect': case 'round2SameRect': case 'round2DiagRect': return roundedRectPath(shape);
    case 'triangle': return polygon([[50, 0], [100, 100], [0, 100]]);
    case 'rtTriangle': return polygon([[0, 0], [100, 100], [0, 100]]);
    case 'diamond': case 'flowChartDecision': return polygon([[50, 0], [100, 50], [50, 100], [0, 50]]);
    case 'parallelogram': case 'flowChartInputOutput': {
      const skew = adjustment(25000); return polygon([[skew, 0], [100, 0], [100 - skew, 100], [0, 100]]);
    }
    case 'trapezoid': {
      const inset = adjustment(25000); return polygon([[inset, 0], [100 - inset, 0], [100, 100], [0, 100]]);
    }
    case 'pentagon': return polygon([[50, 0], [100, 38.2], [80.9, 100], [19.1, 100], [0, 38.2]]);
    case 'hexagon': case 'flowChartPreparation': {
      const inset = adjustment(25000); return polygon([[inset, 0], [100 - inset, 0], [100, 50], [100 - inset, 100], [inset, 100], [0, 50]]);
    }
    case 'heptagon': case 'decagon': case 'dodecagon': {
      const n = kind === 'heptagon' ? 7 : kind === 'decagon' ? 10 : 12;
      return polygon(Array.from({ length: n }, (_, i) => [50 + 50 * Math.cos(i * 2 * Math.PI / n - Math.PI / 2), 50 + 50 * Math.sin(i * 2 * Math.PI / n - Math.PI / 2)]));
    }
    case 'octagon': {
      const inset = adjustment(29289); return polygon([[inset, 0], [100 - inset, 0], [100, inset], [100, 100 - inset], [100 - inset, 100], [inset, 100], [0, 100 - inset], [0, inset]]);
    }
    case 'chevron': {
      const inset = adjustment(50000); return polygon([[0, 0], [100 - inset, 0], [100, 50], [100 - inset, 100], [0, 100], [inset, 50]]);
    }
    case 'homePlate': {
      const inset = adjustment(50000); return polygon([[0, 0], [100 - inset, 0], [100, 50], [100 - inset, 100], [0, 100]]);
    }
    case 'leftArrow': case 'rightArrow': case 'upArrow': case 'downArrow': return arrow(shape);
    case 'leftRightArrow': {
      const head = clamp((shape.adjustments?.[1] ?? 50000) / 1000 * Math.min(shape.w, shape.h) / Math.max(1, shape.w), 50);
      const y = (100 - clamp((shape.adjustments?.[0] ?? 50000) / 1000)) / 2;
      return polygon([[0, 50], [head, 0], [head, y], [100 - head, y], [100 - head, 0], [100, 50], [100 - head, 100], [100 - head, 100 - y], [head, 100 - y], [head, 100]]);
    }
    case 'upDownArrow': return 'M 50 0 L 80 28 H 65 V 72 H 80 L 50 100 L 20 72 H 35 V 28 H 20 Z';
    case 'leftBracket': return 'M 80 0 H 55 Q 20 0 20 25 V 75 Q 20 100 55 100 H 80';
    case 'rightBracket': return 'M 20 0 H 45 Q 80 0 80 25 V 75 Q 80 100 45 100 H 20';
    case 'leftBrace': return 'M 80 0 Q 50 0 50 20 V 35 Q 50 50 20 50 Q 50 50 50 65 V 80 Q 50 100 80 100';
    case 'rightBrace': return 'M 20 0 Q 50 0 50 20 V 35 Q 50 50 80 50 Q 50 50 50 65 V 80 Q 50 100 20 100';
    case 'plus': case 'mathPlus': return 'M 35 0 H 65 V 35 H 100 V 65 H 65 V 100 H 35 V 65 H 0 V 35 H 35 Z';
    case 'heart': return 'M 50 100 C 40 80 0 60 0 28 C 0 -3 36 -8 50 20 C 64 -8 100 -3 100 28 C 100 60 60 80 50 100 Z';
    case 'flowChartDocument': return 'M 0 0 H 100 V 80 C 65 60 35 100 0 80 Z';
    case 'flowChartTerminator': return 'M 25 0 H 75 A 25 50 0 0 1 75 100 H 25 A 25 50 0 0 1 25 0 Z';
    case 'flowChartPredefinedProcess': return 'M 0 0 H 100 V 100 H 0 Z M 12 0 V 100 M 88 0 V 100';
    case 'flowChartConnector': case 'ellipse': return 'M 0 50 A 50 50 0 1 0 100 50 A 50 50 0 1 0 0 50 Z';
    case 'wedgeRectCallout': {
      const x = 50 + (shape.adjustments?.[0] ?? -20833) / 1000;
      const y = 50 + (shape.adjustments?.[1] ?? 62500) / 1000;
      return `M 0 0 H 100 V 100 H 60 L ${x} ${y} L 40 100 H 0 Z`;
    }
    default: return 'M 0 0 H 100 V 100 H 0 Z';
  }
}
