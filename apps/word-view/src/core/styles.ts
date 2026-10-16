/** word-view 渲染样式:注入 Shadow DOM,与宿主隔离 */
export const SHADOW_CSS = `
:host { display: block; }

.doc {
  min-width: min-content;
  margin: 0 auto;
  padding: 24px 20px 96px;
  font: 12pt/1.5 "Times New Roman", "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", serif;
  color: #111418;
  word-break: break-word;
}

.page {
  position: relative;
  box-sizing: border-box;
  margin: 0 auto 24px;
  background: #fff;
  box-shadow: 0 2px 12px rgba(0,0,0,.16);
  overflow: hidden;
}
.page-body { position: absolute; overflow: hidden; }
.page-header, .page-footer { position: absolute; min-height: 14pt; font-size: 9pt; color: #4c535a; }
.page-header p, .page-footer p { margin: 0; }
.page-footer { text-align: center; }
.page-number { display: block; text-align: center; }
.oversize-block { max-height: calc(100% - 1.6em); overflow: auto; }

h1, h2, h3, h4, h5, h6 { line-height: 1.3; margin: 0.9em 0 0.4em; font-weight: 700; }
h1 { font-size: 1.6em; } h2 { font-size: 1.35em; } h3 { font-size: 1.18em; }
h4, h5, h6 { font-size: 1.05em; }
p { margin: 0.35em 0; white-space: pre-wrap; }
p:empty { min-height: 1.5em; }
li { white-space: pre-wrap; }

.b { font-weight: 700; }
.i { font-style: italic; }
.u { text-decoration: underline; }
.s { text-decoration: line-through; }
.u.s { text-decoration: underline line-through; }

a { color: #0969da; }

.img-wrap { margin: 0.6em 0; }
img { max-width: 100%; height: auto; }

.list-run { margin: 0.4em 0; }
.list-run ul { margin: 0.25em 0; padding-left: 1.6em; }
/* Word 多级编号:嵌套 ol 用 CSS counters 生成 1 / 1.1 / 1.1.1,级别缩进 21pt/级 */
.list-run ol { margin: 0.25em 0; padding-left: 0; list-style: none; counter-reset: item; }
.list-run ol > li { counter-increment: item; }
.list-run ol > li::before { content: counters(item, ".") " "; color: #57606a; }
/* 文档真实序号(rwml label:如 "1)"、"(一)"、"[b]")优先于计数器 */
.list-run li[data-marker]::before {
  content: attr(data-marker) " ";
  font-family: var(--marker-font-family, inherit);
  font-style: normal;
  font-weight: normal;
}
li { margin: 0.15em 0; }
/* 真实序号(文档 marker):inline 流内渲染,不与文字重叠 */
li[data-marker] { list-style: none; }

.tbl-wrap { overflow-x: auto; margin: 0.8em 0; }
.tbl-wrap + .tbl-wrap { margin-top: 0; }
table { border-collapse: collapse; }
th, td {
  border: 1px solid #9aa3ab;
  padding: 4px 8px;
  text-align: left;
  vertical-align: top;
}
th { background: #f0f2f4; font-weight: 600; }

@media print {
  .doc { padding: 0; }
  .page { margin: 0; box-shadow: none; break-after: page; }
}
`;
