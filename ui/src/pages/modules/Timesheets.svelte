<script lang="ts">
  import { rpc } from '../../lib/api';
  import { date, today, addDays, fixed } from '../../lib/format';
  import { can, toast, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Status from '../../components/Status.svelte';
  import Modal from '../../components/Modal.svelte';
  import Icon from '../../components/Icon.svelte';
  let rows = $state<any[]>([]);
  let emps = $state<any[]>([]);
  const load = () => rpc('payroll.timesheets').then((r) => (rows = r)).catch(toastError);
  load();
  rpc('payroll.employees', { status: 'ACTIVE' }).then((e) => (emps = e));
  let open = $state(false);
  const monday = () => { const d = new Date(); d.setDate(d.getDate() - ((d.getDay() + 6) % 7)); return d.toISOString().slice(0, 10); };
  let f = $state<any>({ employee_id: null, period_start: monday(), days: Array(7).fill('') });
  async function save() {
    const lines = f.days.map((h: string, i: number) => ({ date: addDays(f.period_start, i), hours: h || '0' }));
    try { await rpc('payroll.saveTimesheet', { timesheet: { employee_id: f.employee_id, period_start: f.period_start, period_end: addDays(f.period_start, 6), lines } }); open = false; toast('Timesheet saved'); load(); } catch (e) { toastError(e); }
  }
  async function approve(t: any) { await rpc('payroll.timesheetStatus', { id: t.id, status: 'APPROVED' }); load(); }
  const DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
</script>

<PageHeader title="Timesheets" crumbs={[{ label: 'Payroll', href: '/payroll' }]}>
  {#if can('payroll:write')}<button class="btn btn-primary" onclick={() => { f = { employee_id: emps[0]?.id ?? null, period_start: monday(), days: Array(7).fill('') }; open = true; }}><Icon name="plus" />New timesheet</button>{/if}
</PageHeader>
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th>Employee</th><th>Week</th><th class="n">Hours</th><th>Status</th><th></th></tr></thead>
    <tbody>{#each rows as t}<tr><td>{t.employee}</td><td>{date(t.period_start)} – {date(t.period_end)}</td><td class="n">{fixed(t.total_hours_e2, 2, 2)}</td><td><Status status={t.status} /></td>
      <td class="text-right">{#if t.status === 'DRAFT' && can('payroll:write')}<button class="btn btn-sm" onclick={() => approve(t)}>Approve</button>{/if}</td></tr>
    {:else}<tr><td colspan="5" class="py-8 text-center text-muted">No timesheets. Approved timesheets set the hours paid to hourly employees in the next pay run.</td></tr>{/each}</tbody></table>
</div>
<Modal bind:open title="Timesheet" width={720}>
  <div class="mb-3 grid grid-cols-2 gap-3"><div><span class="label">Employee</span><select class="select" bind:value={f.employee_id}>{#each emps as e}<option value={e.id}>{e.first_name} {e.last_name}</option>{/each}</select></div>
    <div><label class="label" for="ws">Week starting</label><input id="ws" type="date" class="input" bind:value={f.period_start} /></div></div>
  <div class="grid grid-cols-7 gap-2">{#each DAYS as d, i}<div><div class="label text-center">{d}</div><input class="input num text-center" bind:value={f.days[i]} placeholder="0" /></div>{/each}</div>
  {#snippet footer()}<button class="btn" onclick={() => (open = false)}>Cancel</button><button class="btn btn-primary" onclick={save}>Save</button>{/snippet}
</Modal>
