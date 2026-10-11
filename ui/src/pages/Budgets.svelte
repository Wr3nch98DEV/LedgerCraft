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
  const rowTotal = (a: number) => months.reduce((s, m) => s + Math.round(Number(grid[`${a}|${m}`] || 0) * 100), 0);
  const trackingOpts = $derived(lookups.tracking.flatMap((c) => c.options.map((o: any) => ({ value: o.id, label: o.name, group: c.name }))));
</script>

<PageHeader title="Budget manager" crumbs={[{ label: 'Accounting', href: '/reports' }]}>
  <a class="btn" href="/reports/budget-variance">Budget variance report</a>
  {#if can('accounting:write')}<button class="btn" onclick={newBudget}><Icon name="plus" />New budget</button><button class="btn btn-primary" onclick={save}>Save</button>{/if}
</PageHeader>
<div class="card mb-4 flex flex-wrap items-end gap-3 p-4">
  <div><span class="label">Budget</span><select class="select w-56" value={current} onchange={(e) => open(Number((e.target as HTMLSelectElement).value))}>{#each budgets as b}<option value={b.id}>{b.name}</option>{/each}{#if current === null}<option value={null}>(new)</option>{/if}</select></div>
  <div><label class="label" for="bn">Name</label><input id="bn" class="input w-56" bind:value={name} /></div>
  {#if trackingOpts.length}<div class="w-56"><span class="label">For tracking option</span><Combo bind:value={tracking} options={trackingOpts} placeholder="Overall budget" /></div>{/if}
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
