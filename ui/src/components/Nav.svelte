<script lang="ts">
  import Icon from './Icon.svelte';
  import { route, navigate } from '../lib/router.svelte';
  import { session, ui, theme, toggleTheme, can } from '../lib/stores.svelte';
  import { rpc, setToken } from '../lib/api';

  type Item = { label: string; href?: string; perm?: string; sep?: string };
  const menus: { key: string; label: string; href?: string; items?: Item[]; match: string[] }[] = [
    { key: 'home', label: 'Dashboard', href: '/', match: ['/'] },
    { key: 'business', label: 'Business', match: ['/sales', '/purchases', '/products', '/projects', '/expenses'], items: [
      { label: 'Sales overview', href: '/sales', perm: 'sales:read' },
      { label: 'Invoices', href: '/sales/invoices', perm: 'sales:read' },
      { label: 'Quotes', href: '/sales/quotes', perm: 'sales:read' },
      { label: 'Credit notes', href: '/sales/credit-notes', perm: 'sales:read' },
      { label: 'Repeating invoices', href: '/sales/repeating', perm: 'sales:read' },
      { label: 'Online payments', href: '/settings/payments', perm: 'settings:read' },
      { sep: 'Purchases', label: 'Purchases overview', href: '/purchases', perm: 'purchases:read' },
      { label: 'Bills to pay', href: '/purchases/bills', perm: 'purchases:read' },
      { label: 'Purchase orders', href: '/purchases/orders', perm: 'purchases:read' },
      { label: 'Supplier credits', href: '/purchases/credit-notes', perm: 'purchases:read' },
      { label: 'Batch payments', href: '/purchases/batch', perm: 'banking:read' },
      { label: 'Expense claims', href: '/expenses', perm: 'expenses:read' },
      { sep: 'More', label: 'Products & services', href: '/products', perm: 'inventory:read' },
      { label: 'Projects', href: '/projects', perm: 'projects:read' },
      { label: 'Files inbox', href: '/files' },
    ] },
    { key: 'accounting', label: 'Accounting', match: ['/banking', '/reports', '/accounting'], items: [
      { label: 'Bank accounts', href: '/banking', perm: 'banking:read' },
      { label: 'Bank rules', href: '/banking/rules', perm: 'banking:read' },
      { label: 'Reports', href: '/reports', perm: 'reports:read' },
      { label: 'Tax returns', href: '/accounting/tax-returns', perm: 'reports:read' },
      { sep: 'Advanced', label: 'Chart of accounts', href: '/accounting/chart', perm: 'accounting:read' },
      { label: 'Manual journals', href: '/accounting/journals', perm: 'accounting:read' },
      { label: 'Fixed assets', href: '/accounting/assets', perm: 'assets:read' },
      { label: 'Find & recode', href: '/accounting/recode', perm: 'accounting:write' },
      { label: 'Budget manager', href: '/accounting/budgets', perm: 'accounting:read' },
      { label: 'Tax rates', href: '/accounting/tax-rates', perm: 'accounting:read' },
      { label: 'Currencies', href: '/accounting/currencies', perm: 'accounting:read' },
      { label: 'Tracking categories', href: '/accounting/tracking', perm: 'accounting:read' },
      { label: 'Conversion balances', href: '/accounting/conversion', perm: 'accounting:write' },
      { label: 'Assurance dashboard', href: '/accounting/assurance', perm: 'accounting:read' },
      { label: 'Workpapers', href: '/accounting/workpapers', perm: 'accounting:read' },
    ] },
    { key: 'payroll', label: 'Payroll', match: ['/payroll'], items: [
      { label: 'Payroll overview', href: '/payroll', perm: 'payroll:read' },
      { label: 'Employees', href: '/payroll/employees', perm: 'payroll:read' },
      { label: 'Pay runs', href: '/payroll/pay-runs', perm: 'payroll:read' },
      { label: 'Timesheets', href: '/payroll/timesheets', perm: 'payroll:read' },
      { label: 'Leave', href: '/payroll/leave', perm: 'payroll:read' },
      { label: 'Payroll settings', href: '/payroll/settings', perm: 'payroll:write' },
    ] },
    { key: 'contacts', label: 'Contacts', match: ['/contacts'], items: [
      { label: 'All contacts', href: '/contacts', perm: 'contacts:read' },
      { label: 'Customers', href: '/contacts?filter=customers', perm: 'contacts:read' },
      { label: 'Suppliers', href: '/contacts?filter=suppliers', perm: 'contacts:read' },
      { label: 'Archived', href: '/contacts?filter=archived', perm: 'contacts:read' },
      { label: 'Contact groups', href: '/contacts/groups', perm: 'contacts:read' },
    ] },
  ];
  const newItems: Item[] = [
    { label: 'Invoice', href: '/sales/invoices/new', perm: 'sales:write' },
    { label: 'Quote', href: '/sales/quotes/new', perm: 'sales:write' },
    { label: 'Credit note', href: '/sales/credit-notes/new', perm: 'sales:write' },
    { label: 'Bill', href: '/purchases/bills/new', perm: 'purchases:write' },
    { label: 'Purchase order', href: '/purchases/orders/new', perm: 'purchases:write' },
    { label: 'Contact', href: '/contacts/new', perm: 'contacts:write' },
    { label: 'Spend money', href: '/banking/spend', perm: 'banking:write' },
    { label: 'Receive money', href: '/banking/receive', perm: 'banking:write' },
    { label: 'Transfer money', href: '/banking/transfer', perm: 'banking:write' },
    { label: 'Manual journal', href: '/accounting/journals/new', perm: 'accounting:write' },
    { label: 'Expense claim', href: '/expenses/new', perm: 'expenses:write' },
    { label: 'Product or service', href: '/products/new', perm: 'inventory:write' },
  ];

  let open = $state<string | null>(null);
  let orgMenu = $state(false);
  let userMenu = $state(false);
  let notifCount = $state(0);
  let mobile = $state(false);

  async function refreshNotif() { try { notifCount = (await rpc('notifications')).length; } catch {} }
  refreshNotif();
  setInterval(refreshNotif, 60000);

  function isActive(m: (typeof menus)[number]) {
    if (m.key === 'home') return route.path === '/';
    return m.match.some((p) => route.path === p || route.path.startsWith(p + '/'));
  }
  function go(href: string) { open = null; orgMenu = false; userMenu = false; ui.newMenu = false; navigate(href); }
  async function switchOrg(file: string) { orgMenu = false; await rpc('orgs.open', { file }); location.href = '/'; }
  async function logout() { await rpc('auth.logout'); setToken(null); location.href = '/'; }
  function closeAll(e: MouseEvent) {
    if (!(e.target as HTMLElement).closest('[data-menu]')) { open = null; orgMenu = false; userMenu = false; ui.newMenu = false; }
  }
  const initials = $derived((session.data?.user?.name ?? '?').split(' ').map((s: string) => s[0]).join('').slice(0, 2).toUpperCase());
