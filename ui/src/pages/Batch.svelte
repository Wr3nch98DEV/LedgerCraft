<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, date } from '../lib/format';
  import { toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Modal from '../components/Modal.svelte';
  import Icon from '../components/Icon.svelte';
  let rows = $state<any[]>([]);
  rpc('batch.list').then((r) => (rows = r)).catch(toastError);
  let detail = $state<any>(null);
  let open = $state(false);
  async function show(id: number) { detail = await rpc('batch.get', { id }); open = true; }
</script>

<PageHeader title="Batch payments" crumbs={[{ label: 'Purchases', href: '/purchases' }]}>
  <a class="btn btn-primary" href="/purchases/bills?tab=awaiting"><Icon name="plus" />New batch (select bills)</a>
</PageHeader>
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th>Date</th><th>Type</th><th>Account</th><th>Reference</th><th class="n">Payments</th><th class="n">Total</th><th></th></tr></thead>
    <tbody>{#each rows as b}<tr class="clickable" onclick={() => show(b.id)}><td>{date(b.date)}</td><td>{b.type === 'PAYBATCH' ? 'Bills paid' : 'Invoices received'}</td><td>{b.account_name}</td><td class="text-muted">{b.reference}</td><td class="n">{b.payment_count}</td><td class="n">{money(b.total)}</td>
      <td class="text-right" onclick={(e) => e.stopPropagation()}>{#if b.type === 'PAYBATCH'}<a class="btn btn-sm" href="/download/batch-file?id={b.id}&format=ABA" download>ABA</a> <a class="btn btn-sm" href="/download/batch-file?id={b.id}&format=SEPA" download>SEPA</a> <a class="btn btn-sm" href="/download/batch-file?id={b.id}&format=BACS" download>BACS</a> <a class="btn btn-sm" href="/download/batch-file?id={b.id}&format=NACHA" download>NACHA</a>{/if}</td></tr>
    {:else}<tr><td colspan="7" class="py-10 text-center text-muted">No batch payments yet. Select several bills on the Bills page and choose "Batch payment".</td></tr>{/each}</tbody></table>
</div>
<p class="mt-3 text-[13px] text-muted">Bank payment files (ABA for Australia, BACS for the UK, NACHA for the US, SEPA XML for Europe) use each supplier's bank details from their contact record. Check the file with your bank before uploading.</p>

<Modal bind:open title="Batch payment" width={700}>
  {#if detail}
    <div class="mb-3 text-muted">{date(detail.date)} · {detail.account_name} · {detail.reference}</div>
    <table class="tbl text-[13px]"><thead><tr><th>Contact</th><th>Document</th><th>Bank details</th><th class="n">Amount</th></tr></thead>
      <tbody>{#each detail.payments as p}<tr><td>{p.contact_name}</td><td>{p.number}</td><td class="text-muted">{p.bank_account || '—'}</td><td class="n">{money(p.amount)}</td></tr>{/each}</tbody></table>
  {/if}
</Modal>
