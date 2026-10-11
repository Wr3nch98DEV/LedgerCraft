<script lang="ts">
  import { rpc } from '../lib/api';
  import { rate, date, today } from '../lib/format';
  import { can, toast, toastError, loadLookups, setting } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Modal from '../components/Modal.svelte';
  import Combo from '../components/Combo.svelte';
  let list = $state<any[]>([]);
  const load = () => rpc('currencies.list').then((c) => (list = c)).catch(toastError);
  load();
  const base = setting('base_currency', 'AUD');
  let add = $state<string | null>(null);
  let rateOpen = $state(false);
  let rf = $state({ code: '', date: today(), rate: '' });
  let history = $state<any[]>([]);
  async function enable() { if (!add) return; await rpc('currencies.enable', { code: add, enabled: true }).catch(toastError); add = null; toast('Currency added'); await load(); loadLookups(); }
  async function openRates(code: string) { rf = { code, date: today(), rate: '' }; history = await rpc('rates.list', { code }); rateOpen = true; }
  async function saveRate() { try { await rpc('rates.set', rf); toast('Rate saved'); history = await rpc('rates.list', { code: rf.code }); load(); } catch (e) { toastError(e); } }
  let revalueDate = $state(today());
  async function runRevaluation() {
    try { const r = await rpc('fx.revalue', { date: revalueDate }); toast(r.posted ? `Revaluation posted (${r.detail.length} item(s)), reversing next day` : 'Nothing to revalue'); } catch (e) { toastError(e); }
  }
  const enabled = $derived(list.filter((c) => c.enabled));
  const others = $derived(list.filter((c) => !c.enabled).map((c) => ({ value: c.code, label: c.code, sub: c.name })));
</script>

<PageHeader title="Currencies" crumbs={[{ label: 'Accounting', href: '/reports' }]} />
<div class="grid gap-4 lg:grid-cols-[1fr_340px]">
  <div class="card overflow-x-auto">
    <table class="tbl"><thead><tr><th>Code</th><th>Currency</th><th class="n">Latest rate (1 {base} =)</th><th>As of</th><th></th></tr></thead>
      <tbody>{#each enabled as c}<tr><td class="font-medium">{c.code}{#if c.code === base}<span class="chip ml-2 bg-accent-soft text-accent">Base</span>{/if}</td><td>{c.name}</td>
        <td class="n">{c.code === base ? '1' : c.latest_rate_e8 ? rate(c.latest_rate_e8) : '—'}</td><td class="text-muted">{date(c.latest_rate_date)}</td>
        <td class="text-right">{#if c.code !== base}<button class="btn btn-sm" onclick={() => openRates(c.code)}>Rates</button>{/if}</td></tr>{/each}</tbody></table>
  </div>
  <aside class="space-y-4">
    {#if can('settings:write')}
      <div class="card p-4"><div class="mb-2 font-semibold">Add a currency</div><div class="flex gap-2"><div class="flex-1"><Combo bind:value={add} options={others} placeholder="Search 160+ currencies" /></div><button class="btn btn-primary" disabled={!add} onclick={enable}>Add</button></div></div>
    {/if}
    <div class="card p-4"><div class="mb-1 font-semibold">Currency revaluation</div><p class="mb-3 text-[13px] text-muted">Posts unrealised gains/losses on open foreign-currency invoices, bills and bank accounts at period end, reversing automatically the next day.</p>
      <div class="flex gap-2"><input type="date" class="input" bind:value={revalueDate} /><button class="btn" onclick={runRevaluation}>Revalue</button></div></div>
    <div class="card p-4 text-[13px] text-muted">Exchange rates are entered manually (works offline). Rates are expressed as how many units of the foreign currency equal 1 {base}.</div>
  </aside>
</div>

<Modal bind:open={rateOpen} title="{rf.code} exchange rates">
  <div class="mb-3 flex items-end gap-2"><div><label class="label" for="rd">Date</label><input id="rd" type="date" class="input" bind:value={rf.date} /></div>
    <div class="flex-1"><label class="label" for="rr">1 {base} =</label><input id="rr" class="input num" bind:value={rf.rate} placeholder="0.6500" /></div><button class="btn btn-primary" onclick={saveRate}>Save</button></div>
  <table class="tbl text-[13px]"><thead><tr><th>Date</th><th class="n">Rate</th><th>Source</th></tr></thead><tbody>{#each history as h}<tr><td>{date(h.date)}</td><td class="n">{rate(h.rate_e8)}</td><td>{h.source}</td></tr>{/each}</tbody></table>
</Modal>
