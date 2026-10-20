/** Read paginated tables in source cell order, joining their page fragments. */
export function collectWordFlowContent(root = document.querySelector('#host').shadowRoot) {
  const emitted = new Set();
  const cells = [];
  const read = node => {
    if (node.nodeType === 3) return node.textContent;
    if (!node.matches?.('.tbl-wrap[data-word-table-id]')) return Array.from(node.childNodes, read).join('');
    const id = node.dataset.wordTableId;
    if (emitted.has(id)) return '';
    emitted.add(id);
    const rows = new Map();
    for (const fragment of root.querySelectorAll(`.tbl-wrap[data-word-table-id="${id}"]`)) {
      const table = fragment.querySelector(':scope > table');
      for (const row of table.rows) {
        const rowIndex = Number(row.dataset.wordRow);
        if (!rows.has(rowIndex)) rows.set(rowIndex, new Map());
        const columns = rows.get(rowIndex);
        for (const cell of row.cells) {
          const column = Number(cell.dataset.wordColumn);
          // A repeated table header is a second visual instance of the same cell.
          if (row.parentElement.tagName === 'THEAD' && columns.has(column)) continue;
          columns.set(column, (columns.get(column) ?? '') + Array.from(cell.childNodes, read).join(''));
        }
      }
    }
    const ordered = [...rows.entries()].sort(([left], [right]) => left - right)
      .flatMap(([, columns]) => [...columns.entries()].sort(([left], [right]) => left - right).map(([, text]) => text));
    cells.push(...ordered);
    return ordered.join('');
  };
  const text = Array.from(root.querySelectorAll('.page-body'), read).join('');
  return { text, cells };
}
