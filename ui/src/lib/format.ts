// Formatting helpers. Money values from the API are integer cents.

export function money(cents: number | null | undefined, opts: { blankZero?: boolean; sign?: boolean; dp?: number } = {}): string {
  if (cents === null || cents === undefined || Number.isNaN(cents)) return '';
  if (opts.blankZero && cents === 0) return '';
  const dp = opts.dp ?? 2;
  const neg = cents < 0;
  const abs = Math.abs(cents);
  const whole = Math.floor(abs / 100);
  const frac = abs % 100;
  let s = whole.toLocaleString('en-US') + (dp > 0 ? '.' + String(frac).padStart(2, '0') : '');
  if (neg) s = '-' + s; else if (opts.sign && cents > 0) s = '+' + s;
  return s;
}

/** Money for editable inputs: no thousands separators. */
export function moneyPlain(cents: number | null | undefined): string {
  if (cents === null || cents === undefined) return '';
  const neg = cents < 0; const a = Math.abs(cents);
  return (neg ? '-' : '') + Math.floor(a / 100) + '.' + String(a % 100).padStart(2, '0');
}

/** Fixed point scaled by 10^dp → trimmed decimal string. */
export function fixed(v: number | null | undefined, dp: number, minDp = 0): string {
  if (v === null || v === undefined) return '';
  const neg = v < 0; const a = Math.abs(v);
  const p = 10 ** dp;
  let frac = String(a % p).padStart(dp, '0').replace(/0+$/, '');
  while (frac.length < minDp) frac += '0';
  return (neg ? '-' : '') + Math.floor(a / p) + (frac ? '.' + frac : '');
}
export const qty = (e4: number) => fixed(e4, 4);
export const unitPrice = (e2: number) => fixed(e2, 4, 2);
export const pct = (bp: number) => fixed(bp, 2);
export const rate = (e8: number) => fixed(e8, 8, 4);

/** Parse a user-entered amount ("1,234.50", "(12)") into cents, or null. */
export function parseMoney(s: string): number | null {
  if (s === null || s === undefined) return null;
  let t = String(s).trim().replace(/[,$\s]/g, '');
  if (!t) return null;
  let neg = false;
  if (t.startsWith('(') && t.endsWith(')')) { neg = true; t = t.slice(1, -1); }
  if (t.startsWith('-')) { neg = true; t = t.slice(1); }
  if (!/^\d*(\.\d*)?$/.test(t)) return null;
  const [w, f = ''] = t.split('.');
  const cents = Number(w || '0') * 100 + Number((f + '00').slice(0, 2)) + (Number(f[2] ?? 0) >= 5 ? 1 : 0);
  return neg ? -cents : cents;
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
export function date(s: string | null | undefined): string {
  if (!s) return '';
  const [y, m, d] = s.slice(0, 10).split('-').map(Number);
  if (!y || !m || !d) return s;
  return `${d} ${MONTHS[m - 1]} ${y}`;
}
export function today(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}
export function addDays(s: string, n: number): string {
  const d = new Date(s + 'T00:00:00');
  d.setDate(d.getDate() + n);
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}
export function monthStart(s = today()) { return s.slice(0, 8) + '01'; }
export function monthEnd(s = today()) {
  const [y, m] = s.split('-').map(Number);
  const d = new Date(y, m, 0);
  return `${y}-${String(m).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
}
export function addMonths(s: string, n: number) {
  const [y, m, d] = s.split('-').map(Number);
  const t = new Date(y, m - 1 + n, 1);
  const last = new Date(t.getFullYear(), t.getMonth() + 1, 0).getDate();
  return `${t.getFullYear()}-${String(t.getMonth() + 1).padStart(2, '0')}-${String(Math.min(d, last)).padStart(2, '0')}`;
}
export function daysBetween(a: string, b: string) {
  return Math.round((new Date(b + 'T00:00:00').getTime() - new Date(a + 'T00:00:00').getTime()) / 86400000);
}
export function relative(s: string) {
  const n = daysBetween(today(), s);
  if (n === 0) return 'today';
  if (n === 1) return 'tomorrow';
  if (n === -1) return 'yesterday';
  return n > 0 ? `in ${n} days` : `${-n} days ago`;
}
export function dateTime(s: string) {
  if (!s) return '';
  const d = new Date(s.endsWith('Z') || s.includes('T') ? s : s.replace(' ', 'T') + 'Z');
  return d.toLocaleString(undefined, { day: 'numeric', month: 'short', year: 'numeric', hour: 'numeric', minute: '2-digit' });
}

export const DOC_LABEL: Record<string, string> = {
  ACCREC: 'Invoice', ACCPAY: 'Bill', ACCRECCREDIT: 'Credit note', ACCPAYCREDIT: 'Supplier credit', QUOTE: 'Quote', PO: 'Purchase order'
};

export function docPath(type: string, id: number | string) {
  switch (type) {
    case 'ACCREC': return `/sales/invoices/${id}`;
    case 'ACCRECCREDIT': return `/sales/credit-notes/${id}`;
    case 'QUOTE': return `/sales/quotes/${id}`;
    case 'ACCPAY': return `/purchases/bills/${id}`;
    case 'ACCPAYCREDIT': return `/purchases/credit-notes/${id}`;
    case 'PO': return `/purchases/orders/${id}`;
  }
  return '/';
}

export const STATUS_LABEL: Record<string, string> = {
  DRAFT: 'Draft', SUBMITTED: 'Awaiting approval', AUTHORISED: 'Awaiting payment', PAID: 'Paid', VOIDED: 'Voided', DELETED: 'Deleted',
  SENT: 'Sent', ACCEPTED: 'Accepted', DECLINED: 'Declined', INVOICED: 'Invoiced', BILLED: 'Billed', POSTED: 'Posted',
  UNRECONCILED: 'Unreconciled', RECONCILED: 'Reconciled', ACTIVE: 'Active', ARCHIVED: 'Archived'
};
