<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, date, today } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { can, confirm, toast, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Tabs from '../../components/Tabs.svelte';
  import Status from '../../components/Status.svelte';
  import Modal from '../../components/Modal.svelte';
  import Combo from '../../components/Combo.svelte';
  import Icon from '../../components/Icon.svelte';
  import { accountOptions } from '../../components/Pickers';
  let d = $state<any>(null);
  let tab = $state('REGISTERED');
  const load = () => rpc('assets.list').then((r) => (d = r)).catch(toastError);
  load();
  const shown = $derived((d?.assets ?? []).filter((a: any) => a.status === tab));
  const counts = $derived(Object.fromEntries(['DRAFT', 'REGISTERED', 'DISPOSED'].map((k) => [k, (d?.assets ?? []).filter((a: any) => a.status === k).length])));
  let runDate = $state(today());
  async function run() {
    try { const r = await rpc('assets.runDepreciation', { date: runDate }); toast(r.posted ? `Depreciation of ${money(r.total)} posted` : 'Nothing to depreciate for this period'); load(); } catch (e) { toastError(e); }
  }
  async function rollback() { if (await confirm('Roll back the last depreciation run?', 'The journal is reversed and asset balances restored.', 'Roll back', true)) { try { await rpc('assets.rollback'); toast('Rolled back'); load(); } catch (e) { toastError(e); } } }
  let typeOpen = $state(false);
  let tf = $state<any>({ name: '', asset_account_id: null, accum_dep_account_id: null, dep_expense_account_id: null, method: 'STRAIGHTLINE', rate: '', life_years: '' });
  async function saveType() { try { await rpc('assets.saveType', { id: tf.id ?? null, type: tf }); typeOpen = false; toast('Asset type saved'); load(); } catch (e) { toastError(e); } }
  const METHODS: Record<string, string> = { STRAIGHTLINE: 'Straight line', DIMINISHING100: 'Diminishing value (100%)', DIMINISHING150: 'Diminishing value (150%)', DIMINISHING200: 'Diminishing value (200%)', FULL: 'Full depreciation at purchase', NONE: 'No depreciation' };
</script>

<PageHeader title="Fixed assets" crumbs={[{ label: 'Accounting', href: '/reports' }]}>
  <a class="btn" href="/reports/fixed-asset-register">Register report</a>
  <a class="btn" href="/reports/fixed-asset-reconciliation">Reconciliation</a>
  {#if can('assets:write')}<button class="btn" onclick={() => { tf = { name: '', method: 'STRAIGHTLINE' }; typeOpen = true; }}>Asset types</button><a class="btn btn-primary" href="/accounting/assets/new"><Icon name="plus" />New asset</a>{/if}
</PageHeader>
{#if d}
  {#if can('assets:write')}
    <div class="card mb-4 flex flex-wrap items-end gap-3 p-4">
      <div><label class="label" for="rd">Run depreciation to the end of</label><input id="rd" type="date" class="input" bind:value={runDate} /></div>
      <button class="btn btn-primary" onclick={run}>Run depreciation</button>
      {#if d.runs.some((r: any) => r.status === 'POSTED')}<button class="btn" onclick={rollback}>Roll back last run</button>{/if}
      <div class="ml-auto text-[13px] text-muted">{#if d.runs[0]}Last run to {date(d.runs[0].period_end)} ({money(d.runs[0].total)}){/if}</div>
    </div>
  {/if}
  <Tabs tabs={[{ key: 'DRAFT', label: 'Draft', count: counts.DRAFT }, { key: 'REGISTERED', label: 'Registered', count: counts.REGISTERED }, { key: 'DISPOSED', label: 'Sold / disposed', count: counts.DISPOSED }]} bind:value={tab} />
  <div class="card overflow-x-auto">
    <table class="tbl"><thead><tr><th>Number</th><th>Asset</th><th>Type</th><th>Purchased</th><th class="n">Cost</th><th class="n">Book value</th><th>Method</th></tr></thead>
      <tbody>{#each shown as a}<tr class="clickable" onclick={() => navigate(`/accounting/assets/${a.id}`)}><td>{a.number}</td><td class="font-medium">{a.name}</td><td class="text-muted">{a.type_name ?? '—'}</td>
        <td>{date(a.purchase_date)}</td><td class="n">{money(a.purchase_price)}</td><td class="n">{money(a.book_value)}</td><td class="text-muted">{METHODS[a.method]}</td></tr>
      {:else}<tr><td colspan="7" class="py-10 text-center text-muted">No assets here.</td></tr>{/each}</tbody></table>
  </div>
  <section class="card mt-4 p-5">
    <div class="mb-2 font-semibold">Asset types</div>
    <table class="tbl text-[13px]"><thead><tr><th>Type</th><th>Asset account</th><th>Accumulated depreciation</th><th>Expense account</th><th>Default method</th></tr></thead>
      <tbody>{#each d.types as t}<tr class="clickable" onclick={() => { tf = { ...t, rate: t.rate_bp ? String(t.rate_bp / 100) : '', life_years: t.life_months ? String(t.life_months / 12) : '' }; typeOpen = true; }}><td>{t.name}</td><td>{t.asset_account}</td><td>{t.accum_account}</td><td>{t.expense_account}</td><td>{METHODS[t.method]}</td></tr>{/each}</tbody></table>
  </section>
{/if}

<Modal bind:open={typeOpen} title="Asset type">
  <div class="space-y-3">
    <div><label class="label" for="tn">Name</label><input id="tn" class="input" bind:value={tf.name} /></div>
    <div><span class="label">Asset account</span><Combo bind:value={tf.asset_account_id} options={accountOptions((a) => a.type === 'FIXED' || a.type === 'NONCURRENT')} /></div>
    <div><span class="label">Accumulated depreciation account</span><Combo bind:value={tf.accum_dep_account_id} options={accountOptions((a) => a.type === 'FIXED' || a.type === 'NONCURRENT')} /></div>
    <div><span class="label">Depreciation expense account</span><Combo bind:value={tf.dep_expense_account_id} options={accountOptions((a) => a.class === 'EXPENSE')} /></div>
    <div><span class="label">Method</span><select class="select" bind:value={tf.method}>{#each Object.entries(METHODS) as [k, v]}<option value={k}>{v}</option>{/each}</select></div>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="tr">Rate % p.a.</label><input id="tr" class="input" bind:value={tf.rate} /></div><div><label class="label" for="tl">or effective life (years)</label><input id="tl" class="input" bind:value={tf.life_years} /></div></div>
  </div>
  {#snippet footer()}<button class="btn" onclick={() => (typeOpen = false)}>Cancel</button><button class="btn btn-primary" onclick={saveType}>Save</button>{/snippet}
</Modal>
