/** ppt-view-rs(wasm shim)输出的布局感知文档模型 */

export interface PptDoc {
  slideWidthEmu: number;
  slideHeightEmu: number;
  slides: PptSlide[];
  warnings: string[];
}

export interface PptSlide {
  hidden: boolean;
  shapes: PptShape[];
  tables: PptTable[];
  background?: string;
}

export interface PptShape {
  kind: 'shape' | 'picture' | 'line';
  name: string;
  x: number;
  y: number;
  w: number;
  h: number;
  zOrder?: number;
  /** Maps the local EMU box to slide coordinates, including ancestor transforms. */
  transform?: PptTransform;
  localWidthEmu?: number;
  localHeightEmu?: number;
  rotation: number;
  flipH: boolean;
  flipV: boolean;
  shapeType?: string;
  fill?: string;
  fillGradient?: PptGradient;
  stroke?: string;
  strokeGradient?: PptGradient;
  strokeWidthEmu?: number;
  dash?: string;
  shadow?: PptShadow;
  headEnd?: string;
  tailEnd?: string;
  adjustments: number[];
  customPaths: PptCustomPath[];
  image?: string;
  crop?: { left: number; top: number; right: number; bottom: number };
  paragraphs: PptPara[];
  align?: string;
  verticalAlign?: 'top' | 'center' | 'bottom';
  marginLeftEmu: number;
  marginRightEmu: number;
  marginTopEmu: number;
  marginBottomEmu: number;
  autoFit?: 'shrink' | 'resize';
  wrapText: boolean;
  fontScale: number;
  lineSpacingReduction: number;
  verticalText?: string;
}

export type PptTransform = [number, number, number, number, number, number];

export interface PptGradient {
  type: 'linear' | 'radial';
  angle: number;
  stops: { color: string; position: number }[];
  centerX?: number;
  centerY?: number;
}

export interface PptShadow {
  color: string;
  blurRadiusEmu: number;
  distanceEmu: number;
  directionDeg: number;
}

export interface PptCustomPath {
  width: number;
  height: number;
  data: string;
  fill: boolean;
  stroke: boolean;
}

export interface PptPara {
  level: number;
  align?: 'left' | 'center' | 'right' | 'justify';
  marginLeftEmu?: number;
  marginRightEmu?: number;
  indentEmu?: number;
  lineSpacing?: number;
  lineSpacingPt?: number;
  spaceBefore?: number;
  spaceBeforePt?: number;
  spaceAfter?: number;
  spaceAfterPt?: number;
  bullet?: string;
  bulletFont?: string;
  runs: PptRun[];
}

export interface PptRun {
  text: string;
  bold: boolean;
  font?: string;
  italic: boolean;
  underline: boolean;
  sizePt?: number;
  color?: string;
  colorGradient?: PptGradient;
  shadow?: PptShadow;
}

export interface PptTable {
  x: number;
  y: number;
  w: number;
  h: number;
  zOrder?: number;
  transform?: PptTransform;
  localWidthEmu?: number;
  localHeightEmu?: number;
  rotation?: number;
  flipH?: boolean;
  flipV?: boolean;
  columns: number[];
  rows: PptTableRow[];
}

export interface PptTableRow {
  heightEmu: number;
  cells: PptTableCell[];
}

export interface PptTableCell {
  text: string;
  paragraphs?: PptPara[];
  colSpan: number;
  rowSpan: number;
  fill?: string;
  color?: string;
  fontSizePt?: number;
  align?: 'left' | 'center' | 'right' | 'justify';
  verticalAlign?: 'top' | 'center' | 'bottom';
  verticalText?: string;
  isHeader?: boolean;
  borders?: Partial<Record<'top' | 'right' | 'bottom' | 'left', PptBorder>>;
  marginLeftEmu: number;
  marginRightEmu: number;
  marginTopEmu: number;
  marginBottomEmu: number;
}

export interface PptBorder {
  color?: string;
  widthEmu: number;
  dash?: string;
}
