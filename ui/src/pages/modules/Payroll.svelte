<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, date } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Status from '../../components/Status.svelte';
  import Icon from '../../components/Icon.svelte';
  let s = $state<any>(null), runs = $state<any[]>([]), emps = $state<any[]>([]), leave = $state<any>(null);
  Promise.all([rpc('payroll.settings'), rpc('payroll.payRuns'), rpc('payroll.employees', { status: 'ACTIVE' }), rpc('payroll.leave')])
    .then(([a, b, c, d]) => { s = a; runs = b; emps = c; leave = d; }).catch(toastError);
  const pendingLeave = $derived((leave?.requests ?? []).filter((r: any) => r.status === 'PENDING').length);
</script>

<PageHeader title="Payroll">
  <a class="btn" href="/payroll/employees"><Icon name="users" />Employees</a>
  <a class="btn btn-primary" href="/payroll/pay-runs"><Icon name="dollar" />Pay employees</a>
</PageHeader>
{#if s}
  <div class="mb-4 rounded-lg border border-warn/30 bg-warn-soft p-3 text-[13px]">{s.jurisdiction_name}. {s.disclaimer}</div>
  <div class="grid gap-4 md:grid-cols-3">
    <section class="card p-5"><div class="text-[12px] text-muted">Active employees</div><div class="text-2xl font-semibold">{emps.length}</div><a class="link text-[13px]" href="/payroll/employees">Manage employees</a></section>
    <section class="card p-5"><div class="text-[12px] text-muted">Last pay run</div>{#if runs[0]}<div class="text-2xl font-semibold">{money(runs[0].net_pay)}</div><div class="text-[13px] text-muted">{date(runs[0].payment_date)} · <Status status={runs[0].status} /></div>{:else}<div class="text-muted">None yet</div>{/if}</section>
    <section class="card p-5"><div class="text-[12px] text-muted">Leave requests to approve</div><div class="text-2xl font-semibold">{pendingLeave}</div><a class="link text-[13px]" href="/payroll/leave">Review leave</a></section>
  </div>
  <section class="card mt-4 overflow-x-auto">
    <div class="px-5 py-3 font-semibold">Recent pay runs</div>
    <table class="tbl"><thead><tr><th>Pay run</th><th>Period</th><th>Payment date</th><th class="n">Wages</th><th class="n">Tax</th><th class="n">{s.retirement_label}</th><th class="n">Net pay</th><th>Status</th></tr></thead>
      <tbody>{#each runs.slice(0, 8) as r}<tr class="clickable" onclick={() => navigate(`/payroll/pay-runs/${r.id}`)}><td>{r.number} · {r.calendar_name}</td><td>{date(r.period_start)} – {date(r.period_end)}</td><td>{date(r.payment_date)}</td>
        <td class="n">{money(r.wages)}</td><td class="n">{money(r.tax)}</td><td class="n">{money(r.super)}</td><td class="n font-medium">{money(r.net_pay)}</td><td><Status status={r.status} /></td></tr>
      {:else}<tr><td colspan="8" class="py-8 text-center text-muted">No pay runs yet.</td></tr>{/each}</tbody></table>
  </section>
{/if}
