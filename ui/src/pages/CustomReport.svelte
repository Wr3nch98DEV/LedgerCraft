<script lang="ts">
  import { rpc } from '../lib/api';
  import { today } from '../lib/format';
  import { navigate } from '../lib/router.svelte';
  import { confirm, toast, toastError, setting } from '../lib/stores.svelte';
  import { exportCsv, exportExcel } from '../lib/exporting';
  import PageHeader from '../components/PageHeader.svelte';
  import ReportTable from '../components/ReportTable.svelte';
  import Combo from '../components/Combo.svelte';
  import Icon from '../components/Icon.svelte';
  import { accountOptions } from '../components/Pickers';

  let { id = 'new' }: { id?: string } = $props();
  const isNew = id === 'new';
  const fyEnd = Number(setting('fy_end_month', '6'));
  const fyStart = (() => { const d = new Date(); const sm = (fyEnd % 12) + 1; const y = d.getMonth() + 1 >= sm ? d.getFullYear() : d.getFullYear() - 1; return `${y}-${String(sm).padStart(2, '0')}-01`; })();
  let name = $state('My management report');
  let published = $state(false);
  let def = $state<any>({ from: fyStart, to: today(), compare: 0, period: 'MONTH', monthly: true, basis: 'ACCRUAL', balance: false, rows: [
    { type: 'heading', label: 'Income' },
    { type: 'types', label: 'Revenue', types: ['REVENUE', 'SALES'], sign: -1 },
    { type: 'types', label: 'Cost of sales', types: ['DIRECTCOSTS'], sign: 1 },
    { type: 'formula', label: 'Gross profit', expr: 'r2 - r3' },
    { type: 'formula', label: 'Gross margin %', expr: 'r4 / r2 * 100', percent: true },
    { type: 'types', label: 'Operating expenses', types: ['EXPENSE', 'OVERHEADS', 'DEPRECIATN'], sign: 1 },
    { type: 'formula', label: 'Operating profit', expr: 'r4 - r6' },
  ] });
  let report = $state<any>(null);
  if (!isNew) rpc('layouts.list').then((ls) => { const l = ls.find((x: any) => x.id === Number(id)); if (l) { name = l.name; def = { ...def, ...l.definition }; published = !!l.published; } preview(); });
  else preview();
  async function preview() { try { report = isNew || !published ? await rpc('layouts.preview', { kind: 'CUSTOM', name, definition: def }) : await rpc('layouts.run', { id: Number(id) }); } catch (e) { toastError(e); } }
  async function save() { try { const r = await rpc('layouts.save', { id: isNew ? null : Number(id), name, kind: 'CUSTOM', definition: def }); toast('Report saved'); if (isNew) navigate(`/reports/custom/${r.id}`, { replace: true }); } catch (e) { toastError(e); } }
  async function publish(p: boolean) { try { await rpc('layouts.publish', { id: Number(id), publish: p }); published = p; toast(p ? 'Published — figures are now locked' : 'Unpublished'); preview(); } catch (e) { toastError(e); } }
  async function del() { if (await confirm('Delete this report?', '', 'Delete', true)) { await rpc('layouts.delete', { id: Number(id) }); navigate('/reports'); } }
  const TYPES: [string, string][] = [['REVENUE', 'Revenue'], ['SALES', 'Sales'], ['OTHERINCOME', 'Other income'], ['DIRECTCOSTS', 'Direct costs'], ['EXPENSE', 'Expense'], ['OVERHEADS', 'Overheads'], ['DEPRECIATN', 'Depreciation'],
    ['BANK', 'Bank'], ['CURRENT', 'Current assets'], ['FIXED', 'Fixed assets'], ['CURRLIAB', 'Current liabilities'], ['TERMLIAB', 'Non-current liabilities'], ['EQUITY', 'Equity'],
    ['ASSET', 'All assets'], ['LIABILITY', 'All liabilities'], ['EXPENSE', 'All expenses (class)']];
  function add(type: string) { def.rows.push(type === 'formula' ? { type, label: 'New calculation', expr: '' } : type === 'account' ? { type, label: 'Selected accounts', account_ids: [], sign: 1 } : type === 'types' ? { type, label: 'Account types', types: [], sign: 1 } : type === 'heading' ? { type, label: 'Heading' } : { type, text: 'Commentary…' }); }
  function move(i: number, d: number) { const j = i + d; if (j < 0 || j >= def.rows.length) return; const t = def.rows[i]; def.rows[i] = def.rows[j]; def.rows[j] = t; }
</script>

