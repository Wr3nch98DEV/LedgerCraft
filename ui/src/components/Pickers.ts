// Option builders for Combo.
import { lookups } from '../lib/stores.svelte';

const CLASS_GROUP: Record<string, string> = { ASSET: 'Assets', LIABILITY: 'Liabilities', EQUITY: 'Equity', REVENUE: 'Revenue', EXPENSE: 'Expenses' };

export function accountOptions(filter?: (a: any) => boolean) {
  return lookups.accounts
    .filter((a) => a.status === 'ACTIVE' && (!filter || filter(a)))
    .map((a) => ({ value: a.id, label: `${a.code} - ${a.name}`, group: a.type === 'BANK' ? 'Bank' : CLASS_GROUP[a.class] }));
}
export const salesAccounts = () => accountOptions((a) => a.class === 'REVENUE' || a.class === 'LIABILITY' || a.class === 'ASSET' && a.type !== 'BANK' || a.class === 'EQUITY');
export const purchaseAccounts = () => accountOptions((a) => a.type !== 'BANK');
export const bankAccounts = () => accountOptions((a) => a.type === 'BANK');
export const nonBankAccounts = () => accountOptions((a) => a.type !== 'BANK');

export function taxOptions() {
  return lookups.taxRates.filter((t) => t.status === 'ACTIVE').map((t) => ({ value: t.id, label: t.name, sub: (t.rate_bp / 100).toString() + '%' }));
}

export function trackingCategories() {
  return lookups.tracking.filter((c) => c.status === 'ACTIVE');
}
export function trackingOptions(cat: any) {
  return (cat?.options ?? []).filter((o: any) => o.status === 'ACTIVE').map((o: any) => ({ value: o.id, label: o.name }));
}

export function itemOptions(sales = true) {
  return lookups.items.filter((i) => i.status === 'ACTIVE' && (sales ? i.is_sold : i.is_purchased)).map((i) => ({ value: i.id, label: i.code, sub: i.name }));
}

export function currencyOptions() {
  return lookups.currencies.filter((c) => c.enabled).map((c) => ({ value: c.code, label: c.code, sub: c.name }));
}
