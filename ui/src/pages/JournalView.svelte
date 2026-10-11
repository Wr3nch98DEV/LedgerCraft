<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, date, dateTime } from '../lib/format';
  import { toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  let { id }: { id: string } = $props();
  let j = $state<any>(null);
  rpc('journal.get', { id: Number(id) }).then((x) => (j = x)).catch(toastError);
  const dr = $derived(j ? j.lines.reduce((s: number, l: any) => s + Math.max(0, l.amount), 0) : 0);
</script>

{#if j}
<PageHeader title="Journal #{j.number}" crumbs={[{ label: 'Reports', href: '/reports' }, { label: 'Journal report', href: '/reports/journal-report' }]}>
  {#snippet sub()}{date(j.date)} · {j.source_type} {j.reference} · posted {dateTime(j.created_at)}{#if j.reversed} · <span class="text-warn">reversed</span>{/if}{#if j.reverses_journal_id} · reverses <a class="link" href="/journal/{j.reverses_journal_id}">#{j.reverses_journal_id}</a>{/if}{/snippet}
  {#if ['ACCREC', 'ACCPAY', 'ACCRECCREDIT', 'ACCPAYCREDIT'].includes(j.source_type)}<a class="btn" href="/doc/{j.source_id}">Open source document</a>{/if}
</PageHeader>
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th>Account</th><th>Description</th><th>Tax rate</th><th class="n">Tax</th><th class="n">Debit</th><th class="n">Credit</th></tr></thead>
    <tbody>{#each j.lines as l}<tr><td>{l.account_code} - {l.account_name}</td><td class="text-muted">{l.description}</td><td class="text-muted">{l.tax_rate_name ?? ''}</td><td class="n text-muted">{money(l.tax_amount, { blankZero: true })}</td>
      <td class="n">{l.amount > 0 ? money(l.amount) : ''}</td><td class="n">{l.amount < 0 ? money(-l.amount) : ''}</td></tr>{/each}
      <tr class="font-semibold"><td colspan="4">Total</td><td class="n">{money(dr)}</td><td class="n">{money(dr)}</td></tr></tbody></table>
</div>
<p class="mt-3 text-[13px] text-muted">Posted journals are immutable. Corrections are made by reversing journals, so the full history is always kept.</p>
{/if}
