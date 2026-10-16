/** ppt-view 渲染样式:注入 Shadow DOM */
export const SHADOW_CSS = `
:host { display: block; }

.deck { display: flex; flex-direction: column; gap: 24px; }

.slide {
  position: relative;
  width: 100%;
  container-type: inline-size;
  background: #fff;
  border-radius: 0;
  box-shadow: 0 2px 10px rgba(0,0,0,.12);
  overflow: hidden;
}

.shape {
  position: absolute;
  display: flex;
  flex-direction: column;
  overflow: visible;
  box-sizing: border-box;
  transform-origin: center;
}
.shape-geometry-layer { position: absolute; inset: 0; width: 100%; height: 100%; pointer-events: none; }
.shape-geometry { position: absolute; inset: 0; width: 100%; height: 100%; overflow: visible; pointer-events: none; }
.shape-text { position: relative; z-index: 1; width: 100%; min-width: 0; box-sizing: border-box; overflow: hidden; }
.shape p { margin: 0; font-size: calc(1.875cqw * var(--font-scale, 1)); line-height: var(--line-height, 1); white-space: pre-wrap; }
.shape-autofit-resize .shape-text { overflow: visible; }
.picture { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: fill; pointer-events: none; }
.connector { overflow: visible; pointer-events: none; }

.tbl {
  position: absolute;
  border-collapse: collapse;
  table-layout: fixed;
  font-size: 1.458cqw;
  line-height: 1;
  background: #fff;
}
.tbl th, .tbl td { border: 1px solid #7d8790; padding: 0; white-space: pre-wrap; overflow: hidden; }
.tbl th { background: #eef1f4; font-weight: 600; }

@media print {
  .deck { gap: 12px; }
}
`;
