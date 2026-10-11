<script lang="ts">
  import { rpc, openView } from '../lib/api';
  import { money, date, docPath, DOC_LABEL } from '../lib/format';
  import { navigate } from '../lib/router.svelte';
  import { can, pushRecent, toastError, accountName } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Status from '../components/Status.svelte';
  import Icon from '../components/Icon.svelte';
  import History from '../components/History.svelte';
  import Attachments from '../components/Attachments.svelte';
  import Tabs from '../components/Tabs.svelte';

  let { id }: { id: string } = $props();
  let c = $state<any>(null);
  let activity = $state<any[]>([]);
  let tab = $state('activity');
  Promise.all([rpc('contacts.get', { id: Number(id) }), rpc('contacts.activity', { id: Number(id) })])
    .then(([a, b]) => { c = a; activity = b; pushRecent(a.name, location.pathname); }).catch(toastError);
  function open(r: any) {
    if (r.kind === 'document') navigate(docPath(r.subtype, r.id));
    else if (r.kind === 'banktx') navigate(`/banking/transaction/${r.id}`);
  }
  const KIND: Record<string, string> = { payment: 'Payment', banktx: 'Bank transaction' };
</script>

{#if c}
<PageHeader title={c.name} crumbs={[{ label: 'Contacts', href: '/contacts' }]}>
  {#snippet sub()}{[c.is_customer ? 'Customer' : '', c.is_supplier ? 'Supplier' : '', c.is_employee ? 'Employee' : ''].filter(Boolean).join(' · ')}{#if c.status === 'ARCHIVED'} · <Status status="ARCHIVED" />{/if}{/snippet}
  {#if can('contacts:write')}<a class="btn" href="/contacts/{c.id}/edit"><Icon name="edit" />Edit</a>{/if}
  <button class="btn" onclick={() => openView(`/view/statement/${c.id}`)}><Icon name="print" />Statement</button>
  <a class="btn btn-primary" href="/sales/invoices/new?contact={c.id}"><Icon name="plus" />New invoice</a>
  <a class="btn" href="/purchases/bills/new?contact={c.id}">New bill</a>
</PageHeader>

<div class="grid gap-4 lg:grid-cols-[1fr_320px]">
  <div>
    <div class="mb-4 grid gap-3 sm:grid-cols-4">
      <div class="card p-4"><div class="text-[12px] text-muted">They owe you</div><div class="num text-lg font-semibold">{money(c.summary.receivable)}</div></div>
      <div class="card p-4"><div class="text-[12px] text-bad">Overdue</div><div class="num text-lg font-semibold">{money(c.summary.receivable_overdue)}</div></div>
      <div class="card p-4"><div class="text-[12px] text-muted">You owe them</div><div class="num text-lg font-semibold">{money(c.summary.payable)}</div></div>
      <div class="card p-4"><div class="text-[12px] text-muted">Sales (12 months)</div><div class="num text-lg font-semibold">{money(c.summary.sales_12m)}</div></div>
    </div>
    <Tabs tabs={[{ key: 'activity', label: 'Activity', count: activity.length }]} bind:value={tab} />
    <div class="card overflow-x-auto">
      <table class="tbl">
        <thead><tr><th>Date</th><th>Type</th><th>Number</th><th>Reference</th><th class="n">Amount</th><th class="n">Due</th><th>Status</th></tr></thead>
        <tbody>
          {#each activity as r}
            <tr class="clickable" onclick={() => open(r)}>
              <td class="whitespace-nowrap">{date(r.date)}</td>
              <td>{r.kind === 'document' ? DOC_LABEL[r.subtype] : KIND[r.kind] ?? r.kind}</td>
              <td>{r.number}</td><td class="max-w-[200px] truncate text-muted">{r.reference}</td>
              <td class="n">{r.currency} {money(r.amount)}</td><td class="n">{money(r.due, { blankZero: true })}</td>
              <td><Status status={r.status} /></td>
            </tr>
          {:else}<tr><td colspan="7" class="py-8 text-center text-muted">No activity yet.</td></tr>{/each}
        </tbody>
      </table>
    </div>
  </div>
  <aside class="space-y-4">
    <div class="card p-4 text-[14px]">
      <div class="mb-2 font-semibold">Contact details</div>
      {#if c.first_name || c.last_name}<div>{c.first_name} {c.last_name}</div>{/if}
      {#if c.email}<a class="link block" href="mailto:{c.email}">{c.email}</a>{/if}
      {#if c.phone}<div class="text-muted">{c.phone}</div>{/if}
      {#if c.mobile}<div class="text-muted">{c.mobile}</div>{/if}
      {#if c.website}<div class="text-muted">{c.website}</div>{/if}
      {#if c.postal_address}<div class="mt-2 whitespace-pre-line text-muted">{c.postal_address}</div>{/if}
      {#if c.tax_number}<div class="mt-2 text-[13px]"><span class="text-muted">Tax number:</span> {c.tax_number}</div>{/if}
      {#if c.account_number}<div class="text-[13px]"><span class="text-muted">Account no.:</span> {c.account_number}</div>{/if}
      {#if c.bank_account}<div class="text-[13px]"><span class="text-muted">Bank:</span> {c.bank_account}</div>{/if}
    </div>
    {#if c.people.length}
      <div class="card p-4 text-[14px]"><div class="mb-2 font-semibold">People</div>
        {#each c.people as p}<div class="py-1">{p.first_name} {p.last_name} <span class="text-muted">{p.email}</span></div>{/each}</div>
    {/if}
    <div class="card p-4 text-[13px]">
      <div class="mb-2 font-semibold">Defaults</div>
      <div><span class="text-muted">Sales account:</span> {accountName(c.sales_account_id) || '—'}</div>
      <div><span class="text-muted">Purchase account:</span> {accountName(c.purchase_account_id) || '—'}</div>
      <div><span class="text-muted">Currency:</span> {c.default_currency || 'Base'}</div>
      {#if c.sales_terms_days !== null}<div><span class="text-muted">Invoice terms:</span> {c.sales_terms_days} days</div>{/if}
    </div>
    {#if c.notes}<div class="card whitespace-pre-wrap p-4 text-[13px]"><div class="mb-1 font-semibold">Notes</div>{c.notes}</div>{/if}
    {#if c.groups.length}<div class="card p-4 text-[13px]"><div class="mb-1 font-semibold">Groups</div>{c.groups.map((g: any) => g.name).join(', ')}</div>{/if}
  </aside>
</div>
<Attachments entity="contact" id={c.id} />
<History entity="contact" id={c.id} />
{/if}
