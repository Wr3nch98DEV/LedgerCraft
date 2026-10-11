<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, date } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { can, toast, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Tabs from '../../components/Tabs.svelte';
  import Status from '../../components/Status.svelte';
  import Icon from '../../components/Icon.svelte';
  let rows = $state<any[]>([]);
  let tab = $state('ALL');
  const load = () => rpc('receipts.list', { status: tab }).then((r) => (rows = r)).catch(toastError);
  $effect(() => { tab; load(); });
  async function act(r: any, status: string) { try { await rpc('receipts.setStatus', { id: r.id, status }); toast('Claim updated'); load(); } catch (e) { toastError(e); } }
</script>

<PageHeader title="Expense claims" crumbs={[{ label: 'Business', href: '/sales' }]}>
  <a class="btn" href="/reports/expense-claims">Report</a>
  {#if can('expenses:write')}<a class="btn btn-primary" href="/expenses/new"><Icon name="plus" />New receipt</a><a class="btn" href="/expenses/new?kind=MILEAGE">New mileage claim</a>{/if}
</PageHeader>
<Tabs tabs={[{ key: 'ALL', label: 'All' }, { key: 'DRAFT', label: 'Draft' }, { key: 'SUBMITTED', label: 'Awaiting approval' }, { key: 'AUTHORISED', label: 'Awaiting payment' }, { key: 'PAID', label: 'Paid' }, { key: 'DECLINED', label: 'Declined' }]} bind:value={tab} />
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th>Date</th><th>Claimant</th><th>Spent at</th><th>Type</th><th class="n">Amount</th><th>Status</th><th></th></tr></thead>
    <tbody>{#each rows as r}<tr class="clickable" onclick={() => navigate(r.document_id && r.status !== 'DRAFT' && r.status !== 'SUBMITTED' ? `/doc/${r.document_id}` : `/expenses/${r.id}`)}>
      <td>{date(r.date)}</td><td>{r.claimant}</td><td>{r.supplier}</td><td class="text-muted">{r.kind === 'MILEAGE' ? 'Mileage' : 'Receipt'}</td><td class="n">{money(r.total)}</td><td><Status status={r.status} /></td>
      <td class="whitespace-nowrap text-right" onclick={(e) => e.stopPropagation()}>
        {#if r.status === 'DRAFT' && can('expenses:write')}<button class="btn btn-sm" onclick={() => act(r, 'SUBMITTED')}>Submit</button>{/if}
        {#if r.status === 'SUBMITTED' && can('purchases:write')}<button class="btn btn-sm btn-primary" onclick={() => act(r, 'AUTHORISED')}>Approve</button> <button class="btn btn-sm" onclick={() => act(r, 'DECLINED')}>Decline</button>{/if}
        {#if r.status === 'AUTHORISED'}<a class="btn btn-sm" href="/doc/{r.document_id}">Pay</a>{/if}
      </td></tr>
    {:else}<tr><td colspan="7" class="py-10 text-center text-muted">No expense claims. Staff can record receipts and mileage here; approved claims become bills you reimburse.</td></tr>{/each}</tbody></table>
</div>
