<script lang="ts">
  import { rpc } from '../lib/api';
  import { can, confirm, toast, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Modal from '../components/Modal.svelte';
  import Combo from '../components/Combo.svelte';
  import ContactSelect from '../components/ContactSelect.svelte';
  import Icon from '../components/Icon.svelte';
  import Tabs from '../components/Tabs.svelte';
  import { accountOptions, taxOptions, bankAccounts } from '../components/Pickers';

  let rules = $state<any[]>([]);
  let tab = $state('SPEND');
  const load = () => rpc('bankRules.list').then((r) => (rules = r)).catch(toastError);
  load();
  let open = $state(false);
  let editId = $state<number | null>(null);
  const blank = () => ({ name: '', type: tab, account_id: null, match_mode: 'ALL', conditions: [{ field: 'any', op: 'contains', value: '' }], contact_id: null,
    reference_field: '', fixed_lines: [] as any[], percent_lines: [{ account_id: null, tax_rate_id: null, percent: '100', description: '' }], transfer_account_id: null, priority: 0, active: true });
  let r = $state<any>(blank());
  function edit(x: any) { editId = x.id; r = JSON.parse(JSON.stringify({ ...x, active: !!x.active })); if (!r.percent_lines.length && !r.fixed_lines.length) r.percent_lines = []; open = true; }
  function add() { editId = null; r = blank(); open = true; }
  async function save() {
    try { await rpc('bankRules.save', { id: editId, rule: r }); toast('Rule saved'); open = false; load(); } catch (e) { toastError(e); }
  }
  async function del(x: any) { if (await confirm(`Delete rule "${x.name}"?`, '', 'Delete', true)) { await rpc('bankRules.delete', { id: x.id }); load(); } }
  const shown = $derived(rules.filter((x) => x.type === tab));
  const pctTotal = $derived(r.percent_lines.reduce((s: number, l: any) => s + Number(l.percent || 0), 0));
</script>

<PageHeader title="Bank rules" crumbs={[{ label: 'Bank accounts', href: '/banking' }]}>
  {#if can('banking:write')}<button class="btn btn-primary" onclick={add}><Icon name="plus" />New rule</button>{/if}
</PageHeader>
<Tabs tabs={[{ key: 'SPEND', label: 'Spend money rules' }, { key: 'RECEIVE', label: 'Receive money rules' }, { key: 'TRANSFER', label: 'Transfer rules' }]} bind:value={tab} />
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th>Rule</th><th>Conditions</th><th>Applies to</th><th>Status</th><th></th></tr></thead>
    <tbody>{#each shown as x}
      <tr class="clickable" onclick={() => edit(x)}>
        <td class="font-medium">{x.name}</td>
        <td class="text-[13px] text-muted">{x.conditions.map((c: any) => `${c.field} ${c.op.replace('_', ' ')} "${c.value}"`).join(x.match_mode === 'ANY' ? ' or ' : ' and ')}</td>
        <td class="text-[13px]">{x.account_name ?? 'All bank accounts'}</td>
        <td>{#if x.active}<span class="chip bg-ok-soft text-ok">Active</span>{:else}<span class="chip bg-hover text-muted">Off</span>{/if}</td>
        <td class="text-right" onclick={(e) => e.stopPropagation()}>{#if can('banking:write')}<button class="btn btn-ghost btn-sm" onclick={() => del(x)} aria-label="Delete"><Icon name="trash" size={14} /></button>{/if}</td>
      </tr>
    {:else}<tr><td colspan="5" class="py-10 text-center text-muted">No rules yet. Rules automatically code statement lines that match conditions you set, e.g. payee contains "Uber" → Travel.</td></tr>{/each}</tbody></table>
</div>

<Modal bind:open title={editId ? 'Edit bank rule' : 'New bank rule'} width={760}>
  <div class="space-y-4">
    <div class="grid grid-cols-3 gap-3">
      <div class="col-span-2"><label class="label" for="rn">Rule name</label><input id="rn" class="input" bind:value={r.name} /></div>
      <div><span class="label">Type</span><select class="select" bind:value={r.type}><option value="SPEND">Spend money</option><option value="RECEIVE">Receive money</option><option value="TRANSFER">Transfer</option></select></div>
    </div>
    <div>
      <div class="mb-1 flex items-center gap-2 text-[13px]"><span class="font-semibold">1. When a bank line matches</span><select class="select !w-auto !py-1" bind:value={r.match_mode}><option value="ALL">all</option><option value="ANY">any</option></select><span>of these conditions</span></div>
      {#each r.conditions as c, i}
        <div class="mb-2 grid grid-cols-[150px_150px_1fr_auto] gap-2">
          <select class="select" bind:value={c.field}><option value="any">Any text field</option><option value="payee">Payee</option><option value="description">Description</option><option value="reference">Reference</option><option value="amount">Amount</option></select>
          <select class="select" bind:value={c.op}>{#if c.field === 'amount'}<option value="equals">equals</option><option value="gt">is greater than</option><option value="lt">is less than</option>{:else}<option value="contains">contains</option><option value="equals">equals</option><option value="starts_with">starts with</option><option value="not_contains">doesn't contain</option>{/if}</select>
          <input class="input" bind:value={c.value} />
          <button class="btn btn-ghost btn-sm" onclick={() => r.conditions.splice(i, 1)} aria-label="Remove"><Icon name="x" size={14} /></button>
        </div>
      {/each}
      <button class="btn btn-sm" onclick={() => r.conditions.push({ field: 'any', op: 'contains', value: '' })}><Icon name="plus" size={14} />Add condition</button>
    </div>
    {#if r.type === 'TRANSFER'}
      <div><span class="label">2. Transfer to / from</span><Combo bind:value={r.transfer_account_id} options={bankAccounts()} /></div>
    {:else}
      <div><span class="label">2. Set the contact (optional)</span><ContactSelect bind:value={r.contact_id} /></div>
      <div>
        <div class="mb-1 text-[13px] font-semibold">3. Allocate</div>
        {#each r.fixed_lines as l, i}
          <div class="mb-2 grid grid-cols-[1fr_1fr_1fr_110px_auto] gap-2"><input class="input" placeholder="Description" bind:value={l.description} /><Combo bind:value={l.account_id} options={accountOptions((a) => a.type !== 'BANK')} placeholder="Account" /><Combo bind:value={l.tax_rate_id} options={taxOptions()} placeholder="Tax" /><input class="input num" placeholder="Fixed amount" bind:value={l.amount} /><button class="btn btn-ghost btn-sm" onclick={() => r.fixed_lines.splice(i, 1)} aria-label="Remove"><Icon name="x" size={14} /></button></div>
        {/each}
        {#each r.percent_lines as l, i}
          <div class="mb-2 grid grid-cols-[1fr_1fr_1fr_110px_auto] gap-2"><input class="input" placeholder="Description" bind:value={l.description} /><Combo bind:value={l.account_id} options={accountOptions((a) => a.type !== 'BANK')} placeholder="Account" /><Combo bind:value={l.tax_rate_id} options={taxOptions()} placeholder="Tax" /><div class="relative"><input class="input num pr-6" bind:value={l.percent} /><span class="absolute right-2 top-1/2 -translate-y-1/2 text-muted">%</span></div><button class="btn btn-ghost btn-sm" onclick={() => r.percent_lines.splice(i, 1)} aria-label="Remove"><Icon name="x" size={14} /></button></div>
        {/each}
        <div class="flex gap-2"><button class="btn btn-sm" onclick={() => r.fixed_lines.push({ account_id: null, tax_rate_id: null, amount: '', description: '' })}>Add fixed line</button><button class="btn btn-sm" onclick={() => r.percent_lines.push({ account_id: null, tax_rate_id: null, percent: '', description: '' })}>Add % line</button>
          {#if r.percent_lines.length}<span class="ml-auto text-[13px] {pctTotal === 100 ? 'text-ok' : 'text-warn'}">Percentages total {pctTotal}%</span>{/if}</div>
      </div>
    {/if}
    <div class="grid grid-cols-2 gap-3">
      <div><span class="label">Run this rule on</span><Combo bind:value={r.account_id} options={bankAccounts()} placeholder="All bank accounts" /></div>
      <label class="mt-5 flex items-center gap-2"><input type="checkbox" bind:checked={r.active} class="accent-[var(--accent)]" />Rule is active</label>
    </div>
  </div>
  {#snippet footer()}<button class="btn" onclick={() => (open = false)}>Cancel</button><button class="btn btn-primary" onclick={save}>Save rule</button>{/snippet}
</Modal>
