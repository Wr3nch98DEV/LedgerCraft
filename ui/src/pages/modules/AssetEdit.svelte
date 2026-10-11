<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, date, today, moneyPlain } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { can, confirm, toast, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Status from '../../components/Status.svelte';
  import History from '../../components/History.svelte';
  import Modal from '../../components/Modal.svelte';
  import Combo from '../../components/Combo.svelte';
  import { accountOptions, bankAccounts } from '../../components/Pickers';
  let { id = 'new' }: { id?: string } = $props();
  const isNew = id === 'new';
  let a = $state<any>(null);
  let types = $state<any[]>([]);
  let f = $state<any>({ name: '', description: '', asset_type_id: null, purchase_date: today(), purchase_price: '', serial_number: '', warranty_expiry: '', method: '', rate: '', life_years: '', residual_value: '', dep_start_date: '', opening_accum_dep: '', tax_method: '', tax_rate: '', tax_life_years: '' });
  rpc('assets.list').then((r) => (types = r.types));
  async function load() {
    if (isNew) return;
    a = await rpc('assets.get', { id: Number(id) });
    f = { name: a.name, description: a.description, asset_type_id: a.asset_type_id, purchase_date: a.purchase_date, purchase_price: moneyPlain(a.purchase_price), serial_number: a.serial_number, warranty_expiry: a.warranty_expiry ?? '',
      method: a.method, rate: a.rate_bp ? String(a.rate_bp / 100) : '', life_years: a.life_months ? String(a.life_months / 12) : '', residual_value: moneyPlain(a.residual_value), dep_start_date: a.dep_start_date ?? '',
      opening_accum_dep: moneyPlain(a.opening_accum_dep), tax_method: a.tax_method ?? '', tax_rate: a.tax_rate_bp ? String(a.tax_rate_bp / 100) : '', tax_life_years: a.tax_life_months ? String(a.tax_life_months / 12) : '' };
  }
  load().catch(toastError);
  async function save(register = false) {
    try {
      const r = await rpc('assets.save', { id: isNew ? null : Number(id), asset: f });
      if (register) await rpc('assets.register', { id: r.id });
      toast(register ? 'Asset registered' : 'Asset saved');
      if (isNew) navigate(`/accounting/assets/${r.id}`, { replace: true }); else load();
    } catch (e) { toastError(e); }
  }
  let disp = $state({ open: false, date: today(), sale_price: '', proceeds_account_id: null as number | null, gain_loss_account_id: null as number | null });
  async function dispose() { try { await rpc('assets.dispose', { id: Number(id), ...disp }); disp.open = false; toast('Asset disposed'); load(); } catch (e) { toastError(e); } }
  async function del() { if (await confirm('Delete this draft asset?', '', 'Delete', true)) { await rpc('assets.delete', { id: Number(id) }); navigate('/accounting/assets'); } }
  const locked = $derived(a && a.status !== 'DRAFT' && a.accum_dep !== 0);
  const METHODS: [string, string][] = [['', 'Use asset type default'], ['STRAIGHTLINE', 'Straight line'], ['DIMINISHING100', 'Diminishing value (100%)'], ['DIMINISHING150', 'Diminishing value (150%)'], ['DIMINISHING200', 'Diminishing value (200%)'], ['FULL', 'Full depreciation at purchase'], ['NONE', 'No depreciation']];
</script>

<PageHeader title={isNew ? 'New asset' : `${a?.number ?? ''} ${a?.name ?? ''}`} crumbs={[{ label: 'Fixed assets', href: '/accounting/assets' }]}>
  {#snippet sub()}{#if a}<Status status={a.status} />{/if}{/snippet}
  {#if a?.status === 'REGISTERED' && can('assets:write')}<button class="btn" onclick={() => (disp.open = true)}>Sell / dispose</button>{/if}
  {#if a?.status === 'DRAFT' && can('assets:write')}<button class="btn btn-danger" onclick={del}>Delete</button>{/if}
</PageHeader>
{#if a && a.status !== 'DRAFT'}
  <div class="mb-4 grid gap-3 sm:grid-cols-4">
    <div class="card p-4"><div class="text-[12px] text-muted">Cost</div><div class="num text-lg font-semibold">{money(a.purchase_price)}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">Accumulated depreciation</div><div class="num text-lg font-semibold">{money(a.accum_dep + a.opening_accum_dep)}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">Book value</div><div class="num text-lg font-semibold">{money(a.book_value)}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">Depreciated to</div><div class="text-lg font-semibold">{date(a.last_dep_date) || '—'}</div></div>
  </div>
{/if}
<div class="grid gap-4 lg:grid-cols-2">
  <section class="card space-y-3 p-5">
    <h2 class="font-semibold">Asset details</h2>
    <div><label class="label" for="an">Name</label><input id="an" class="input" bind:value={f.name} /></div>
    <div><span class="label">Asset type</span><select class="select" bind:value={f.asset_type_id} disabled={locked}><option value={null}>— choose —</option>{#each types as t}<option value={t.id}>{t.name}</option>{/each}</select></div>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="pd">Purchase date</label><input id="pd" type="date" class="input" bind:value={f.purchase_date} disabled={locked} /></div><div><label class="label" for="pp">Purchase price</label><input id="pp" class="input num" bind:value={f.purchase_price} disabled={locked} /></div></div>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="sn">Serial number</label><input id="sn" class="input" bind:value={f.serial_number} /></div><div><label class="label" for="we">Warranty expiry</label><input id="we" type="date" class="input" bind:value={f.warranty_expiry} /></div></div>
    <div><label class="label" for="ds">Description</label><textarea id="ds" rows="2" class="textarea" bind:value={f.description}></textarea></div>
  </section>
  <section class="card space-y-3 p-5">
    <h2 class="font-semibold">Book depreciation</h2>
    <div><span class="label">Method</span><select class="select" bind:value={f.method} disabled={locked}>{#each METHODS as [k, v]}<option value={k}>{v}</option>{/each}</select></div>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="rt">Rate % p.a.</label><input id="rt" class="input" bind:value={f.rate} disabled={locked} /></div><div><label class="label" for="ly">or effective life (years)</label><input id="ly" class="input" bind:value={f.life_years} disabled={locked} /></div></div>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="dsd">Depreciation start date</label><input id="dsd" type="date" class="input" bind:value={f.dep_start_date} disabled={locked} /></div><div><label class="label" for="rv">Residual value</label><input id="rv" class="input num" bind:value={f.residual_value} disabled={locked} /></div></div>
    <div><label class="label" for="oa">Accumulated depreciation before start (existing assets)</label><input id="oa" class="input num" bind:value={f.opening_accum_dep} disabled={locked} /></div>
    <h2 class="pt-2 font-semibold">Tax depreciation <span class="text-[12px] font-normal text-muted">(optional, if different)</span></h2>
    <div class="grid grid-cols-3 gap-3"><select class="select" bind:value={f.tax_method} disabled={locked}>{#each METHODS as [k, v]}<option value={k}>{k ? v : 'Same as book'}</option>{/each}</select><input class="input" placeholder="Rate %" bind:value={f.tax_rate} disabled={locked} /><input class="input" placeholder="Life (years)" bind:value={f.tax_life_years} disabled={locked} /></div>
  </section>
</div>
{#if a?.schedule?.length}
  <section class="card mt-4 p-5"><div class="mb-2 font-semibold">Projected depreciation</div>
    <table class="tbl text-[13px]"><thead><tr><th>Financial year starting</th><th class="n">Depreciation</th><th class="n">Closing book value</th></tr></thead>
      <tbody>{#each a.schedule as y}<tr><td>{date(y.year_start)}</td><td class="n">{money(y.depreciation)}</td><td class="n">{money(y.closing_book_value)}</td></tr>{/each}</tbody></table></section>
{/if}
{#if can('assets:write') && a?.status !== 'DISPOSED'}
  <div class="mt-4 flex justify-end gap-2"><a class="btn" href="/accounting/assets">Cancel</a><button class="btn" onclick={() => save(false)}>Save{isNew || a?.status === 'DRAFT' ? ' as draft' : ''}</button>{#if isNew || a?.status === 'DRAFT'}<button class="btn btn-primary" onclick={() => save(true)}>Register</button>{/if}</div>
{/if}
{#if a}<History entity="asset" id={a.id} />{/if}

<Modal bind:open={disp.open} title="Sell or dispose of asset">
  <div class="space-y-3">
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="dd">Date</label><input id="dd" type="date" class="input" bind:value={disp.date} /></div><div><label class="label" for="dp">Sale price (0 if scrapped)</label><input id="dp" class="input num" bind:value={disp.sale_price} /></div></div>
    {#if disp.sale_price && Number(disp.sale_price) > 0}<div><label class="label" for="cb-assetedi-proceeds-received-in-1">Proceeds received into</label><Combo id="cb-assetedi-proceeds-received-in-1" bind:value={disp.proceeds_account_id} options={bankAccounts()} /></div>{/if}
    <div><label class="label" for="cb-assetedi-gain-loss-account-2">Gain / loss account</label><Combo id="cb-assetedi-gain-loss-account-2" bind:value={disp.gain_loss_account_id} options={accountOptions((x) => x.class === 'REVENUE' || x.class === 'EXPENSE')} /></div>
    <p class="text-[12px] text-muted">Depreciation is brought up to the end of the previous month first. The asset's cost and accumulated depreciation are removed and the gain or loss is posted.</p>
  </div>
  {#snippet footer()}<button class="btn" onclick={() => (disp.open = false)}>Cancel</button><button class="btn btn-primary" disabled={!disp.gain_loss_account_id} onclick={dispose}>Dispose</button>{/snippet}
</Modal>
