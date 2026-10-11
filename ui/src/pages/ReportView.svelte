<script lang="ts">
  import { rpc } from '../lib/api';
  import { today, monthStart, monthEnd, addMonths, addDays, money } from '../lib/format';
  import { route, setQuery } from '../lib/router.svelte';
  import { lookups, setting, toastError, pushRecent } from '../lib/stores.svelte';
  import { exportCsv, exportExcel } from '../lib/exporting';
  import PageHeader from '../components/PageHeader.svelte';
  import ReportTable from '../components/ReportTable.svelte';
  import Combo from '../components/Combo.svelte';
  import Icon from '../components/Icon.svelte';
  import { accountOptions, bankAccounts } from '../components/Pickers';

  let { name }: { name: string } = $props();
  const pointInTime = ['balance-sheet', 'trial-balance', 'aged-receivables', 'aged-payables', 'bank-reconciliation', 'inventory-summary', 'fixed-asset-register', 'leave-balances'].includes(name);
  const canCompare = ['profit-and-loss', 'balance-sheet', 'cash-summary'].includes(name);
  const canMonthly = ['profit-and-loss', 'cash-summary'].includes(name);
  const canBasis = ['profit-and-loss', 'balance-sheet', 'tax-reconciliation', 'cash-summary'].includes(name);
  const canTracking = ['profit-and-loss', 'balance-sheet', 'trial-balance', 'account-transactions', 'general-ledger', 'general-ledger-detail', 'cash-summary', 'budget-variance'].includes(name);
  const needsAccount = name === 'account-transactions' || name === 'bank-reconciliation';

  const fyEndMonth = Number(setting('fy_end_month', '6'));
  function fyStart(d = today()) {
    const [y, m] = d.split('-').map(Number);
    const sm = (fyEndMonth % 12) + 1;
    const yy = m >= sm ? y : y - 1;
    return `${yy}-${String(sm).padStart(2, '0')}-01`;
  }
  function quarterStart(d = today()) { const [y, m] = d.split('-').map(Number); const qm = Math.floor((m - 1) / 3) * 3 + 1; return `${y}-${String(qm).padStart(2, '0')}-01`; }
  const presets: [string, () => [string, string]][] = [
    ['This month', () => [monthStart(), monthEnd()]],
    ['Last month', () => { const s = addMonths(monthStart(), -1); return [s, monthEnd(s)]; }],
    ['This quarter', () => { const s = quarterStart(); return [s, monthEnd(addMonths(s, 2))]; }],
    ['Last quarter', () => { const s = addMonths(quarterStart(), -3); return [s, monthEnd(addMonths(s, 2))]; }],
    ['This financial year', () => { const s = fyStart(); return [s, addDays(addMonths(s, 12), -1)]; }],
    ['Last financial year', () => { const s = addMonths(fyStart(), -12); return [s, addDays(fyStart(), -1)]; }],
    ['Year to date', () => [fyStart(), today()]],
  ];

  const q = route.query;
  let p = $state<any>({
    from: q.get('from') ?? (name === 'tax-reconciliation' ? quarterStart() : name === 'executive-summary' ? monthStart() : fyStart()),
    to: q.get('to') ?? (name === 'tax-reconciliation' ? monthEnd(addMonths(quarterStart(), 2)) : today()),
    date: q.get('date') ?? today(), basis: q.get('basis') ?? 'ACCRUAL', compare: Number(q.get('compare') ?? 0), period: q.get('period') ?? (name === 'balance-sheet' ? 'YEAR' : 'MONTH'),
    monthly: q.get('monthly') === '1', tracking: q.get('tracking') ? Number(q.get('tracking')) : null, account_id: q.get('account_id') ? Number(q.get('account_id')) : null,
    ageing_by: q.get('ageing_by') ?? 'DUE', budget_id: null as number | null, contact_id: q.get('contact_id') ? Number(q.get('contact_id')) : null,
  });
  let pctIncome = $state(false);
  let report = $state<any>(null);
  let loading = $state(false);
  let err = $state('');
  let budgets = $state<any[]>([]);
  if (name === 'budget-variance' || name === 'budget-summary' || name === 'profit-and-loss') rpc('budgets.list').then((b) => (budgets = b)).catch(() => {});

  async function run() {
    if (needsAccount && !p.account_id) { report = null; return; }
    loading = true; err = '';
    try {
      const params: any = { ...p, date: pointInTime ? p.date : undefined, to: pointInTime ? p.date : p.to, from: pointInTime ? undefined : p.from };
      if (!params.tracking) delete params.tracking;
      if (!params.budget_id) delete params.budget_id;
      report = await rpc('reports.run', { name, params });
      pushRecent(report.title, location.pathname + location.search);
      setQuery('account_id', p.account_id ? String(p.account_id) : null);
    } catch (e: any) { err = e.message; report = null; } finally { loading = false; }
  }
  run();
  const trackingOpts = $derived(lookups.tracking.flatMap((c) => c.options.map((o: any) => ({ value: o.id, label: o.name, group: c.name }))));
  const title = $derived(report?.title ?? name.replace(/-/g, ' ').replace(/^./, (c) => c.toUpperCase()));
</script>

