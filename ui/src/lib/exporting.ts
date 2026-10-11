// Client-side export of report JSON to CSV and Excel (SpreadsheetML 2003, opens in Excel/LibreOffice).

function flat(report: any): string[][] {
  const out: string[][] = [];
  const cols: string[] = report.columns ?? [];
  const head = ['', ...(report.detail ? ['Date', 'Source', 'Reference'] : []), ...cols];
  out.push([report.title]); if (report.org) out.push([report.org]); if (report.subtitle) out.push([report.subtitle]); out.push([]);
  out.push(head);
  const v = (x: any, fmt?: string) => (x === null || x === undefined ? '' : typeof x === 'number' ? (fmt === 'num' ? String(x) : (x / 100).toFixed(2)) : String(x));
  for (const s of report.sections) {
    if (s.title) out.push([s.title]);
    for (const r of s.rows ?? []) {
      out.push([r.label ?? '', ...(report.detail ? [r.date ?? '', r.source_label ?? '', r.reference ?? ''] : []), ...(r.values ?? []).map((x: any) => v(x, r.format))]);
      for (const c of r.children ?? []) out.push(['  ' + c.label, ...(c.values ?? []).map((x: any) => v(x))]);
    }
    if (s.total) out.push([s.total.label, ...(report.detail ? ['', '', ''] : []), ...(s.total.values ?? []).map((x: any) => v(x))]);
  }
  return out;
}

function save(name: string, mime: string, data: string) {
  const a = document.createElement('a');
  a.href = URL.createObjectURL(new Blob([data], { type: mime }));
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 2000);
}

export function exportCsv(report: any) {
  const rows = flat(report).map((r) => r.map((c) => (/[",\n]/.test(c) ? `"${c.replace(/"/g, '""')}"` : c)).join(','));
  save(`${report.title}.csv`, 'text/csv', '﻿' + rows.join('\r\n'));
}

export function exportExcel(report: any) {
  const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  const rows = flat(report).map((r) => '<Row>' + r.map((c, i) => {
    const num = i > 0 && c !== '' && /^-?\d+(\.\d+)?$/.test(c);
    return `<Cell${num ? ' ss:StyleID="n"' : ''}><Data ss:Type="${num ? 'Number' : 'String'}">${esc(c)}</Data></Cell>`;
  }).join('') + '</Row>');
  const xml = `<?xml version="1.0"?><?mso-application progid="Excel.Sheet"?><Workbook xmlns="urn:schemas-microsoft-com:office:spreadsheet" xmlns:ss="urn:schemas-microsoft-com:office:spreadsheet"><Styles><Style ss:ID="n"><NumberFormat ss:Format="#,##0.00"/></Style></Styles><Worksheet ss:Name="Report"><Table>${rows.join('')}</Table></Worksheet></Workbook>`;
  save(`${report.title}.xls`, 'application/vnd.ms-excel', xml);
}

export function downloadText(name: string, text: string, mime = 'text/plain') { save(name, mime, text); }
