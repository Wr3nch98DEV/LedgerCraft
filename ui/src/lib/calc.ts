// Client-side mirror of the server's line maths (the server is authoritative).
import { lookups } from './stores.svelte';
import { parseMoney } from './format';

function rnd(n: number, d: number) { const q = n / d; return q >= 0 ? Math.floor(q + 0.5) : -Math.floor(-q + 0.5); }

export function calcLine(l: any, lat: string) {
  const q = Number(String(l.quantity ?? '1').replace(/,/g, '')) || 0;
  const hasPrice = l.unit_price !== undefined && l.unit_price !== null && String(l.unit_price).trim() !== '';
  const p = hasPrice ? Number(String(l.unit_price).replace(/,/g, '')) || 0 : null;
  const disc = Number(l.discount || 0) || 0;
  const la = p === null ? (parseMoney(l.amount ?? '') ?? 0) : Math.round(q * p * 100 * (1 - disc / 100));
  const tr = lookups.taxRates.find((t) => t.id === l.tax_rate_id);
  const rate = lat === 'NOTAX' || !tr ? 0 : tr.rate_bp;
  let tax = 0, net = la;
  if (lat === 'EXCLUSIVE') tax = rnd(la * rate, 10000);
  else if (lat === 'INCLUSIVE') { tax = rnd(la * rate, 10000 + rate); net = la - tax; }
  return { la, tax, net, rc: tr?.report_type === 'REVERSECHARGE', rateName: tr?.name ?? '' };
}

export function totals(lines: any[], lat: string) {
  let sub = 0, tax = 0;
  const byRate: Record<string, number> = {};
  for (const l of lines) {
    const r = calcLine(l, lat);
    sub += r.net;
    if (!r.rc) tax += r.tax;
    if (r.tax) byRate[r.rateName] = (byRate[r.rateName] ?? 0) + r.tax;
  }
  return { subtotal: sub, tax, total: sub + tax, byRate };
}
