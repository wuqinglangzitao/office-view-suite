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
  rotation: number;
  flipH: boolean;
  flipV: boolean;
  shapeType?: string;
  fill?: string;
  stroke?: string;
  strokeWidthEmu?: number;
  dash?: string;
  shadow?: PptShadow;
  headEnd?: string;
  tailEnd?: string;
  adjustments: number[];
  customPaths: PptCustomPath[];
  image?: string;
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
  /** ppt-rs 0.2.36 将 u="none" 读为 true(已知 bug),渲染层不使用 */
  underline: boolean;
  sizePt?: number;
  color?: string;
  shadow?: PptShadow;
}

export interface PptTable {
  x: number;
  y: number;
  w: number;
  h: number;
  columns: number[];
  rows: PptTableRow[];
}

export interface PptTableRow {
  heightEmu: number;
  cells: PptTableCell[];
}

export interface PptTableCell {
  text: string;
  colSpan: number;
  rowSpan: number;
  fill?: string;
  color?: string;
  fontSizePt?: number;
  align?: 'left' | 'center' | 'right' | 'justify';
  marginLeftEmu: number;
  marginRightEmu: number;
  marginTopEmu: number;
  marginBottomEmu: number;
}
