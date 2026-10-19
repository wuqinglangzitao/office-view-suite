/**
 * Excel 数字格式渲染(常用子集)。
 * 覆盖:百分比 / 千分位+小数 / 科学计数 / 日期序列号 → yyyy-mm-dd;
 * 其余格式原样显示。
 */

const EXCEL_EPOCH = Date.UTC(1899, 11, 30); // Excel 1900 日期系统的原点

function serialToDate(serial: number): Date {
  return new Date(EXCEL_EPOCH + serial * 86400000);
}

function decimalsOf(fmt: string): number {
  const m = fmt.split(';')[0].match(/\.([0#]+)/);
  return m ? m[1].length : 0;
}

export function formatValue(value: string, numfmt?: string): string {
  if (!numfmt) return value;
  // 去掉格式转义符:yyyy\-mm\-dd → yyyy-mm-dd
  const fmt = numfmt.split(';')[0].replace(/\\(.)/g, '$1');
  // 文本占位或纯文本格式
  if (fmt.includes('@')) return value;

  const num = Number(value);
  const isNum = value.trim() !== '' && !Number.isNaN(num);

  if (/yyyy|mm|dd|hh|ss/i.test(fmt) && isNum) {
    const d = serialToDate(num);
    if (!Number.isNaN(d.getTime())) {
      const p2 = (n: number) => String(n).padStart(2, '0');
      return fmt
        .replace(/yyyy/i, String(d.getUTCFullYear()))
        .replace(/mm/i, p2(d.getUTCMonth() + 1))
        .replace(/dd/i, p2(d.getUTCDate()))
        .replace(/hh/i, p2(d.getUTCHours()))
        .replace(/ss/i, p2(d.getUTCSeconds()));
    }
  }

  if (!isNum) return value;

  if (fmt.includes('%')) {
    const dec = decimalsOf(fmt);
    return (num * 100).toFixed(dec) + '%';
  }
  if (/E\+/i.test(fmt)) {
    const dec = decimalsOf(fmt);
    return num.toExponential(dec).toUpperCase();
  }
  if (fmt.includes('#,##')) {
    const dec = decimalsOf(fmt);
    return num.toLocaleString('en-US', {
      minimumFractionDigits: dec,
      maximumFractionDigits: dec,
    });
  }
  if (/^0(\.0+)?$/.test(fmt)) {
    const dec = decimalsOf(fmt);
    return num.toFixed(dec);
  }
  return value;
}
