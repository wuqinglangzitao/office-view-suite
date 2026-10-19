/** excel-view-rs(wasm shim)输出的文档模型 */

export interface ExcelDoc {
  sheets: ExcelSheet[];
}

export interface ExcelSheet {
  name: string;
  highestCol: number;
  highestRow: number;
  merges: string[];
  colWidths: { col: number; width: number }[];
  rowHeights: { row: number; height: number }[];
  cells: ExcelCell[];
  images: ExcelImage[];
  shapes: ExcelShape[];
  frozenColumns: number;
  frozenRows: number;
}

export interface ExcelCell {
  col: number;
  row: number;
  value: string;
  formula?: string;
  fontName?: string;
  fontSize?: number;
  bold: boolean;
  italic: boolean;
  underline: boolean;
  fontColor?: string;
  fill?: string;
  hAlign?: string;
  vAlign?: string;
  wrapText?: boolean;
  borderLeft?: string;
  borderRight?: string;
  borderTop?: string;
  borderBottom?: string;
  numfmt?: string;
  richText?: ExcelRichTextRun[];
}

export interface ExcelRichTextRun {
  text: string;
  fontName?: string;
  fontSize?: number;
  bold?: boolean;
  italic?: boolean;
  underline?: boolean;
  fontColor?: string;
}

export interface ExcelImage {
  col: number;
  row: number;
  toCol?: number;
  toRow?: number;
  widthPx?: number;
  heightPx?: number;
  data: string;
  name: string;
  fitCell: boolean;
}

export interface ExcelShape {
  col: number;
  row: number;
  toCol?: number;
  toRow?: number;
  widthPx?: number;
  heightPx?: number;
  offsetXPx: number;
  offsetYPx: number;
  name: string;
  geometry: string;
  fill?: string;
  lineColor?: string;
  lineWidthPx?: number;
  paragraphs: { runs: ExcelRichTextRun[] }[];
}
