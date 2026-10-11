// Minimal history-API router.

export const route = $state({ path: location.pathname, query: new URLSearchParams(location.search) });

const guards: (() => boolean)[] = [];
/** Register a guard returning false to block navigation (e.g. unsaved changes). Returns an unregister fn. */
export function addGuard(fn: () => boolean) { guards.push(fn); return () => { const i = guards.indexOf(fn); if (i >= 0) guards.splice(i, 1); }; }

export function navigate(to: string, opts: { replace?: boolean } = {}) {
  if (guards.some((g) => !g())) return;
  const url = new URL(to, location.origin);
  if (opts.replace) history.replaceState({}, '', url); else history.pushState({}, '', url);
  route.path = url.pathname;
  route.query = url.searchParams;
  window.scrollTo(0, 0);
}

export function setQuery(key: string, value: string | null) {
  const q = new URLSearchParams(location.search);
  if (value === null || value === '') q.delete(key); else q.set(key, value);
  const s = q.toString();
  history.replaceState({}, '', location.pathname + (s ? '?' + s : ''));
  route.query = q;
}

window.addEventListener('popstate', () => {
  route.path = location.pathname;
  route.query = new URLSearchParams(location.search);
});

/** Intercept clicks on internal <a href> links. */
document.addEventListener('click', (e) => {
  const a = (e.target as HTMLElement).closest('a');
  if (!a || a.target === '_blank' || a.hasAttribute('download') || e.ctrlKey || e.metaKey || e.shiftKey || e.button !== 0) return;
  const href = a.getAttribute('href');
  if (!href || !href.startsWith('/') || href.startsWith('/view/') || href.startsWith('/online/') || href.startsWith('/download/') || href.startsWith('/api/')) return;
  e.preventDefault();
  navigate(href);
});

/** Match a pattern like /sales/invoices/:id. Returns params or null. */
export function match(pattern: string, path = route.path): Record<string, string> | null {
  const p = pattern.split('/').filter(Boolean);
  const s = path.split('/').filter(Boolean);
  if (p.length !== s.length) return null;
  const out: Record<string, string> = {};
  for (let i = 0; i < p.length; i++) {
    if (p[i].startsWith(':')) out[p[i].slice(1)] = decodeURIComponent(s[i]);
    else if (p[i] !== s[i]) return null;
  }
  return out;
}
