<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, date } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { can, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Status from '../../components/Status.svelte';
  let runs = $state<any[]>([]);
  let s = $state<any>(null);
  let cal = $state<number | null>(null);
  let next = $state<any>(null);
  let payDate = $state('');
  Promise.all([rpc('payroll.payRuns'), rpc('payroll.settings')]).then(([r, x]) => { runs = r; s = x; cal = x.calendars[0]?.id ?? null; }).catch(toastError);
  $effect(() => { if (cal) rpc('payroll.nextPeriod', { calendar_id: cal }).then((n) => { next = n; payDate = n.to; }); });
  async function create() { try { const r = await rpc('payroll.createPayRun', { calendar_id: cal, payment_date: payDate }); navigate(`/payroll/pay-runs/${r.id}`); } catch (e) { toastError(e); } }
</script>

<PageHeader title="Pay runs" crumbs={[{ label: 'Payroll', href: '/payroll' }]} />
{#if s && can('payroll:write')}
  <div class="card mb-4 flex flex-wrap items-end gap-3 p-4">
    <div><span class="label">Pay calendar</span><select class="select w-52" bind:value={cal}>{#each s.calendars as c}<option value={c.id}>{c.name} ({c.frequency.toLowerCase()})</option>{/each}</select></div>
    {#if next}<div><span class="label">Next period</span><div class="py-2">{date(next.from)} – {date(next.to)}</div></div>{/if}
    <div><label class="label" for="pd">Payment date</label><input id="pd" type="date" class="input" bind:value={payDate} /></div>
    <button class="btn btn-primary" onclick={create}>Add pay run</button>
  </div>
{/if}
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th>Pay run</th><th>Calendar</th><th>Period</th><th>Payment date</th><th class="n">Employees</th><th class="n">Wages</th><th class="n">Net pay</th><th>Status</th></tr></thead>
    <tbody>{#each runs as r}<tr class="clickable" onclick={() => navigate(`/payroll/pay-runs/${r.id}`)}><td>{r.number}</td><td>{r.calendar_name}</td><td>{date(r.period_start)} – {date(r.period_end)}</td><td>{date(r.payment_date)}</td><td class="n">{r.employees}</td><td class="n">{money(r.wages)}</td><td class="n font-medium">{money(r.net_pay)}</td><td><Status status={r.status} /></td></tr>
    {:else}<tr><td colspan="8" class="py-8 text-center text-muted">No pay runs yet.</td></tr>{/each}</tbody></table>
</div>
