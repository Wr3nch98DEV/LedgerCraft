<script lang="ts">
  // Ctrl/Cmd+K command palette: navigation commands, global search, recent items.
  import { ui, recent, toggleTheme } from '../lib/stores.svelte';
  import { navigate } from '../lib/router.svelte';
  import { rpc } from '../lib/api';
  import { docPath } from '../lib/format';
  import Icon from './Icon.svelte';

  const commands = [
    ['Dashboard', '/'], ['New invoice', '/sales/invoices/new'], ['New bill', '/purchases/bills/new'], ['New quote', '/sales/quotes/new'],
    ['New purchase order', '/purchases/orders/new'], ['New credit note', '/sales/credit-notes/new'], ['New contact', '/contacts/new'],
    ['Spend money', '/banking/spend'], ['Receive money', '/banking/receive'], ['Transfer money', '/banking/transfer'], ['New manual journal', '/accounting/journals/new'],
    ['Invoices', '/sales/invoices'], ['Bills', '/purchases/bills'], ['Quotes', '/sales/quotes'], ['Purchase orders', '/purchases/orders'],
    ['Contacts', '/contacts'], ['Bank accounts', '/banking'], ['Reconcile', '/banking'], ['Bank rules', '/banking/rules'], ['Reports', '/reports'],
    ['Profit and Loss', '/reports/profit-and-loss'], ['Balance Sheet', '/reports/balance-sheet'], ['Trial Balance', '/reports/trial-balance'],
    ['Aged Receivables', '/reports/aged-receivables'], ['Aged Payables', '/reports/aged-payables'], ['Cash Summary', '/reports/cash-summary'],
    ['Chart of accounts', '/accounting/chart'], ['Tax rates', '/accounting/tax-rates'], ['Manual journals', '/accounting/journals'],
    ['Find & recode', '/accounting/recode'], ['Budget manager', '/accounting/budgets'], ['Fixed assets', '/accounting/assets'],
    ['Tax returns', '/accounting/tax-returns'], ['Currencies', '/accounting/currencies'], ['Products & services', '/products'],
    ['Expense claims', '/expenses'], ['Projects', '/projects'], ['Payroll', '/payroll'], ['Employees', '/payroll/employees'], ['Pay runs', '/payroll/pay-runs'],
    ['Settings', '/settings'], ['Users', '/settings/users'], ['Lock dates', '/settings/financial'], ['Assurance dashboard', '/accounting/assurance'],
    ['Import & export', '/settings/import'], ['Cash flow forecast', '/cashflow'], ['Business snapshot', '/snapshot']
  ].map(([label, href]) => ({ label, href }));

  let q = $state('');
  let results = $state<any[]>([]);
  let hi = $state(0);
  let input: HTMLInputElement | undefined = $state();
  let timer: any;

  const cmdMatches = $derived(q ? commands.filter((c) => c.label.toLowerCase().includes(q.toLowerCase())).slice(0, 6) : []);
  const items = $derived([
    ...cmdMatches.map((c) => ({ kind: 'Go to', label: c.label, href: c.href })),
    ...results.map((r) => ({ kind: r.entity, label: r.title, href: hrefFor(r), sub: r.snippet })),
    ...(!q ? recent.items.map((r) => ({ kind: 'Recent', label: r.label, href: r.path })) : [])
  ]);

  function hrefFor(r: any) {
    switch (r.entity) {
      case 'contact': return `/contacts/${r.id}`;
      case 'document': return `/doc/${r.id}`;
      case 'account': return `/reports/account-transactions?account_id=${r.id}`;
      case 'item': return `/products/${r.id}`;
      case 'banktx': return `/banking/transaction/${r.id}`;
      case 'manual_journal': return `/accounting/journals/${r.id}`;
      default: return '/';
    }
  }

  $effect(() => {
    const t = q;
    clearTimeout(timer);
    if (!t.trim()) { results = []; return; }
    timer = setTimeout(async () => { try { results = await rpc('search', { q: t, limit: 12 }); } catch { results = []; } hi = 0; }, 120);
  });
  $effect(() => { if (ui.palette) { q = ''; results = []; hi = 0; setTimeout(() => input?.focus(), 10); } });

  function go(it: any) { ui.palette = false; navigate(it.href); }
  function key(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); ui.palette = !ui.palette; return; }
    if (!ui.palette) {
      const tag = (e.target as HTMLElement).tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return;
      if (e.key === '/') { e.preventDefault(); ui.palette = true; }
      if (e.altKey && e.key.toLowerCase() === 't') { toggleTheme(); }
      return;
    }
    if (e.key === 'Escape') ui.palette = false;
    else if (e.key === 'ArrowDown') { hi = Math.min(hi + 1, items.length - 1); e.preventDefault(); }
    else if (e.key === 'ArrowUp') { hi = Math.max(hi - 1, 0); e.preventDefault(); }
    else if (e.key === 'Enter' && items[hi]) { e.preventDefault(); go(items[hi]); }
  }
</script>

<svelte:window onkeydown={key} />
{#if ui.palette}
  <div class="no-print fixed inset-0 z-[55] flex items-start justify-center bg-black/55 p-4 pt-[12vh]" role="presentation" onmousedown={(e) => { if (e.target === e.currentTarget) ui.palette = false; }}>
    <div class="card w-full max-w-xl overflow-hidden shadow-[var(--shadow)]" role="dialog" aria-label="Search">
      <div class="flex items-center gap-2 border-b border-line px-4">
        <Icon name="search" class="text-muted" />
        <input bind:this={input} bind:value={q} class="h-12 flex-1 bg-transparent text-[15px] outline-none" placeholder="Search contacts, invoices, amounts… or jump to a page" />
        <kbd class="rounded border border-line px-1.5 text-[11px] text-faint">Esc</kbd>
      </div>
      <div class="max-h-[50vh] overflow-y-auto py-1">
        {#each items as it, i}
          <button class="flex w-full items-center gap-3 px-4 py-2 text-left {i === hi ? 'bg-hover' : ''}" onmouseenter={() => (hi = i)} onclick={() => go(it)}>
            <span class="w-20 shrink-0 text-[11px] font-semibold uppercase tracking-wide text-faint">{it.kind.replace('_', ' ')}</span>
            <span class="min-w-0 flex-1"><span class="block truncate">{it.label}</span>{#if it.sub}<span class="block truncate text-[12px] text-muted">{it.sub}</span>{/if}</span>
          </button>
        {:else}
          <div class="px-4 py-6 text-center text-muted">{q ? 'No results' : 'Type to search. Tip: press / anywhere, Alt+T toggles light/dark.'}</div>
        {/each}
      </div>
    </div>
  </div>
{/if}
