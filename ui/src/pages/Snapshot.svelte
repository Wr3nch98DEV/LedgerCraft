<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, fixed } from '../lib/format';
  import { toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Bars from '../components/Bars.svelte';
  let ex = $state<any>(null);
  let pl = $state<any>(null);
  Promise.all([rpc('reports.run', { name: 'executive-summary', params: {} }), rpc('reports.run', { name: 'profit-and-loss', params: {} })])
    .then(([a, b]) => { ex = a; pl = b; }).catch(toastError);
  const metric = (label: string) => ex?.sections[0].rows.find((r: any) => r.label === label)?.values ?? [];
  // Ratios use the last complete month; balances use today.
  const last = (label: string) => { const v = metric(label); return v[v.length - 1] ?? 0; };
  const prev = (label: string) => { const v = metric(label); return v[v.length - 2] ?? 0; };
  const expenses = $derived(pl ? (pl.sections.find((s: any) => s.title === 'Operating Expenses')?.rows ?? []).slice().sort((a: any, b: any) => b.values[0] - a.values[0]).slice(0, 6) : []);
  const expTotal = $derived(expenses.reduce((s: number, r: any) => s + r.values[0], 0));
  const COLORS = ['var(--accent)', 'var(--info)', 'var(--warn)', 'var(--ok)', 'var(--bad)', 'var(--muted)'];
</script>

<PageHeader title="Business snapshot" crumbs={[{ label: 'Dashboard', href: '/' }]} />
{#if ex}
  <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
    <section class="card p-5"><div class="text-[12px] font-semibold uppercase tracking-wide text-faint">Profitability</div>
      <div class="mt-2 text-[13px] text-muted">Net profit last month</div><div class="num text-2xl font-semibold">{money(prev('Net profit'))}</div>
      <div class="mt-2 text-[13px] text-muted">Gross margin <b class="text-text">{fixed(prev('Gross margin %'), 2)}%</b> · Net margin <b class="text-text">{fixed(prev('Net margin %'), 2)}%</b></div></section>
    <section class="card p-5"><div class="text-[12px] font-semibold uppercase tracking-wide text-faint">Efficiency</div>
      <div class="mt-2 text-[13px] text-muted">Average time to get paid</div><div class="num text-2xl font-semibold">{prev('Debtor days')} days</div>
      <div class="mt-2 text-[13px] text-muted">Average time to pay suppliers <b class="text-text">{prev('Creditor days')} days</b></div></section>
    <section class="card p-5"><div class="text-[12px] font-semibold uppercase tracking-wide text-faint">Financial position</div>
      <div class="mt-2 text-[13px] text-muted">Cash in the bank</div><div class="num text-2xl font-semibold">{money(last('Closing bank balance'))}</div>
      <div class="mt-2 text-[13px] text-muted">Current ratio <b class="text-text">{(last('Current ratio %') / 10000).toFixed(2)}</b></div></section>
    <section class="card p-5"><div class="text-[12px] font-semibold uppercase tracking-wide text-faint">Owed</div>
      <div class="mt-2 text-[13px] text-muted">Customers owe you</div><div class="num text-2xl font-semibold">{money(last('Debtors (receivables)'))}</div>
      <div class="mt-2 text-[13px] text-muted">You owe suppliers <b class="text-text">{money(last('Creditors (payables)'))}</b></div></section>
    <section class="card p-5 md:col-span-2"><div class="mb-3 font-semibold">Income vs expenses (3 months)</div>
      <Bars data={ex.columns.map((c: string, i: number) => ({ label: c, income: metric('Income')[i], expenses: metric('Expenses')[i] + metric('Direct costs')[i] }))} series={[{ key: 'income', label: 'Income', color: 'var(--ok)' }, { key: 'expenses', label: 'Expenses', color: 'var(--bad)' }]} height={170} /></section>
    <section class="card p-5 md:col-span-2"><div class="mb-3 font-semibold">Top expenses this financial year</div>
      <div class="mb-3 flex h-3 overflow-hidden rounded-full">{#each expenses as e, i}<div style="width:{(e.values[0] / (expTotal || 1)) * 100}%;background:{COLORS[i]}"></div>{/each}</div>
      {#each expenses as e, i}<div class="flex items-center justify-between py-1 text-[13px]"><span class="flex items-center gap-2"><span class="h-2.5 w-2.5 rounded-sm" style="background:{COLORS[i]}"></span>{e.label}</span><span class="num">{money(e.values[0])}</span></div>{/each}</section>
  </div>
{/if}
