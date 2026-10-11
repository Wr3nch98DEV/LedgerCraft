<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, date, dateTime } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { can, confirm, toast, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Status from '../../components/Status.svelte';
  import Icon from '../../components/Icon.svelte';
  let { id }: { id: string } = $props();
  let r = $state<any>(null);
  const load = () => rpc('taxReturns.get', { id: Number(id) }).then((x) => (r = x)).catch(toastError);
  load();
  async function file() {
    if (!(await confirm('Mark this return as filed?', 'The figures are frozen and transactions up to the period end are locked for non-adviser users.', 'Mark as filed'))) return;
    try { await rpc('taxReturns.file', { id: Number(id) }); toast('Return filed'); load(); } catch (e) { toastError(e); }
  }
  async function del() { if (await confirm('Delete this return?', '', 'Delete', true)) { try { await rpc('taxReturns.delete', { id: Number(id) }); navigate('/accounting/tax-returns'); } catch (e) { toastError(e); } } }
</script>

{#if r}
<PageHeader title={r.data.title} crumbs={[{ label: 'Tax returns', href: '/accounting/tax-returns' }]}>
  {#snippet sub()}{date(r.period_start)} – {date(r.period_end)} · {r.basis.toLowerCase()} basis · <Status status={r.status} />{#if r.filed_at} · filed {dateTime(r.filed_at)}{/if}{/snippet}
  <button class="btn" onclick={() => window.print()}><Icon name="print" />Print</button>
  {#if r.jurisdiction === 'UK-VAT100'}<a class="btn" href="/download/mtd?id={r.id}" download><Icon name="download" />MTD JSON</a>{/if}
  {#if can('accounting:write')}{#if r.status === 'DRAFT'}<button class="btn btn-primary" onclick={file}>Mark as filed</button>{/if}<button class="btn btn-danger" onclick={del}>Delete</button>{/if}
</PageHeader>
<div class="grid gap-4 lg:grid-cols-[1fr_420px]">
  <section class="card p-5">
    <table class="tbl"><thead><tr><th class="w-16">Box</th><th>Description</th><th class="n">Amount</th></tr></thead>
      <tbody>{#each r.data.boxes as b}<tr class={b.total ? 'font-semibold' : ''}><td><span class="chip bg-accent-soft text-accent">{b.box}</span></td><td>{b.label}</td><td class="n {b.total ? 'text-[16px]' : ''}">{money(b.amount)}</td></tr>{/each}</tbody></table>
    {#if r.status === 'DRAFT'}<p class="mt-3 text-[13px] text-muted">Draft returns update automatically as transactions change.</p>{/if}
  </section>
  <section class="card p-5">
    <div class="mb-2 font-semibold">Transactions by tax rate</div>
    <table class="tbl text-[13px]"><thead><tr><th>Tax rate</th><th class="n">Net</th><th class="n">Tax</th></tr></thead>
      <tbody>{#each r.transactions as t}<tr><td>{t.name}</td><td class="n">{money(t.net)}</td><td class="n">{money(t.tax)}</td></tr>{/each}</tbody></table>
    <a class="mt-3 inline-block link text-[13px]" href="/reports/tax-reconciliation?from={r.period_start}&to={r.period_end}&basis={r.basis}">Open the full tax reconciliation →</a>
  </section>
</div>
{/if}
