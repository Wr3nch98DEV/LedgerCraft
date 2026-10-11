<script lang="ts">
  import { rpc, openView } from '../../lib/api';
  import { money, date, moneyPlain, fixed } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { can, confirm, toast, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Status from '../../components/Status.svelte';
  import Combo from '../../components/Combo.svelte';
  import Icon from '../../components/Icon.svelte';
  import { bankAccounts } from '../../components/Pickers';
  let { id }: { id: string } = $props();
  let r = $state<any>(null);
  let items = $state<any[]>([]);
  let bank = $state<number | null>(bankAccounts()[0]?.value ?? null);
  let editing = $state<number | null>(null);
  let lines = $state<any[]>([]);
  const load = () => rpc('payroll.payRun', { id: Number(id) }).then((x) => (r = x)).catch(toastError);
  load();
  rpc('payroll.settings').then((s) => (items = s.pay_items.filter((p: any) => p.kind !== 'SUPER')));
  function edit(s: any) { editing = s.id; lines = s.lines.filter((l: any) => !l.auto && !['TAX', 'SUPER', 'EMPLOYERTAX'].includes(l.kind)).map((l: any) => ({ ...l, amount: moneyPlain(l.amount) })); }
  async function saveSlip() { try { await rpc('payroll.updatePayslip', { id: editing, lines }); editing = null; toast('Payslip updated'); load(); } catch (e) { toastError(e); } }
  function addLine() { const p = items[0]; lines.push({ pay_item_id: p?.id, name: p?.name, kind: p?.kind, amount: '' }); }
  async function post() {
    if (!(await confirm('Post this pay run?', `Wages are posted to the ledger${bank ? ' and net pay is recorded as paid from the selected bank account' : ''}. Leave balances are updated.`, 'Post pay run'))) return;
    try { await rpc('payroll.post', { id: Number(id), bank_account_id: bank }); toast('Pay run posted'); load(); } catch (e) { toastError(e); }
  }
  async function del() { if (await confirm('Delete this draft pay run?', '', 'Delete', true)) { await rpc('payroll.deletePayRun', { id: Number(id) }); navigate('/payroll/pay-runs'); } }
  async function email() { const x = await rpc('payroll.emailPayslips', { id: Number(id) }); toast(`${x.queued} payslip email(s) queued`); }
</script>

{#if r}
<PageHeader title="Pay run {r.number}" crumbs={[{ label: 'Pay runs', href: '/payroll/pay-runs' }]}>
  {#snippet sub()}{r.calendar_name} · {date(r.period_start)} – {date(r.period_end)} · paid {date(r.payment_date)} · <Status status={r.status} />{/snippet}
  <a class="btn" href="/download/payroll-filing?id={r.id}" download title="Requires government developer registration — export file only"><Icon name="download" />Filing export</a>
  {#if r.status === 'POSTED'}<button class="btn" onclick={email}><Icon name="mail" />Email payslips</button>{/if}
  {#if r.status === 'DRAFT' && can('payroll:write')}<button class="btn btn-danger" onclick={del}>Delete</button>{/if}
</PageHeader>
<div class="mb-4 grid gap-3 sm:grid-cols-5">
  {#each [['Wages', r.wages], ['Tax', r.tax], ['Deductions', r.deductions], ['Employer retirement', r.super], ['Net pay', r.net_pay]] as [l, v]}<div class="card p-4"><div class="text-[12px] text-muted">{l}</div><div class="num text-lg font-semibold">{money(v as number)}</div></div>{/each}
</div>
<div class="mb-4 rounded-lg border border-warn/30 bg-warn-soft p-3 text-[13px]">{r.disclaimer}</div>
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th>Employee</th><th class="n">Earnings</th><th class="n">Tax</th><th class="n">Deductions</th><th class="n">Reimbursements</th><th class="n">Retirement</th><th class="n">Net pay</th><th></th></tr></thead>
    <tbody>{#each r.payslips as s}
      <tr><td class="font-medium">{s.employee}</td><td class="n">{money(s.gross)}</td><td class="n">{money(s.tax)}</td><td class="n">{money(s.deductions)}</td><td class="n">{money(s.reimbursements)}</td><td class="n">{money(s.super)}</td><td class="n font-semibold">{money(s.net)}</td>
        <td class="whitespace-nowrap text-right">{#if r.status === 'DRAFT' && can('payroll:write')}<button class="btn btn-sm" onclick={() => edit(s)}>Edit</button>{/if} <button class="btn btn-sm" onclick={() => openView(`/view/payslip/${s.id}`)}>Payslip</button></td></tr>
      {#if editing === s.id}
        <tr><td colspan="8" class="bg-bg2">
          {#each lines as l, i}<div class="mb-2 grid grid-cols-[2fr_1fr_auto] gap-2"><select class="select" value={l.pay_item_id} onchange={(e) => { const p = items.find((x) => x.id === Number((e.target as HTMLSelectElement).value)); Object.assign(l, { pay_item_id: p.id, name: p.name, kind: p.kind }); }}>{#each items as p}<option value={p.id}>{p.name} ({p.kind.toLowerCase()})</option>{/each}</select>
            <input class="input num" bind:value={l.amount} placeholder="Amount" /><button class="btn btn-ghost btn-sm" onclick={() => lines.splice(i, 1)} aria-label="Remove"><Icon name="x" size={14} /></button></div>{/each}
          <div class="flex gap-2"><button class="btn btn-sm" onclick={addLine}><Icon name="plus" size={14} />Add line</button><button class="btn btn-sm btn-primary" onclick={saveSlip}>Save & recalculate</button><button class="btn btn-sm" onclick={() => (editing = null)}>Cancel</button></div>
          <div class="mt-2 text-[12px] text-muted">Tax, retirement contributions and leave accrual are recalculated automatically.</div>
        </td></tr>
      {/if}
      <tr class="text-[12px] text-muted"><td colspan="8" class="!pt-0">{s.lines.map((l: any) => `${l.name} ${money(l.amount)}`).join(' · ')}{#each s.leave as lv} · {lv.name} +{fixed(lv.accrued_e2, 2, 2)}h{/each}</td></tr>
    {/each}</tbody></table>
</div>
{#if r.status === 'DRAFT' && can('payroll:write')}
  <div class="mt-4 flex flex-wrap items-end justify-end gap-3"><div class="w-64"><label class="label" for="cb-payrun-pay-net-wages-from-1">Pay net wages from</label><Combo id="cb-payrun-pay-net-wages-from-1" bind:value={bank} options={bankAccounts()} placeholder="Don't record payment" /></div><button class="btn btn-primary" onclick={post}>Post pay run</button></div>
{/if}
{/if}
