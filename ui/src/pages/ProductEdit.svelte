<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, qty, date, today } from '../lib/format';
  import { navigate } from '../lib/router.svelte';
  import { can, confirm, toast, toastError, loadLookups } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Combo from '../components/Combo.svelte';
  import Modal from '../components/Modal.svelte';
  import Icon from '../components/Icon.svelte';
  import { accountOptions, taxOptions } from '../components/Pickers';

  let { id = undefined }: { id?: string } = $props();
  let item = $state<any>(null);
  let f = $state<any>({ code: '', name: '', sales_description: '', purchase_description: '', is_sold: true, is_purchased: true, sale_price: '', sale_account_id: null, sale_tax_rate_id: null,
    purchase_price: '', purchase_account_id: null, purchase_tax_rate_id: null, is_tracked: false, inventory_account_id: null, reorder_level: '' });
  const p4 = (v: number | null) => (v === null || v === undefined ? '' : (v / 10000).toFixed(v % 100 === 0 ? 2 : 4));
  async function load() {
    if (!id) return;
    item = await rpc('items.get', { id: Number(id) });
    f = { ...item, is_sold: !!item.is_sold, is_purchased: !!item.is_purchased, is_tracked: !!item.is_tracked, sale_price: p4(item.sale_unit_e2), purchase_price: p4(item.purchase_unit_e2),
      reorder_level: item.reorder_level_e4 !== null ? qty(item.reorder_level_e4) : '' };
  }
  load().catch(toastError);
  async function save(e: Event) {
    e.preventDefault();
    try {
      if (id) { await rpc('items.update', { id: Number(id), item: f }); toast('Item saved'); }
      else { const r = await rpc('items.create', f); toast('Item created'); navigate(`/products/${r.id}`, { replace: true }); }
      await loadLookups(); await load();
    } catch (x) { toastError(x); }
  }
  let adj = $state({ open: false, quantity: '', unit_cost: '', date: today(), account_id: null as number | null, reason: 'Stocktake adjustment' });
  async function adjust() {
    try { await rpc('items.adjust', { id: Number(id), ...adj }); adj.open = false; toast('Inventory adjusted'); await load(); } catch (e) { toastError(e); }
  }
  async function archive() {
    if (!(await confirm(item.status === 'ACTIVE' ? 'Archive this item?' : 'Restore this item?'))) return;
    await rpc('items.setStatus', { id: Number(id), status: item.status === 'ACTIVE' ? 'ARCHIVED' : 'ACTIVE' }).catch(toastError); await load(); await loadLookups();
  }
</script>

