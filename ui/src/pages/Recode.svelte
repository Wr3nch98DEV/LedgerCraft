<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, date, DOC_LABEL } from '../lib/format';
  import { confirm, toast, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Combo from '../components/Combo.svelte';
  import ContactSelect from '../components/ContactSelect.svelte';
  import { accountOptions, taxOptions, trackingCategories, trackingOptions } from '../components/Pickers';
  let f = $state<any>({ account_id: null, tax_rate_id: null, contact_id: null, from: '', to: '', q: '' });
  let rows = $state<any[] | null>(null);
  let sel = $state<Set<number>>(new Set());
  let to = $state<any>({ account_id: null, tax_rate_id: null, tracking1: null, tracking2: null });
  async function find() { try { rows = await rpc('recode.find', f); sel = new Set(); } catch (e) { toastError(e); } }
  function toggle(id: number) { const s = new Set(sel); s.has(id) ? s.delete(id) : s.add(id); sel = s; }
  async function apply() {
    if (!(await confirm(`Recode ${sel.size} line(s)?`, 'Each affected transaction is reversed and re-posted with the new coding. Locked periods cannot be recoded.', 'Recode'))) return;
    try { const r = await rpc('recode.apply', { line_ids: [...sel], ...to }); toast(`${r.transactions} transaction(s) recoded`); find(); } catch (e) { toastError(e); }
  }
  const cats = trackingCategories();
</script>

<PageHeader title="Find and recode" crumbs={[{ label: 'Accounting', href: '/reports' }]} />
<div class="card mb-4 grid gap-3 p-4 md:grid-cols-6">
  <div class="md:col-span-2"><span class="label">Account</span><Combo bind:value={f.account_id} options={accountOptions()} placeholder="Any" /></div>
  <div><span class="label">Tax rate</span><Combo bind:value={f.tax_rate_id} options={taxOptions()} placeholder="Any" /></div>
  <div><span class="label">Contact</span><ContactSelect bind:value={f.contact_id} placeholder="Any" /></div>
  <div><label class="label" for="ff">From</label><input id="ff" type="date" class="input" bind:value={f.from} /></div>
  <div><label class="label" for="ft">To</label><input id="ft" type="date" class="input" bind:value={f.to} /></div>
  <div class="md:col-span-4"><label class="label" for="fq">Description contains</label><input id="fq" class="input" bind:value={f.q} /></div>
  <div class="flex items-end md:col-span-2"><button class="btn btn-primary w-full justify-center" onclick={find}>Search</button></div>
</div>
{#if rows}
  {#if sel.size}
    <div class="card mb-3 flex flex-wrap items-end gap-3 p-4">
      <div class="text-muted">Recode {sel.size} selected line(s) to:</div>
      <div class="w-60"><span class="label">Account</span><Combo bind:value={to.account_id} options={accountOptions()} placeholder="Keep" /></div>
      <div class="w-48"><span class="label">Tax rate</span><Combo bind:value={to.tax_rate_id} options={taxOptions()} placeholder="Keep" /></div>
      {#each cats as c, k}<div class="w-40"><span class="label">{c.name}</span><Combo value={k === 0 ? to.tracking1 : to.tracking2} options={trackingOptions(c)} placeholder="Keep" onchange={(v) => { if (k === 0) to.tracking1 = v; else to.tracking2 = v; }} /></div>{/each}
      <button class="btn btn-primary" onclick={apply}>Recode</button>
    </div>
  {/if}
  <div class="card overflow-x-auto">
    <table class="tbl text-[13px]"><thead><tr><th class="w-8"><input type="checkbox" class="accent-[var(--accent)]" checked={rows.length > 0 && sel.size === rows.length} onchange={(e) => (sel = (e.target as HTMLInputElement).checked ? new Set(rows!.map((r) => r.line_id)) : new Set())} aria-label="Select all" /></th><th>Date</th><th>Type</th><th>Number</th><th>Contact</th><th>Description</th><th>Account</th><th>Tax</th><th class="n">Net</th></tr></thead>
      <tbody>{#each rows as r}<tr class="clickable {sel.has(r.line_id) ? 'sel' : ''}" onclick={() => toggle(r.line_id)}><td><input type="checkbox" checked={sel.has(r.line_id)} class="accent-[var(--accent)]" aria-label="Select" /></td><td>{date(r.date)}</td><td>{DOC_LABEL[r.type] ?? r.type}</td><td>{r.number}</td><td>{r.contact ?? ''}</td><td class="max-w-[220px] truncate">{r.description}</td><td>{r.account_code} {r.account_name}</td><td>{r.tax_rate_name ?? ''}</td><td class="n">{money(r.net_amount)}</td></tr>
      {:else}<tr><td colspan="9" class="py-8 text-center text-muted">No matching lines.</td></tr>{/each}</tbody></table>
  </div>
{/if}
