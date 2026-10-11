<script lang="ts">
  import { rpc } from '../lib/api';
  import { pct } from '../lib/format';
  import { can, toast, toastError, loadLookups } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Modal from '../components/Modal.svelte';
  import Icon from '../components/Icon.svelte';
  const REPORT_TYPES = [['OUTPUT', 'Tax on sales'], ['INPUT', 'Tax on purchases'], ['CAPEXINPUT', 'Tax on capital purchases'], ['EXEMPTOUTPUT', 'Exempt / GST-free sales'], ['EXEMPTINPUT', 'Exempt / GST-free purchases'],
    ['ZERORATED', 'Zero-rated sales'], ['ZERORATEDINPUT', 'Zero-rated purchases'], ['EXPORTS', 'Exports'], ['INPUTTAXED', 'Input taxed'], ['GSTONIMPORTS', 'Tax on imports'], ['REVERSECHARGE', 'Reverse charge'],
    ['BASEXCLUDED', 'Excluded from returns'], ['NONE', 'No tax']];
  let rates = $state<any[]>([]);
  const load = () => rpc('taxRates.list').then((r) => (rates = r)).catch(toastError);
  load();
  let open = $state(false);
  let editId = $state<number | null>(null);
  let f = $state<any>({ name: '', report_type: 'OUTPUT', components: [{ name: '', rate: '', compound: false }] });
  function add() { editId = null; f = { name: '', report_type: 'OUTPUT', components: [{ name: 'Tax', rate: '', compound: false }] }; open = true; }
  function edit(r: any) { editId = r.id; f = { name: r.name, report_type: r.report_type, components: r.components.map((c: any) => ({ name: c.name, rate: pct(c.rate_bp), compound: !!c.compound })) }; open = true; }
  async function save() {
    const body = { name: f.name, report_type: f.report_type, components: f.components.map((c: any) => ({ name: c.name || f.name, rate_bp: Math.round(Number(c.rate || 0) * 100), compound: c.compound })) };
    try { if (editId) await rpc('taxRates.update', { id: editId, rate: body }); else await rpc('taxRates.create', body); toast('Tax rate saved'); open = false; load(); loadLookups(); } catch (e) { toastError(e); }
  }
  async function toggle(r: any) { await rpc('taxRates.setStatus', { id: r.id, status: r.status === 'ACTIVE' ? 'ARCHIVED' : 'ACTIVE' }).catch(toastError); load(); loadLookups(); }
  const eff = $derived.by(() => { let simple = 0; let total = 0; for (const c of f.components) if (!c.compound) simple += Number(c.rate || 0); total = simple; for (const c of f.components) if (c.compound) total += (100 + simple) * Number(c.rate || 0) / 100; return total; });
</script>

<PageHeader title="Tax rates" crumbs={[{ label: 'Accounting', href: '/reports' }]}>
  {#if can('accounting:write')}<button class="btn btn-primary" onclick={add}><Icon name="plus" />New tax rate</button>{/if}
</PageHeader>
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th>Name</th><th>Components</th><th>Reports as</th><th class="n">Effective rate</th><th>Status</th><th></th></tr></thead>
    <tbody>{#each rates as r}
      <tr class="clickable" onclick={() => edit(r)}><td class="font-medium">{r.name}</td><td class="text-muted">{r.components.map((c: any) => `${c.name} ${pct(c.rate_bp)}%${c.compound ? ' (compound)' : ''}`).join(' + ')}</td>
        <td class="text-muted">{REPORT_TYPES.find((x) => x[0] === r.report_type)?.[1] ?? r.report_type}</td><td class="n">{pct(r.rate_bp)}%</td>
        <td>{#if r.status === 'ACTIVE'}<span class="chip bg-ok-soft text-ok">Active</span>{:else}<span class="chip bg-hover text-muted">Archived</span>{/if}</td>
        <td class="text-right" onclick={(e) => e.stopPropagation()}>{#if !r.system && can('accounting:write')}<button class="btn btn-ghost btn-sm" onclick={() => toggle(r)}>{r.status === 'ACTIVE' ? 'Archive' : 'Restore'}</button>{/if}</td></tr>{/each}</tbody></table>
</div>
<p class="mt-3 text-[13px] text-muted">Changing a rate only affects new transactions. Posted transactions keep the tax calculated at the time.</p>

<Modal bind:open title={editId ? 'Edit tax rate' : 'New tax rate'}>
  <div class="space-y-3">
    <div><label class="label" for="tn">Display name</label><input id="tn" class="input" bind:value={f.name} /></div>
    <div><span class="label">Reports as</span><select class="select" bind:value={f.report_type}>{#each REPORT_TYPES as [k, l]}<option value={k}>{l}</option>{/each}</select></div>
    <div><span class="label">Tax components</span>
      {#each f.components as c, i}
        <div class="mb-2 grid grid-cols-[1fr_100px_auto_auto] items-center gap-2"><input class="input" placeholder="Component name" bind:value={c.name} /><div class="relative"><input class="input num pr-6" bind:value={c.rate} /><span class="absolute right-2 top-1/2 -translate-y-1/2 text-muted">%</span></div>
          <label class="flex items-center gap-1 text-[13px]"><input type="checkbox" bind:checked={c.compound} class="accent-[var(--accent)]" />Compound</label>
          <button class="btn btn-ghost btn-sm" onclick={() => f.components.splice(i, 1)} disabled={f.components.length === 1} aria-label="Remove"><Icon name="x" size={14} /></button></div>
      {/each}
      <button class="btn btn-sm" onclick={() => f.components.push({ name: '', rate: '', compound: false })}><Icon name="plus" size={14} />Add component</button>
    </div>
    <div class="text-[13px] text-muted">Effective rate: <b class="text-text">{eff.toFixed(4).replace(/0+$/, '').replace(/\.$/, '')}%</b></div>
  </div>
  {#snippet footer()}<button class="btn" onclick={() => (open = false)}>Cancel</button>{#if can('accounting:write')}<button class="btn btn-primary" onclick={save}>Save</button>{/if}{/snippet}
</Modal>
