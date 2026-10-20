/** word-view-rs(wasm shim)输出的富样式文档模型 */

export interface WordDoc {
  blocks: WordBlock[];
  setup: WordSectionSetup;
  stats: { paragraphs: number; tables: number; images: number; runs: number };
  defaultTabStopPt?: number;
  typography?: WordTypography;
}

export interface WordTypography {
  balanceSingleByteDoubleByteWidth: boolean;
  underlineTrailingSpaces: boolean;
  compressPunctuation: boolean;
}

export interface WordBlock {
  kind: 'heading' | 'paragraph' | 'listItem' | 'table' | 'image' | 'chart' | 'pageBreak' | 'columnBreak' | 'sectionBreak';
  level?: number;
  content?: WordInline[];
  align?: 'center' | 'right' | 'justify';
  indentLeftPt?: number;
  indentRightPt?: number;
  indentFirstPt?: number;
  hangingPt?: number;
  linePct?: number;
  spacingBeforePt?: number;
  spacingAfterPt?: number;
  pageBreakBefore?: boolean;
  pagination?: { keepNext: boolean; keepLines: boolean; widowControl: boolean };
  lineSpacing?: { rule: 'exact' | 'atLeast'; valuePt: number };
  tabStops?: WordTabStop[];
  list?: { level: number; ordered: boolean; label?: string; markerFont?: string };
  table?: WordTable;
  src?: string;
  alt?: string;
  widthPx?: number;
  heightPx?: number;
  section?: WordSectionSetup;
  shading?: string;
  bidi?: boolean;
  rotationDegrees?: number;
  chart?: WordChart;
  columnBreakOffsets?: number[];
}

export interface WordChart {
  kind: string;
  title?: string;
  alt?: string;
  categories: string[];
  series: { name: string; values: number[]; bubbleSizes?: number[] }[];
  widthPx?: number;
  heightPx?: number;
  wireframe?: boolean;
}

export interface WordTabStop {
  positionPt: number;
  alignment: 'left' | 'center' | 'right' | 'decimal' | 'bar' | 'clear';
  leader: 'none' | 'dot' | 'hyphen' | 'underscore' | 'heavy' | 'middleDot';
}

export interface WordPageSetup {
  widthPt: number;
  heightPt: number;
  leftPt: number;
  rightPt: number;
  topPt: number;
  bottomPt: number;
  landscape: boolean;
}

export interface WordSectionSetup {
  page: WordPageSetup;
  header: WordBlock[];
  firstHeader: WordBlock[];
  evenHeader: WordBlock[];
  footer: WordBlock[];
  firstFooter: WordBlock[];
  evenFooter: WordBlock[];
  titlePage: boolean;
  pageNumbers: boolean;
  linePitchPt?: number;
  /** Start behavior for the section following this boundary. */
  breakType?: 'continuous' | 'nextPage' | 'evenPage' | 'oddPage' | 'nextColumn';
  headerDistancePt?: number;
  footerDistancePt?: number;
  pageNumberStart?: number;
  pageNumberFormat?: string;
  columns?: number;
  columnGapPt?: number;
  columnWidthsPt?: number[];
  columnSpacesPt?: number[];
  columnSeparator?: boolean;
  columnsRtl?: boolean;
  textDirection?: string;
}

export interface WordInline {
  kind: 'text' | 'link' | 'image';
  text?: string;
  font?: string;
  sizeHalfPt?: number;
  color?: string;
  highlight?: string;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  strike?: boolean;
  vertAlign?: 'super' | 'sub';
  href?: string;
  src?: string;
  alt?: string;
  widthPx?: number;
  heightPx?: number;
  caps?: boolean;
  smallCaps?: boolean;
  rtl?: boolean;
  rotationDegrees?: number;
  field?: { kind: 'PAGE' | 'NUMPAGES' | 'SECTIONPAGES'; format?: string };
  anchor?: {
    horizontalFrom: 'page' | 'margin' | 'column';
    verticalFrom: 'page' | 'margin' | 'paragraph';
    xPt: number;
    yPt: number;
  };
}

export interface WordTable {
  grid: WordSlot[][];
  headerRows: number;
  colWidthsPct: number[];
  widthPct?: number;
  fixedLayout: boolean;
  indentPt?: number;
  align?: string;
  bidiVisual?: boolean;
  rowCantSplit?: boolean[];
  rowMinHeightsPt?: (number | null)[];
}

export interface WordSlot {
  kind: 'origin' | 'covered';
  cell?: WordCell;
}

export interface WordCell {
  blocks: WordBlock[];
  colSpan: number;
  rowSpan: number;
  isHeader: boolean;
  shading?: string;
  borders?: Partial<Record<'top' | 'right' | 'bottom' | 'left', WordBorder>>;
  widthPct?: number;
  valign?: string;
  marginTopPt?: number;
  marginRightPt?: number;
  marginBottomPt?: number;
  marginLeftPt?: number;
}

export interface WordBorder {
  style: 'solid' | 'dashed' | 'dotted' | 'double' | 'none';
  color?: string;
  widthPt?: number;
}
