<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, addMonths, moneyPlain } from '../lib/format';
  import { lookups, setting, can, toast, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Combo from '../components/Combo.svelte';
  import Icon from '../components/Icon.svelte';
  let budgets = $state<any[]>([]);
  let current = $state<number | null>(null);
  let name = $state('');
  let tracking = $state<number | null>(null);
  let grid = $state<Record<string, string>>({});
  const fyEnd = Number(setting('fy_end_month', '6'));
  let year = $state((() => { const d = new Date(); const sm = (fyEnd % 12) + 1; const y = d.getMonth() + 1 >= sm ? d.getFullYear() : d.getFullYear() - 1; return `${y}-${String(sm).padStart(2, '0')}-01`; })());
  const months = $derived(Array.from({ length: 12 }, (_, i) => addMonths(year, i).slice(0, 7)));
  const plAccounts = $derived(lookups.accounts.filter((a) => (a.class === 'REVENUE' || a.class === 'EXPENSE') && a.status === 'ACTIVE'));
  const load = () => rpc('budgets.list').then((b) => { budgets = b; if (!current && b.length) open(b[0].id); });
  load();
  async function open(id: number) {
    current = id;
    const b = await rpc('budgets.get', { id });
    name = b.name; tracking = b.tracking_option_id; grid = {};
    for (const l of b.lines) grid[`${l.account_id}|${l.period}`] = moneyPlain(l.amount);
  }
  function newBudget() { current = null; name = 'New budget'; tracking = null; grid = {}; }
  function fill(acct: number) { const first = grid[`${acct}|${months[0]}`]; if (first) for (const m of months) grid[`${acct}|${m}`] = first; }
  async function save() {
    const lines = Object.entries(grid).filter(([, v]) => v !== '' && v !== undefined).map(([k, v]) => { const [a, p] = k.split('|'); return { account_id: Number(a), period: p, amount: v }; });
    try { const r = await rpc('budgets.save', { id: current, name, tracking_option_id: tracking, lines }); toast('Budget saved'); current = r.id; load(); } catch (e) { toastError(e); }
  }
  async function importCsv(file: File) {
    const text = await file.text();
    const rows = text.split(/\r?\n/).map((l) => l.split(',').map((c) => c.replace(/^"|"$/g, '').trim())).filter((r) => r.length > 1);
    let n = 0;
    for (const r of rows) {
      const code = r[0].split(' ')[0];
      const a = lookups.accounts.find((x) => x.code === code || x.name.toLowerCase() === r[0].toLowerCase());
      if (!a) continue;
      months.forEach((m, i) => { const v = (r[i + 1] ?? '').replace(/[$,]/g, ''); if (v !== '' && !isNaN(Number(v))) grid[`${a.id}|${m}`] = v; });
      n++;
    }
    toast(`Imported ${n} account row(s) — review and Save`);
  }
  function exportCsv() {
    const lines = ['Account,' + months.join(',')];
    for (const a of plAccounts) { const vals = months.map((m) => grid[`${a.id}|${m}`] ?? ''); if (vals.some((v) => v !== '')) lines.push(`"${a.code} ${a.name}",` + vals.join(',')); }
    const el = document.createElement('a'); el.href = URL.createObjectURL(new Blob([lines.join('\r\n')], { type: 'text/csv' })); el.download = `${name}.csv`; el.click();
  }
  const rowTotal = (a: number) => months.reduce((s, m) => s + Math.round(Number(grid[`${a}|${m}`] || 0) * 100), 0);
  const trackingOpts = $derived(lookups.tracking.flatMap((c) => c.options.map((o: any) => ({ value: o.id, label: o.name, group: c.name }))));
</script>

<PageHeader title="Budget manager" crumbs={[{ label: 'Accounting', href: '/reports' }]}>
  <a class="btn" href="/reports/budget-variance">Budget variance report</a>
  <button class="btn" onclick={exportCsv}><Icon name="download" />Export CSV</button>
  {#if can('accounting:write')}<label class="btn cursor-pointer"><Icon name="upload" />Import CSV<input type="file" accept=".csv" class="hidden" onchange={(e) => { const f = (e.target as HTMLInputElement).files?.[0]; if (f) importCsv(f); }} /></label><button class="btn" onclick={newBudget}><Icon name="plus" />New budget</button><button class="btn btn-primary" onclick={save}>Save</button>{/if}
</PageHeader>
<div class="card mb-4 flex flex-wrap items-end gap-3 p-4">
  <div><span class="label">Budget</span><select class="select w-56" value={current} onchange={(e) => open(Number((e.target as HTMLSelectElement).value))}>{#each budgets as b}<option value={b.id}>{b.name}</option>{/each}{#if current === null}<option value={null}>(new)</option>{/if}</select></div>
  <div><label class="label" for="bn">Name</label><input id="bn" class="input w-56" bind:value={name} /></div>
  {#if trackingOpts.length}<div class="w-56"><label class="label" for="cb-budgets-for-tracking-option-1">For tracking option</label><Combo id="cb-budgets-for-tracking-option-1" bind:value={tracking} options={trackingOpts} placeholder="Overall budget" /></div>{/if}
  <div><label class="label" for="by">Financial year starting</label><input id="by" type="month" class="input" value={year.slice(0, 7)} onchange={(e) => (year = (e.target as HTMLInputElement).value + '-01')} /></div>
</div>
<div class="card overflow-x-auto">
  <table class="tbl min-w-[1400px] text-[13px]">
    <thead><tr><th class="sticky left-0 z-10 min-w-[220px] bg-panel">Account</th>{#each months as m}<th class="!text-right">{m}</th>{/each}<th class="!text-right">Total</th></tr></thead>
    <tbody>{#each plAccounts as a}
      <tr><td class="sticky left-0 bg-panel">{a.code} {a.name}<button class="ml-1 text-[11px] text-faint hover:text-accent" title="Copy first month across" onclick={() => fill(a.id)}>→</button></td>
        {#each months as m}<td class="!p-1"><input class="grid-in num w-24 text-right" bind:value={grid[`${a.id}|${m}`]} /></td>{/each}
        <td class="n font-medium">{money(rowTotal(a.id), { blankZero: true })}</td></tr>{/each}</tbody>
  </table>
</div>