<PageHeader title={id ? `${f.code} · ${f.name}` : 'New item'} crumbs={[{ label: 'Products and services', href: '/products' }]}>
  {#if item && can('inventory:write')}
    {#if item.is_tracked}<button class="btn" onclick={() => (adj.open = true)}><Icon name="box" />Adjust stock</button>{/if}
    <button class="btn" onclick={archive}>{item.status === 'ACTIVE' ? 'Archive' : 'Restore'}</button>
  {/if}
</PageHeader>

{#if item && item.is_tracked}
  <div class="mb-4 grid gap-3 sm:grid-cols-3">
    <div class="card p-4"><div class="text-[12px] text-muted">Quantity on hand</div><div class="num text-lg font-semibold">{qty(item.qty_on_hand_e4)}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">Average cost</div><div class="num text-lg font-semibold">{money(item.avg_cost)}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">Total value</div><div class="num text-lg font-semibold">{money(item.total_cost)}</div></div>
  </div>
{/if}

<form class="grid gap-4 lg:grid-cols-2" onsubmit={save}>
  <section class="card space-y-3 p-5 lg:col-span-2">
    <div class="grid grid-cols-1 gap-3 sm:grid-cols-3">
      <div><label class="label" for="ic">Item code *</label><input id="ic" class="input" bind:value={f.code} required /></div>
      <div class="sm:col-span-2"><label class="label" for="in">Item name</label><input id="in" class="input" bind:value={f.name} /></div>
    </div>
    <label class="flex items-center gap-2"><input type="checkbox" bind:checked={f.is_tracked} disabled={!!item?.movements?.length} class="accent-[var(--accent)]" />Track inventory for this item (quantity on hand and average cost)</label>
    {#if f.is_tracked}<div class="grid grid-cols-2 gap-3"><div><label class="label" for="cb-producte-inventory-asset-acco-1">Inventory asset account</label><Combo id="cb-producte-inventory-asset-acco-1" bind:value={f.inventory_account_id} options={accountOptions((a) => a.class === 'ASSET' && a.type !== 'BANK')} placeholder="Default inventory account" /></div>
      <div><label class="label" for="rl">Reorder level</label><input id="rl" class="input num" bind:value={f.reorder_level} /></div></div>{/if}
  </section>
  <section class="card space-y-3 p-5">
    <label class="flex items-center gap-2 font-semibold"><input type="checkbox" bind:checked={f.is_purchased} class="accent-[var(--accent)]" />I purchase this item</label>
    {#if f.is_purchased}
      <div><label class="label" for="pp">Cost price</label><input id="pp" class="input num" bind:value={f.purchase_price} /></div>
      <div><label class="label" for="cb-producte-f-is-tracked-cost-of-2">{f.is_tracked ? 'Cost of goods sold account' : 'Purchases account'}</label><Combo id="cb-producte-f-is-tracked-cost-of-2" bind:value={f.purchase_account_id} options={accountOptions((a) => a.type !== 'BANK')} /></div>
      <div><label class="label" for="cb-producte-tax-rate-3">Tax rate</label><Combo id="cb-producte-tax-rate-3" bind:value={f.purchase_tax_rate_id} options={taxOptions()} /></div>
      <div><label class="label" for="pd">Purchase description</label><textarea id="pd" rows="2" class="textarea" bind:value={f.purchase_description}></textarea></div>
    {/if}
  </section>
  <section class="card space-y-3 p-5">
    <label class="flex items-center gap-2 font-semibold"><input type="checkbox" bind:checked={f.is_sold} class="accent-[var(--accent)]" />I sell this item</label>
    {#if f.is_sold}
      <div><label class="label" for="sp">Sale price</label><input id="sp" class="input num" bind:value={f.sale_price} /></div>
      <div><label class="label" for="cb-producte-sales-account-4">Sales account</label><Combo id="cb-producte-sales-account-4" bind:value={f.sale_account_id} options={accountOptions((a) => a.type !== 'BANK')} /></div>
      <div><label class="label" for="cb-producte-tax-rate-5">Tax rate</label><Combo id="cb-producte-tax-rate-5" bind:value={f.sale_tax_rate_id} options={taxOptions()} /></div>
      <div><label class="label" for="sd">Sales description</label><textarea id="sd" rows="2" class="textarea" bind:value={f.sales_description}></textarea></div>
    {/if}
  </section>
  <div class="flex justify-end gap-2 lg:col-span-2"><a class="btn" href="/products">Cancel</a>{#if can('inventory:write')}<button class="btn btn-primary">Save</button>{/if}</div>
</form>

{#if item?.movements?.length}
  <section class="card mt-4 overflow-x-auto">
    <div class="px-5 py-3 font-semibold">Stock movements</div>
    <table class="tbl text-[13px]"><thead><tr><th>Date</th><th>Source</th><th class="n">Quantity</th><th class="n">Cost</th></tr></thead>
      <tbody>{#each item.movements as m}<tr><td>{date(m.date)}</td><td>{m.source_type}</td><td class="n">{qty(m.qty_e4)}</td><td class="n">{money(m.cost)}</td></tr>{/each}</tbody></table>
  </section>
{/if}

<Modal bind:open={adj.open} title="Adjust inventory">
  <div class="space-y-3">
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="aq">Quantity change (+ add / − remove)</label><input id="aq" class="input num" bind:value={adj.quantity} placeholder="-2" /></div>
      <div><label class="label" for="ad">Date</label><input id="ad" type="date" class="input" bind:value={adj.date} /></div></div>
    {#if !adj.quantity.startsWith('-')}<div><label class="label" for="au">Unit cost (for added stock)</label><input id="au" class="input num" bind:value={adj.unit_cost} /></div>{/if}
    <div><label class="label" for="cb-producte-adjustment-account-6">Adjustment account</label><Combo id="cb-producte-adjustment-account-6" bind:value={adj.account_id} options={accountOptions((a) => a.type !== 'BANK')} placeholder="e.g. Cost of Goods Sold" /></div>
    <div><label class="label" for="ar">Reason</label><input id="ar" class="input" bind:value={adj.reason} /></div>
  </div>
  {#snippet footer()}<button class="btn" onclick={() => (adj.open = false)}>Cancel</button><button class="btn btn-primary" disabled={!adj.account_id || !adj.quantity} onclick={adjust}>Adjust</button>{/snippet}
</Modal>