<PageHeader title={name} crumbs={[{ label: 'Reports', href: '/reports' }]}>
  {#snippet sub()}{published ? 'Published (locked)' : 'Draft custom report'}{/snippet}
  {#if report}<button class="btn" onclick={() => window.print()}><Icon name="print" />Print</button><button class="btn" onclick={() => exportExcel(report)}>Excel</button><button class="btn" onclick={() => exportCsv(report)}>CSV</button>{/if}
  {#if !isNew}{#if published}<button class="btn" onclick={() => publish(false)}>Unpublish</button>{:else}<button class="btn" onclick={() => publish(true)}><Icon name="lock" />Publish</button>{/if}<button class="btn btn-danger" onclick={del}>Delete</button>{/if}
  {#if !published}<button class="btn btn-primary" onclick={save}>Save</button>{/if}
</PageHeader>

{#if !published}
<div class="no-print grid gap-4 xl:grid-cols-[460px_1fr]">
  <section class="card space-y-3 p-4">
    <div><label class="label" for="rn">Report name</label><input id="rn" class="input" bind:value={name} /></div>
    <div class="grid grid-cols-2 gap-2"><div><label class="label" for="rf">From</label><input id="rf" type="date" class="input" bind:value={def.from} /></div><div><label class="label" for="rt">To</label><input id="rt" type="date" class="input" bind:value={def.to} /></div></div>
    <div class="flex flex-wrap items-center gap-3 text-[13px]"><label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={def.monthly} class="accent-[var(--accent)]" />Monthly columns</label>
      <label class="flex items-center gap-1.5"><input type="checkbox" bind:checked={def.balance} class="accent-[var(--accent)]" />Balances (as at) instead of movements</label>
      <select class="select !w-28 !py-1" bind:value={def.basis}><option value="ACCRUAL">Accrual</option><option value="CASH">Cash</option></select></div>
    <div class="text-[12px] font-semibold uppercase tracking-wide text-faint">Rows</div>
    {#each def.rows as r, i}
      <div class="rounded-lg border border-line p-2">
        <div class="mb-1.5 flex items-center gap-2"><span class="chip bg-accent-soft text-accent">r{i + 1}</span><span class="text-[12px] text-muted">{r.type}</span>
          <span class="ml-auto flex gap-1"><button class="btn btn-ghost btn-sm" onclick={() => move(i, -1)} aria-label="Up">↑</button><button class="btn btn-ghost btn-sm" onclick={() => move(i, 1)} aria-label="Down">↓</button><button class="btn btn-ghost btn-sm" onclick={() => def.rows.splice(i, 1)} aria-label="Remove"><Icon name="x" size={13} /></button></span></div>
        {#if r.type === 'text'}<textarea class="textarea" rows="2" bind:value={r.text}></textarea>
        {:else}<input class="input mb-1.5" bind:value={r.label} placeholder="Label" />{/if}
        {#if r.type === 'formula'}<input class="input font-mono" bind:value={r.expr} placeholder="e.g. r2 - r3" /><label class="mt-1 flex items-center gap-1.5 text-[12px]"><input type="checkbox" bind:checked={r.percent} class="accent-[var(--accent)]" />Show as percentage</label>{/if}
        {#if r.type === 'types'}<div class="flex flex-wrap gap-1">{#each TYPES as [k, l]}<button class="chip {r.types.includes(k) ? 'bg-accent text-accent-ink' : 'bg-hover text-muted'}" onclick={() => (r.types = r.types.includes(k) ? r.types.filter((x: string) => x !== k) : [...r.types, k])}>{l}</button>{/each}</div>{/if}
        {#if r.type === 'account'}
          <div class="flex flex-wrap gap-1">{#each r.account_ids as aid, j}<span class="chip bg-hover">{accountOptions().find((o) => o.value === aid)?.label}<button onclick={() => r.account_ids.splice(j, 1)} aria-label="Remove">×</button></span>{/each}</div>
          <Combo value={null} options={accountOptions()} placeholder="Add account" onchange={(v) => { if (v && !r.account_ids.includes(v)) r.account_ids.push(v); }} />
        {/if}
        {#if r.type === 'types' || r.type === 'account'}<label class="mt-1 flex items-center gap-1.5 text-[12px]"><input type="checkbox" checked={r.sign === -1} onchange={(e) => (r.sign = (e.target as HTMLInputElement).checked ? -1 : 1)} class="accent-[var(--accent)]" />Show credits as positive (income, liabilities, equity)</label>{/if}
      </div>
    {/each}
    <div class="flex flex-wrap gap-1"><button class="btn btn-sm" onclick={() => add('types')}>+ Account types</button><button class="btn btn-sm" onclick={() => add('account')}>+ Accounts</button><button class="btn btn-sm" onclick={() => add('formula')}>+ Formula</button><button class="btn btn-sm" onclick={() => add('heading')}>+ Heading</button><button class="btn btn-sm" onclick={() => add('text')}>+ Text</button></div>
    <button class="btn btn-primary w-full justify-center" onclick={preview}>Update preview</button>
  </section>
  <section class="card p-5">{#if report}<div class="mb-3 text-center"><div class="text-lg font-semibold">{report.title}</div><div class="text-[13px] text-muted">{report.org} · {report.subtitle}</div></div><ReportTable {report} />{/if}</section>
</div>
{:else if report}
  <section class="card p-5"><div class="mb-3 text-center"><div class="text-lg font-semibold">{report.title}</div><div class="text-[13px] text-muted">{report.org} · {report.subtitle} · published</div></div><ReportTable {report} /></section>
{/if}
