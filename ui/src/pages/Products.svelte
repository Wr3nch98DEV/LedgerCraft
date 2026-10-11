<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, qty, unitPrice } from '../lib/format';
  import { navigate } from '../lib/router.svelte';
  import { can, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Icon from '../components/Icon.svelte';
  import Empty from '../components/Empty.svelte';
  import Tabs from '../components/Tabs.svelte';
  let rows = $state<any[] | null>(null);
  let q = $state('');
  let tab = $state('all');
  let timer: any;
  const load = () => rpc('items.list', { q }).then((r) => (rows = r)).catch(toastError);
  load();
  const shown = $derived((rows ?? []).filter((r) => tab === 'all' ? r.status === 'ACTIVE' : tab === 'tracked' ? r.is_tracked && r.status === 'ACTIVE' : tab === 'low' ? r.low_stock : r.status === 'ARCHIVED'));
</script>

<PageHeader title="Products and services">
  <a class="btn" href="/reports/inventory-summary"><Icon name="chart" />Inventory report</a>
  {#if can('inventory:write')}<a class="btn btn-primary" href="/products/new"><Icon name="plus" />New item</a>{/if}
</PageHeader>
<Tabs tabs={[{ key: 'all', label: 'All items' }, { key: 'tracked', label: 'Tracked inventory' }, { key: 'low', label: 'Low stock', count: (rows ?? []).filter((r) => r.low_stock).length || null }, { key: 'archived', label: 'Archived' }]} bind:value={tab} />
<div class="mb-3 w-72 relative"><Icon name="search" class="absolute left-2.5 top-1/2 -translate-y-1/2 text-faint" /><input class="input pl-8" placeholder="Search code or name" bind:value={q} oninput={() => { clearTimeout(timer); timer = setTimeout(load, 180); }} /></div>
<div class="card overflow-x-auto">
  {#if rows === null}<div class="p-8 text-center text-muted">Loading…</div>
  {:else if !shown.length}<Empty icon="box" title="No items" text="Items make invoicing faster and let you track stock.">{#if can('inventory:write')}<a class="btn btn-primary" href="/products/new">New item</a>{/if}</Empty>
  {:else}
    <table class="tbl">
      <thead><tr><th>Code</th><th>Name</th><th class="n">Cost price</th><th class="n">Sale price</th><th class="n">Quantity</th><th class="n">Avg cost</th><th class="n">Value</th></tr></thead>
      <tbody>{#each shown as r (r.id)}
        <tr class="clickable" onclick={() => navigate(`/products/${r.id}`)}>
          <td class="font-medium">{r.code}</td><td>{r.name}{#if r.low_stock}<span class="chip ml-2 bg-warn-soft text-warn">Low stock</span>{/if}</td>
          <td class="n">{r.purchase_unit_e2 !== null ? unitPrice(r.purchase_unit_e2 / 100) : ''}</td>
          <td class="n">{r.sale_unit_e2 !== null ? unitPrice(r.sale_unit_e2 / 100) : ''}</td>
          <td class="n">{r.is_tracked ? qty(r.qty_on_hand_e4) : ''}</td>
          <td class="n">{r.is_tracked ? money(r.avg_cost) : ''}</td>
          <td class="n">{r.is_tracked ? money(r.total_cost) : ''}</td>
        </tr>{/each}</tbody>
    </table>
  {/if}
</div>