</script>

<svelte:window onclick={closeAll} onkeydown={(e) => { if (e.key === 'Escape' && (open || orgMenu || userMenu || ui.newMenu)) { open = null; orgMenu = false; userMenu = false; ui.newMenu = false; } }} />

<header class="no-print sticky top-0 z-30 border-b border-line bg-bg2/95 backdrop-blur">
  <div class="mx-auto flex h-14 max-w-[1440px] items-center gap-1 px-4">
    <div class="relative" data-menu>
      <button class="mr-2 flex items-center gap-2 rounded-lg px-2 py-1.5 hover:bg-hover" onclick={() => (orgMenu = !orgMenu)} aria-haspopup="menu">
        <span class="grid h-7 w-7 place-items-center rounded-md bg-accent text-[13px] font-bold text-accent-ink">{(session.data?.org?.name ?? 'L')[0]}</span>
        <span class="hidden max-w-[200px] truncate font-semibold sm:inline">{session.data?.org?.name ?? 'LedgerCraft'}</span>
        <Icon name="chevron" size={14} class="text-muted" />
      </button>
      {#if orgMenu}
        <div class="absolute left-0 mt-1 w-72 rounded-xl border border-line2 bg-panel p-1.5 shadow-[var(--shadow)]" role="menu">
          <div class="px-2.5 pb-1 pt-1.5 text-[11px] font-semibold uppercase tracking-wide text-faint">Organisations</div>
          {#each session.data?.orgs ?? [] as o}
            <button class="flex w-full items-center justify-between rounded-md px-2.5 py-2 text-left hover:bg-hover" onclick={() => switchOrg(o.file)}>
              <span class="truncate">{o.name}</span>{#if o.current}<Icon name="check" class="text-accent" />{/if}
            </button>
          {/each}
          <div class="my-1 border-t border-line"></div>
          <button class="w-full rounded-md px-2.5 py-2 text-left hover:bg-hover" onclick={() => go('/setup?new=1')}>+ Add a new organisation</button>
          <button class="w-full rounded-md px-2.5 py-2 text-left hover:bg-hover" onclick={() => go('/settings')}>Settings</button>
        </div>
      {/if}
    </div>

    <button class="btn btn-ghost btn-sm lg:hidden" aria-label="Menu" onclick={() => (mobile = !mobile)}><Icon name="menu" /></button>
    <nav class="hidden items-center gap-0.5 lg:flex" aria-label="Main">
      {#each menus as m}
        {#if m.href}
          <a href={m.href} class="rounded-lg px-3 py-1.5 font-medium {isActive(m) ? 'bg-accent-soft text-accent' : 'text-muted hover:bg-hover hover:text-text'}">{m.label}</a>
        {:else}
          <div class="relative" data-menu>
            <button class="flex items-center gap-1 rounded-lg px-3 py-1.5 font-medium {isActive(m) ? 'bg-accent-soft text-accent' : 'text-muted hover:bg-hover hover:text-text'}"
              onclick={() => (open = open === m.key ? null : m.key)} aria-haspopup="menu" aria-expanded={open === m.key}>
              {m.label}<Icon name="chevron" size={14} />
            </button>
            {#if open === m.key}
              <div class="absolute left-0 mt-1 w-60 rounded-xl border border-line2 bg-panel p-1.5 shadow-[var(--shadow)]" role="menu">
                {#each m.items ?? [] as it}
                  {#if it.sep}<div class="mx-2 mt-1.5 border-t border-line pt-2 text-[11px] font-semibold uppercase tracking-wide text-faint">{it.sep}</div>{/if}
                  {#if !it.perm || can(it.perm)}
                    <button class="block w-full rounded-md px-2.5 py-1.5 text-left hover:bg-hover" onclick={() => go(it.href!)} role="menuitem">{it.label}</button>
                  {/if}
                {/each}
              </div>
            {/if}
          </div>
        {/if}
      {/each}
    </nav>

    <div class="ml-auto flex items-center gap-1">
      <div class="relative" data-menu>
        <button class="btn btn-primary btn-sm" onclick={() => (ui.newMenu = !ui.newMenu)} aria-haspopup="menu"><Icon name="plus" size={15} />New</button>
        {#if ui.newMenu}
          <div class="absolute right-0 mt-1 w-56 rounded-xl border border-line2 bg-panel p-1.5 shadow-[var(--shadow)]" role="menu">
            {#each newItems as it}
              {#if !it.perm || can(it.perm)}<button class="block w-full rounded-md px-2.5 py-1.5 text-left hover:bg-hover" onclick={() => go(it.href!)}>{it.label}</button>{/if}
            {/each}
          </div>
        {/if}
      </div>
      <button class="btn btn-ghost btn-sm" title="Search (Ctrl+K)" aria-label="Search" onclick={() => (ui.palette = true)}><Icon name="search" /></button>
      <button class="btn btn-ghost btn-sm relative" title="Notifications" aria-label="Notifications" onclick={() => { ui.notifications = !ui.notifications; refreshNotif(); }}>
        <Icon name="bell" />{#if notifCount}<span class="absolute -right-0.5 -top-0.5 grid h-4 min-w-4 place-items-center rounded-full bg-bad px-1 text-[10px] font-bold text-white">{notifCount}</span>{/if}
      </button>
      <button class="btn btn-ghost btn-sm hidden sm:inline-flex" title="Help" aria-label="Help" onclick={() => (ui.help = true)}><Icon name="help" /></button>
      <button class="btn btn-ghost btn-sm" title={theme.mode === 'dark' ? 'Switch to light mode' : 'Switch to dark mode'} aria-label="Toggle theme" onclick={toggleTheme}><Icon name={theme.mode === 'dark' ? 'sun' : 'moon'} /></button>
      <div class="relative" data-menu>
        <button class="ml-1 grid h-8 w-8 place-items-center rounded-full bg-panel2 text-[12px] font-bold ring-1 ring-line2" onclick={() => (userMenu = !userMenu)} aria-label="Account">{initials}</button>
        {#if userMenu}
          <div class="absolute right-0 mt-1 w-56 rounded-xl border border-line2 bg-panel p-1.5 shadow-[var(--shadow)]" role="menu">
            <div class="px-2.5 py-2"><div class="font-semibold">{session.data?.user?.name}</div><div class="text-[12px] text-muted">{session.data?.user?.email} · {session.data?.role}</div></div>
            <button class="block w-full rounded-md px-2.5 py-1.5 text-left hover:bg-hover" onclick={() => go('/settings/profile')}>Profile & password</button>
            <button class="block w-full rounded-md px-2.5 py-1.5 text-left hover:bg-hover" onclick={() => go('/settings')}>Settings</button>
            {#if session.data?.login_required !== undefined || true}<button class="flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left hover:bg-hover" onclick={logout}><Icon name="logout" />Sign out</button>{/if}
          </div>
        {/if}
      </div>
    </div>
  </div>
</header>

{#if mobile}
  <div class="no-print fixed inset-0 z-40 bg-black/50 lg:hidden" role="presentation" onmousedown={() => (mobile = false)}></div>
  <aside class="no-print fixed bottom-0 left-0 top-0 z-50 w-72 max-w-[85vw] overflow-y-auto border-r border-line2 bg-panel p-3 shadow-[var(--shadow)] lg:hidden" aria-label="Menu">
    <button class="mb-2 block w-full rounded-md px-3 py-2 text-left font-semibold hover:bg-hover" onclick={() => { mobile = false; go('/'); }}>Dashboard</button>
    {#each menus.filter((m) => m.items) as m}
      <div class="mt-2 px-3 text-[11px] font-semibold uppercase tracking-wide text-faint">{m.label}</div>
      {#each m.items ?? [] as it}{#if !it.perm || can(it.perm)}<button class="block w-full rounded-md px-3 py-1.5 text-left hover:bg-hover" onclick={() => { mobile = false; go(it.href!); }}>{it.label}</button>{/if}{/each}
    {/each}
    <div class="mt-3 border-t border-line pt-2"><button class="block w-full rounded-md px-3 py-1.5 text-left hover:bg-hover" onclick={() => { mobile = false; go('/settings'); }}>Settings</button></div>
  </aside>
{/if}