<PageHeader {title} crumbs={[{ label: 'Reports', href: '/reports' }]}>
  {#if report}
    <button class="btn" onclick={() => window.print()}><Icon name="print" />Print / PDF</button>
    <button class="btn" onclick={() => exportExcel(report)}><Icon name="download" />Excel</button>
    <button class="btn" onclick={() => exportCsv(report)}><Icon name="download" />CSV</button>
  {/if}
</PageHeader>

<div class="no-print card mb-4 flex flex-wrap items-end gap-3 p-4">
  {#if needsAccount}
    <div class="w-72"><label class="label" for="cb-reportvi-account-1">Account</label><Combo id="cb-reportvi-account-1" bind:value={p.account_id} options={name === 'bank-reconciliation' ? bankAccounts() : accountOptions()} placeholder="Choose an account" onchange={run} /></div>
  {/if}
  {#if pointInTime}
    <div><label class="label" for="dt">{name.startsWith('aged') ? 'Ageing as at' : 'As at'}</label><input id="dt" type="date" class="input" bind:value={p.date} /></div>
  {:else}
    <div><label class="label" for="pr">Date range</label>
      <select id="pr" class="select w-48" onchange={(e) => { const i = Number((e.target as HTMLSelectElement).value); if (i >= 0) { [p.from, p.to] = presets[i][1](); run(); } }}>
        <option value={-1}>Custom</option>{#each presets as [l], i}<option value={i}>{l}</option>{/each}</select></div>
    <div><label class="label" for="fr">From</label><input id="fr" type="date" class="input" bind:value={p.from} /></div>
    <div><label class="label" for="to">To</label><input id="to" type="date" class="input" bind:value={p.to} /></div>
  {/if}
  {#if canCompare}
    <div><label class="label" for="cmp">Compare with</label>
      <div class="flex gap-1"><select id="cmp" class="select w-24" bind:value={p.compare}>{#each [0, 1, 2, 3, 4, 5, 6, 11] as n}<option value={n}>{n === 0 ? 'None' : n}</option>{/each}</select>
        <select class="select w-32" bind:value={p.period}><option value="MONTH">month(s)</option><option value="QUARTER">quarter(s)</option><option value="YEAR">year(s)</option></select></div></div>
  {/if}
  {#if canMonthly}<label class="mb-2 flex items-center gap-1.5 text-[13px]"><input type="checkbox" bind:checked={p.monthly} class="accent-[var(--accent)]" />Monthly columns</label>{/if}
  {#if canBasis}<div><label class="label" for="bs">Basis</label><select id="bs" class="select w-28" bind:value={p.basis}><option value="ACCRUAL">Accrual</option><option value="CASH">Cash</option></select></div>{/if}
  {#if name.startsWith('aged')}<div><label class="label" for="ab">Ageing by</label><select id="ab" class="select w-36" bind:value={p.ageing_by}><option value="DUE">Due date</option><option value="INVOICE">Invoice date</option></select></div>{/if}
  {#if canTracking && trackingOpts.length}<div class="w-48"><label class="label" for="cb-reportvi-tracking-2">Tracking</label><Combo id="cb-reportvi-tracking-2" bind:value={p.tracking} options={trackingOpts} placeholder="All" /></div>{/if}
  {#if (name === 'profit-and-loss' || name.startsWith('budget')) && budgets.length}
    <div><label class="label" for="bud">{name === 'profit-and-loss' ? 'Compare to budget' : 'Budget'}</label><select id="bud" class="select w-44" bind:value={p.budget_id}><option value={null}>{name === 'profit-and-loss' ? 'None' : 'Default'}</option>{#each budgets as b}<option value={b.id}>{b.name}</option>{/each}</select></div>
  {/if}
  {#if name === 'profit-and-loss'}<label class="mb-2 flex items-center gap-1.5 text-[13px]"><input type="checkbox" bind:checked={pctIncome} class="accent-[var(--accent)]" />% of income</label>{/if}
  <button class="btn btn-primary ml-auto" onclick={run} disabled={loading}>{loading ? 'Updating…' : 'Update'}</button>
</div>

<div class="card p-5 print:border-0 print:p-0">
  {#if err}<div class="text-bad">{err}</div>
  {:else if needsAccount && !p.account_id}<div class="py-10 text-center text-muted">Choose an account to run this report.</div>
  {:else if !report}<div class="py-10 text-center text-muted">Running report…</div>
  {:else}
    <div class="mb-4 text-center">
      <div class="text-[20px] font-semibold">{report.title}</div>
      <div class="text-muted">{report.org}</div>
      <div class="text-[13px] text-muted">{report.subtitle}{report.basis ? ` · ${report.basis} basis` : ''}</div>
    </div>
    <ReportTable {report} showPct={pctIncome} />
    {#if report.check && report.check.some((c: number) => c !== 0)}<div class="mt-3 text-[13px] text-bad">Warning: the balance sheet does not balance by {money(report.check[0])}.</div>{/if}
    {#if report.difference !== undefined && report.gl_movement !== undefined}
      <div class="mt-4 rounded-lg bg-bg2 p-3 text-[13px]">Movement in the tax account over the period: <b class="num">{money(report.gl_movement)}</b>.
        {#if report.difference === 0}<span class="text-ok">Reconciles to the ledger.</span>{:else}<span class="text-warn">Difference of {money(report.difference)} (manual journals or opening balances posted directly to the tax account).</span>{/if}</div>
    {/if}
  {/if}
</div>
