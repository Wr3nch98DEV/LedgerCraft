<script lang="ts">
  import { rpc } from '../lib/api';
  import { money } from '../lib/format';
  import { navigate } from '../lib/router.svelte';
  import { can, confirm, toast, toastError, loadLookups } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Tabs from '../components/Tabs.svelte';
  import Modal from '../components/Modal.svelte';
  import Combo from '../components/Combo.svelte';
  import Icon from '../components/Icon.svelte';
  import { taxOptions } from '../components/Pickers';

  const TYPES: [string, string, string][] = [['BANK', 'ASSET', 'Bank'], ['CURRENT', 'ASSET', 'Current Asset'], ['FIXED', 'ASSET', 'Fixed Asset'], ['NONCURRENT', 'ASSET', 'Non-current Asset'], ['INVENTORY', 'ASSET', 'Inventory'], ['PREPAYMENT', 'ASSET', 'Prepayment'],
    ['CURRLIAB', 'LIABILITY', 'Current Liability'], ['LIABILITY', 'LIABILITY', 'Liability'], ['TERMLIAB', 'LIABILITY', 'Non-current Liability'], ['EQUITY', 'EQUITY', 'Equity'],
    ['REVENUE', 'REVENUE', 'Revenue'], ['SALES', 'REVENUE', 'Sales'], ['OTHERINCOME', 'REVENUE', 'Other Income'], ['DIRECTCOSTS', 'EXPENSE', 'Direct Costs'], ['EXPENSE', 'EXPENSE', 'Expense'], ['OVERHEADS', 'EXPENSE', 'Overhead'], ['DEPRECIATN', 'EXPENSE', 'Depreciation']];
  const typeLabel = (t: string) => TYPES.find((x) => x[0] === t)?.[2] ?? t;
  let rows = $state<any[]>([]);
  let tab = $state('ALL');
  let q = $state('');
  let sel = $state<Set<number>>(new Set());
  const load = () => rpc('accounts.list').then((r) => (rows = r)).catch(toastError);
  load();
  const shown = $derived(rows.filter((r) => (tab === 'ARCHIVED' ? r.status === 'ARCHIVED' : r.status === 'ACTIVE' && (tab === 'ALL' || r.class === tab))
    && (!q || `${r.code} ${r.name} ${r.description}`.toLowerCase().includes(q.toLowerCase()))));
  let open = $state(false);
  let editId = $state<number | null>(null);
  let f = $state<any>({});
  function add() { editId = null; f = { code: '', name: '', type: 'EXPENSE', tax_rate_id: null, description: '', enable_payments: false, show_in_expense_claims: false, dashboard_watch: false }; open = true; }
  function edit(r: any) { editId = r.id; f = { ...r, enable_payments: !!r.enable_payments, show_in_expense_claims: !!r.show_in_expense_claims, dashboard_watch: !!r.dashboard_watch }; open = true; }
  async function save() {
    try {
      if (editId) await rpc('accounts.update', { id: editId, account: f }); else await rpc('accounts.create', f);
      toast('Account saved'); open = false; await load(); await loadLookups();
    } catch (e) { toastError(e); }
  }
  async function status(s: string) {
    for (const id of sel) await rpc('accounts.setStatus', { id, status: s }).catch(toastError);
    sel = new Set(); toast('Accounts updated'); load(); loadLookups();
  }
  async function del() {
    if (!(await confirm(`Delete ${sel.size} account(s)?`, 'Only accounts with no transactions can be deleted.', 'Delete', true))) return;
    for (const id of sel) await rpc('accounts.delete', { id }).catch(toastError);
    sel = new Set(); load(); loadLookups();
  }
  let bulkTax = $state<number | null>(null);
  async function applyTax() { await rpc('accounts.bulkTax', { ids: [...sel], tax_rate_id: bulkTax }).catch(toastError); toast('Tax rates updated'); sel = new Set(); load(); loadLookups(); }
  function toggle(id: number) { const s = new Set(sel); s.has(id) ? s.delete(id) : s.add(id); sel = s; }
  function exportCsv() {
    const lines = ['*Code,*Name,*Type,*Tax Code,Description,Dashboard,Expense Claims,Enable Payments,Balance'];
    for (const r of rows) lines.push([r.code, r.name, typeLabel(r.type), r.tax_rate_name ?? '', r.description, r.dashboard_watch ? 'Yes' : 'No', r.show_in_expense_claims ? 'Yes' : 'No', r.enable_payments ? 'Yes' : 'No', (r.ytd_balance / 100).toFixed(2)].map((c) => `"${String(c).replace(/"/g, '""')}"`).join(','));
    const a = document.createElement('a'); a.href = URL.createObjectURL(new Blob([lines.join('\r\n')], { type: 'text/csv' })); a.download = 'ChartOfAccounts.csv'; a.click();
  }
</script>

