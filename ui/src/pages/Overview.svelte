<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, date } from '../lib/format';
  import { navigate } from '../lib/router.svelte';
  import { toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Bars from '../components/Bars.svelte';
  import Icon from '../components/Icon.svelte';
  let { side }: { side: 'sales' | 'purchases' } = $props();
  const sales = side === 'sales';
  let d = $state<any>(null);
  let aged = $state<any>(null);
  let top = $state<any>(null);
  Promise.all([rpc('dashboard'), rpc('reports.run', { name: sales ? 'aged-receivables' : 'aged-payables', params: {} }), rpc('reports.run', { name: sales ? 'sales-by-contact' : 'purchases-by-contact', params: {} })])
    .then(([a, b, c]) => { d = a; aged = b; top = c; }).catch(toastError);
  const s = $derived(d ? (sales ? d.sales : d.purchases) : null);
  const kind = sales ? 'invoices' : 'bills';
  const base = sales ? '/sales/invoices' : '/purchases/bills';
</script>

<PageHeader title={sales ? 'Sales overview' : 'Purchases overview'}>
  {#if sales}<a class="btn" href="/sales/quotes/new">New quote</a><a class="btn btn-primary" href="/sales/invoices/new"><Icon name="plus" />New invoice</a>
  {:else}<a class="btn" href="/purchases/orders/new">New purchase order</a><a class="btn btn-primary" href="/purchases/bills/new"><Icon name="plus" />New bill</a>{/if}
</PageHeader>

{#if s}
  <div class="mb-4 grid gap-3 sm:grid-cols-4">
    <a href="{base}?tab=draft" class="card p-4 hover:border-line2"><div class="text-[12px] text-muted">Draft ({s.draft_count})</div><div class="num text-lg font-semibold">{money(s.draft_total)}</div></a>
    <a href="{base}?tab=submitted" class="card p-4 hover:border-line2"><div class="text-[12px] text-muted">Awaiting approval ({s.submitted_count})</div><div class="num text-lg font-semibold">{money(s.submitted_total)}</div></a>
    <a href="{base}?tab=awaiting" class="card p-4 hover:border-line2"><div class="text-[12px] text-muted">{sales ? 'Awaiting payment' : 'Awaiting payment'} ({s.awaiting_count})</div><div class="num text-lg font-semibold">{money(s.awaiting_total)}</div></a>
    <a href="{base}?tab=overdue" class="card p-4 hover:border-line2"><div class="text-[12px] text-bad">Overdue ({s.overdue_count})</div><div class="num text-lg font-semibold">{money(s.overdue_total)}</div></a>
  </div>
  <div class="grid gap-4 lg:grid-cols-2">
    <section class="card p-5"><div class="mb-3 font-semibold">{sales ? 'Money coming in' : 'Money going out'} by due date</div><Bars data={sales ? d.sales_buckets : d.purchase_buckets} series={[{ key: 'amount', label: 'Due', color: sales ? 'var(--accent)' : 'var(--info)' }]} height={170} /></section>
    <section class="card p-5">
      <div class="mb-3 flex items-center justify-between"><span class="font-semibold">{sales ? 'Customers owing the most' : 'Suppliers you owe the most'}</span><a class="text-[13px] link" href="/reports/{sales ? 'aged-receivables' : 'aged-payables'}">Aged report</a></div>
      <table class="tbl text-[13px]"><thead><tr><th>Contact</th><th class="n">Overdue</th><th class="n">Total</th></tr></thead>
        <tbody>{#each (aged?.sections?.[0]?.rows ?? []).slice().sort((a: any, b: any) => b.values[5] - a.values[5]).slice(0, 8) as r}
          <tr class="clickable" onclick={() => navigate(`/contacts/${r.contact_id}`)}><td>{r.label}</td><td class="n text-bad">{money(r.values[1] + r.values[2] + r.values[3] + r.values[4], { blankZero: true })}</td><td class="n">{money(r.values[5])}</td></tr>
        {:else}<tr><td colspan="3" class="py-6 text-center text-muted">Nothing outstanding.</td></tr>{/each}</tbody></table>
    </section>
    <section class="card p-5 lg:col-span-2">
      <div class="mb-3 font-semibold">{sales ? 'Top customers' : 'Top suppliers'} this financial year</div>
      <table class="tbl text-[13px]"><thead><tr><th>Contact</th><th class="n">Transactions</th><th class="n">Net</th><th class="n">Gross</th></tr></thead>
        <tbody>{#each (top?.sections?.[0]?.rows ?? []).slice(0, 10) as r}<tr class="clickable" onclick={() => navigate(`/contacts/${r.contact_id}`)}><td>{r.label}</td><td class="n">{r.values[0]}</td><td class="n">{money(r.values[1])}</td><td class="n">{money(r.values[2])}</td></tr>{/each}</tbody></table>
    </section>
  </div>
{/if}
