<script lang="ts">
  import { rpc } from '../../lib/api';
  import { date, today, fixed } from '../../lib/format';
  import { can, toast, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Status from '../../components/Status.svelte';
  import Modal from '../../components/Modal.svelte';
  import Icon from '../../components/Icon.svelte';
  let d = $state<any>(null);
  let s = $state<any>(null);
  let emps = $state<any[]>([]);
  const load = () => rpc('payroll.leave').then((r) => (d = r)).catch(toastError);
  load();
  rpc('payroll.settings').then((x) => (s = x));
  rpc('payroll.employees', { status: 'ACTIVE' }).then((e) => (emps = e));
  let open = $state(false);
  let f = $state<any>({ employee_id: null, leave_type_id: null, start_date: today(), end_date: today(), hours: '', description: '' });
  async function save() { try { await rpc('payroll.saveLeave', { request: f }); open = false; toast('Leave request added'); load(); } catch (e) { toastError(e); } }
  async function status(r: any, st: string) { await rpc('payroll.leaveStatus', { id: r.id, status: st }); load(); }
</script>

<PageHeader title="Leave" crumbs={[{ label: 'Payroll', href: '/payroll' }]}>
  {#if can('payroll:write')}<button class="btn btn-primary" onclick={() => { f = { employee_id: emps[0]?.id ?? null, leave_type_id: s?.leave_types[0]?.id ?? null, start_date: today(), end_date: today(), hours: '7.6', description: '' }; open = true; }}><Icon name="plus" />New leave request</button>{/if}
</PageHeader>
{#if d}
  <div class="grid gap-4 lg:grid-cols-[1fr_380px]">
    <div class="card overflow-x-auto"><div class="px-5 py-3 font-semibold">Requests</div>
      <table class="tbl"><thead><tr><th>Employee</th><th>Type</th><th>Dates</th><th class="n">Hours</th><th>Status</th><th></th></tr></thead>
        <tbody>{#each d.requests as r}<tr><td>{r.employee}</td><td>{r.leave_type}</td><td>{date(r.start_date)} – {date(r.end_date)}</td><td class="n">{fixed(r.hours_e2, 2, 2)}</td><td><Status status={r.status} /></td>
          <td class="whitespace-nowrap text-right">{#if r.status === 'PENDING' && can('payroll:write')}<button class="btn btn-sm btn-primary" onclick={() => status(r, 'APPROVED')}>Approve</button> <button class="btn btn-sm" onclick={() => status(r, 'DECLINED')}>Decline</button>{/if}</td></tr>
        {:else}<tr><td colspan="6" class="py-8 text-center text-muted">No leave requests.</td></tr>{/each}</tbody></table></div>
    <div class="card h-fit p-5"><div class="mb-2 font-semibold">Balances</div>
      {#each d.balances as b}<div class="flex justify-between border-t border-line py-1.5 text-[13px] first:border-0"><span>{b.employee} · {b.leave_type}</span><span class="num font-medium">{fixed(b.hours_e2, 2, 2)} h</span></div>{/each}
      <p class="mt-2 text-[12px] text-muted">Leave accrues with each posted pay run; approved leave is deducted when it's paid.</p></div>
  </div>
{/if}
<Modal bind:open title="Leave request">
  <div class="space-y-3">
    <div class="grid grid-cols-2 gap-3"><div><span class="label">Employee</span><select class="select" bind:value={f.employee_id}>{#each emps as e}<option value={e.id}>{e.first_name} {e.last_name}</option>{/each}</select></div>
      <div><span class="label">Leave type</span><select class="select" bind:value={f.leave_type_id}>{#each s?.leave_types ?? [] as t}<option value={t.id}>{t.name}</option>{/each}</select></div></div>
    <div class="grid grid-cols-3 gap-3"><div><label class="label" for="ls">From</label><input id="ls" type="date" class="input" bind:value={f.start_date} /></div><div><label class="label" for="le">To</label><input id="le" type="date" class="input" bind:value={f.end_date} /></div><div><label class="label" for="lh">Hours</label><input id="lh" class="input num" bind:value={f.hours} /></div></div>
    <div><label class="label" for="ld">Description</label><input id="ld" class="input" bind:value={f.description} /></div>
  </div>
  {#snippet footer()}<button class="btn" onclick={() => (open = false)}>Cancel</button><button class="btn btn-primary" onclick={save}>Save</button>{/snippet}
</Modal>
