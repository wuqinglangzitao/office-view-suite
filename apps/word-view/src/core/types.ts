/** word-view-rs(wasm shim)输出的富样式文档模型 */

export interface WordDoc {
  blocks: WordBlock[];
  setup: WordSectionSetup;
  stats: { paragraphs: number; tables: number; images: number; runs: number };
}

export interface WordBlock {
  kind: 'heading' | 'paragraph' | 'listItem' | 'table' | 'image' | 'pageBreak' | 'sectionBreak';
  level?: number;
  content?: WordInline[];
  align?: 'center' | 'right' | 'justify';
  indentLeftPt?: number;
  indentFirstPt?: number;
  hangingPt?: number;
  linePct?: number;
  spacingBeforePt?: number;
  spacingAfterPt?: number;
  list?: { level: number; ordered: boolean; label?: string; markerFont?: string };
  table?: WordTable;
  src?: string;
  alt?: string;
  widthPx?: number;
  heightPx?: number;
  section?: WordSectionSetup;
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
}

export interface WordTable {
  grid: WordSlot[][];
  headerRows: number;
  colWidthsPct: number[];
  widthPct?: number;
  fixedLayout: boolean;
  indentPt?: number;
  align?: string;
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
  widthPct?: number;
  valign?: string;
  marginTopPt?: number;
  marginRightPt?: number;
  marginBottomPt?: number;
  marginLeftPt?: number;
}
