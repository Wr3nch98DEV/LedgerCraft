// Global app state: session, lookups (accounts, tax rates, tracking...), toasts, dialogs.
import { rpc } from './api';

export const session = $state<{ loaded: boolean; data: any }>({ loaded: false, data: null });

export async function loadSession() {
  try { session.data = await rpc('session.get'); } catch (e) { session.data = { error: String(e) }; }
  session.loaded = true;
  return session.data;
}

export function can(perm: string): boolean {
  const p: string[] = session.data?.perms ?? [];
  return p.includes(perm);
}

export function setting(key: string, fallback = ''): string {
  return session.data?.settings?.[key] ?? fallback;
}

// ---------- Lookups (cached, refreshable) ----------
export const lookups = $state<{ accounts: any[]; taxRates: any[]; tracking: any[]; currencies: any[]; items: any[]; loaded: boolean }>({
  accounts: [], taxRates: [], tracking: [], currencies: [], items: [], loaded: false
});

export async function loadLookups() {
  const [accounts, taxRates, tracking, currencies, items] = await Promise.all([
    rpc('accounts.list'), rpc('taxRates.list'), rpc('tracking.list'), rpc('currencies.list'), rpc('items.list', { q: '' })
  ]);
  Object.assign(lookups, { accounts, taxRates, tracking, currencies, items, loaded: true });
}

export function accountName(id: number | null | undefined) {
  const a = lookups.accounts.find((x) => x.id === id);
  return a ? `${a.code} - ${a.name}` : '';
}

// ---------- Toasts ----------
export type Toast = { id: number; kind: 'ok' | 'error' | 'info'; text: string; action?: { label: string; run: () => void } };
export const toasts = $state<Toast[]>([]);
let tid = 0;
export function toast(text: string, kind: Toast['kind'] = 'ok', action?: Toast['action'], ms = 4500) {
  const t = { id: ++tid, kind, text, action };
  toasts.push(t);
  setTimeout(() => { const i = toasts.findIndex((x) => x.id === t.id); if (i >= 0) toasts.splice(i, 1); }, ms);
}
export function toastError(e: any) { toast(e?.message ?? String(e), 'error', undefined, 7000); }

// ---------- Confirm dialog ----------
export const confirmState = $state<{ open: boolean; title: string; body: string; ok: string; danger: boolean; resolve: ((v: boolean) => void) | null }>({
  open: false, title: '', body: '', ok: 'OK', danger: false, resolve: null
});
export function confirm(title: string, body = '', ok = 'OK', danger = false): Promise<boolean> {
  return new Promise((resolve) => Object.assign(confirmState, { open: true, title, body, ok, danger, resolve }));
}

// ---------- Theme ----------
export const theme = $state({ mode: (document.documentElement.dataset.theme === 'light' ? 'light' : 'dark') as 'dark' | 'light' });
export function toggleTheme() {
  theme.mode = theme.mode === 'dark' ? 'light' : 'dark';
  document.documentElement.dataset.theme = theme.mode;
  try { localStorage.setItem('lc-theme', theme.mode); } catch {}
}

// ---------- Recent items ----------
export const recent = $state<{ items: { label: string; path: string }[] }>({ items: [] });
try { recent.items = JSON.parse(localStorage.getItem('lc-recent') ?? '[]'); } catch {}
export function pushRecent(label: string, path: string) {
  recent.items = [{ label, path }, ...recent.items.filter((r) => r.path !== path)].slice(0, 8);
  try { localStorage.setItem('lc-recent', JSON.stringify(recent.items)); } catch {}
}

// ---------- Global UI ----------
export const ui = $state({ palette: false, notifications: false, newMenu: false, help: false });
