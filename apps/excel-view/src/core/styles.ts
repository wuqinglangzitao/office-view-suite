/** excel-view 渲染样式:注入 Shadow DOM */
export const SHADOW_CSS = `
:host { display: block; }

.workbook { display: flex; flex-direction: column; gap: 12px; }
.sheet-tabs { position: sticky; top: 0; z-index: 20; display: flex; gap: 0; overflow-x: auto; padding: 6px 0 0; background: #f6f7f9; border-bottom: 1px solid #c9d0d7; }
.sheet-tab { flex: 0 0 auto; min-height: 34px; padding: 6px 14px; border: 0; border-right: 1px solid #d7dce1; border-bottom: 2px solid transparent; background: #eef1f4; color: #30363d; cursor: pointer; }
.sheet-tab[aria-selected="true"] { background: #fff; border-bottom-color: #217346; color: #111; font-weight: 600; }
.sheet[hidden] { display: none; }
.sheet h2 { font-size: 15px; margin: 0 0 10px; color: #1f2328; }

.grid-wrap { overflow: auto; max-height: 75vh; background: #fff; border: 1px solid #d0d7de; border-radius: 6px; }
.grid { display: grid; gap: 0; width: max-content;
  border-left: 1px solid #e4e8ec; border-top: 1px solid #e4e8ec; }

.cell {
  border-right: 1px solid #e4e8ec;
  border-bottom: 1px solid #e4e8ec;
  padding: 3px 8px;
  font-size: 14px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #111418;
  background: #fff;
}
.frozen-row, .frozen-column { position: sticky; z-index: 5; }
.frozen-corner { z-index: 6; }
.sheet-image { position: relative; z-index: 8; align-self: start; justify-self: start; pointer-events: none; overflow: hidden; }
.sheet-image img { display: block; width: 100%; height: 100%; object-fit: contain; }
.sheet-image-cell { width: 100%; height: 100%; align-self: stretch; justify-self: stretch; padding: 2px; box-sizing: border-box; background: #fff; }
.sheet-shape {
  position: relative;
  z-index: 9;
  box-sizing: border-box;
  align-self: stretch;
  justify-self: stretch;
  min-width: 1px;
  min-height: 1px;
  padding: 4px 6px;
  border: 1px solid transparent;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  pointer-events: none;
  white-space: pre-wrap;
}
.sheet-shape-paragraph { width: 100%; text-align: center; line-height: 1.2; }
.sheet-shape-line { align-self: center; min-height: 0; height: 0 !important; padding: 0; border: 0; border-top: 1px solid #666; background: transparent !important; }

@media print { .grid-wrap { overflow: visible; } }
`;