<PageHeader title="Chart of accounts" crumbs={[{ label: 'Accounting', href: '/reports' }]}>
  <a class="btn" href="/settings/import?what=accounts"><Icon name="upload" />Import</a>
  <button class="btn" onclick={exportCsv}><Icon name="download" />Export</button>
  {#if can('accounting:write')}<button class="btn btn-primary" onclick={add}><Icon name="plus" />Add account</button>{/if}
</PageHeader>
<Tabs tabs={[{ key: 'ALL', label: 'All accounts' }, { key: 'ASSET', label: 'Assets' }, { key: 'LIABILITY', label: 'Liabilities' }, { key: 'EQUITY', label: 'Equity' }, { key: 'EXPENSE', label: 'Expenses' }, { key: 'REVENUE', label: 'Revenue' }, { key: 'ARCHIVED', label: 'Archived' }]} bind:value={tab} />
<div class="mb-3 flex flex-wrap items-center gap-2">
  <div class="relative w-72"><Icon name="search" class="absolute left-2.5 top-1/2 -translate-y-1/2 text-faint" /><input class="input pl-8" placeholder="Search" bind:value={q} /></div>
  {#if sel.size && can('accounting:write')}
    <span class="ml-2 text-muted">{sel.size} selected</span>
    {#if tab === 'ARCHIVED'}<button class="btn btn-sm" onclick={() => status('ACTIVE')}>Restore</button>{:else}<button class="btn btn-sm" onclick={() => status('ARCHIVED')}>Archive</button>{/if}
    <button class="btn btn-sm btn-danger" onclick={del}>Delete</button>
    <div class="w-48"><Combo bind:value={bulkTax} options={taxOptions()} placeholder="Change tax rate" /></div><button class="btn btn-sm" disabled={!bulkTax} onclick={applyTax}>Apply</button>
  {/if}
</div>
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th class="w-8"></th><th>Code</th><th>Name</th><th>Type</th><th>Tax rate</th><th class="n">YTD balance</th></tr></thead>
    <tbody>{#each shown as r (r.id)}
      <tr class="clickable {sel.has(r.id) ? 'sel' : ''}" onclick={() => edit(r)}>
        <td onclick={(e) => e.stopPropagation()}>{#if !r.system_account}<input type="checkbox" checked={sel.has(r.id)} onchange={() => toggle(r.id)} class="accent-[var(--accent)]" aria-label="Select" />{:else}<span title="System account"><Icon name="lock" size={13} class="text-faint" /></span>{/if}</td>
        <td class="font-medium">{r.code}</td>
        <td><div>{r.name}</div>{#if r.description}<div class="text-[12px] text-faint">{r.description}</div>{/if}</td>
        <td class="text-muted">{typeLabel(r.type)}</td><td class="text-muted">{r.tax_rate_name ?? ''}</td>
        <td class="n"><button class="link" onclick={(e) => { e.stopPropagation(); navigate(`/reports/account-transactions?account_id=${r.id}`); }}>{money(r.class === 'REVENUE' || r.class === 'LIABILITY' || r.class === 'EQUITY' ? -r.ytd_balance : r.ytd_balance)}</button></td>
      </tr>{/each}</tbody></table>
</div>

<Modal bind:open title={editId ? 'Edit account' : 'Add account'}>
  <div class="space-y-3">
    <div><span class="label">Account type</span><select class="select" bind:value={f.type} disabled={!!f.system_account}>{#each TYPES as [k, c, l]}<option value={k}>{l} ({c.toLowerCase()})</option>{/each}</select></div>
    <div class="grid grid-cols-3 gap-3"><div><label class="label" for="ac">Code</label><input id="ac" class="input" bind:value={f.code} maxlength="10" /></div><div class="col-span-2"><label class="label" for="an">Name</label><input id="an" class="input" bind:value={f.name} /></div></div>
    <div><label class="label" for="ad">Description</label><textarea id="ad" rows="2" class="textarea" bind:value={f.description}></textarea></div>
    <div><label class="label" for="cb-chart-default-tax-rate-1">Default tax rate</label><Combo id="cb-chart-default-tax-rate-1" bind:value={f.tax_rate_id} options={taxOptions()} /></div>
    <label class="flex items-center gap-2"><input type="checkbox" bind:checked={f.dashboard_watch} class="accent-[var(--accent)]" />Show on dashboard watchlist</label>
    <label class="flex items-center gap-2"><input type="checkbox" bind:checked={f.show_in_expense_claims} class="accent-[var(--accent)]" />Show in expense claims</label>
    <label class="flex items-center gap-2"><input type="checkbox" bind:checked={f.enable_payments} class="accent-[var(--accent)]" />Enable payments to this account</label>
    {#if f.system_account}<p class="text-[12px] text-muted">This is a system account ({f.system_account}) used automatically by LedgerCraft.</p>{/if}
  </div>
  {#snippet footer()}<button class="btn" onclick={() => (open = false)}>Cancel</button>{#if can('accounting:write')}<button class="btn btn-primary" onclick={save}>Save</button>{/if}{/snippet}
</Modal>
