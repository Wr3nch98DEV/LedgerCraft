<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, date } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { can, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Tabs from '../../components/Tabs.svelte';
  import Icon from '../../components/Icon.svelte';
  let rows = $state<any[]>([]);
  let tab = $state('ACTIVE');
  $effect(() => { rpc('payroll.employees', { status: tab }).then((r) => (rows = r)).catch(toastError); });
</script>

<PageHeader title="Employees" crumbs={[{ label: 'Payroll', href: '/payroll' }]}>
  {#if can('payroll:write')}<a class="btn btn-primary" href="/payroll/employees/new"><Icon name="plus" />Add employee</a>{/if}
</PageHeader>
<Tabs tabs={[{ key: 'ACTIVE', label: 'Current' }, { key: 'TERMINATED', label: 'Terminated' }, { key: 'ALL', label: 'All' }]} bind:value={tab} />
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th>Name</th><th>Employment</th><th>Pay calendar</th><th>Pay basis</th><th class="n">Salary / rate</th><th>Start date</th></tr></thead>
    <tbody>{#each rows as e}<tr class="clickable" onclick={() => navigate(`/payroll/employees/${e.id}`)}><td class="font-medium">{e.first_name} {e.last_name}</td><td class="text-muted">{e.employment_type.toLowerCase()}</td><td>{e.calendar_name ?? '—'}</td>
      <td>{e.pay_basis === 'HOURLY' ? 'Hourly' : 'Salary'}</td><td class="n">{e.pay_basis === 'HOURLY' ? money(e.hourly_rate_e2 / 100) + ' /h' : money(e.annual_salary) + ' p.a.'}</td><td>{date(e.start_date)}</td></tr>
    {:else}<tr><td colspan="6" class="py-10 text-center text-muted">No employees.</td></tr>{/each}</tbody></table>
</div>
