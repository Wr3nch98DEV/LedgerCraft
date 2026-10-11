<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, date } from '../lib/format';
  import { navigate } from '../lib/router.svelte';
  import { can, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Tabs from '../components/Tabs.svelte';
  import Status from '../components/Status.svelte';
  import Icon from '../components/Icon.svelte';
  let rows = $state<any[]>([]);
  let tab = $state('ALL');
  $effect(() => { rpc('mj.list', { status: tab }).then((r) => (rows = r)).catch(toastError); });
</script>

<PageHeader title="Manual journals" crumbs={[{ label: 'Accounting', href: '/reports' }]}>
  <a class="btn" href="/accounting/repeating-journals">Repeating</a>
  {#if can('accounting:write')}<a class="btn btn-primary" href="/accounting/journals/new"><Icon name="plus" />New journal</a>{/if}
</PageHeader>
<Tabs tabs={[{ key: 'ALL', label: 'All' }, { key: 'DRAFT', label: 'Draft' }, { key: 'POSTED', label: 'Posted' }, { key: 'VOIDED', label: 'Voided' }]} bind:value={tab} />
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th>Narration</th><th>Date</th><th>Auto-reversing</th><th class="n">Debit total</th><th>Status</th></tr></thead>
    <tbody>{#each rows as r}<tr class="clickable" onclick={() => navigate(`/accounting/journals/${r.id}`)}><td class="font-medium">{r.narration}</td><td>{date(r.date)}</td><td class="text-muted">{date(r.auto_reverse_date)}</td><td class="n">{money(r.debit_total)}</td><td><Status status={r.status} /></td></tr>
    {:else}<tr><td colspan="5" class="py-10 text-center text-muted">No manual journals.</td></tr>{/each}</tbody></table>
</div>
