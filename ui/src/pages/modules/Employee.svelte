<script lang="ts">
  import { rpc, openView } from '../../lib/api';
  import { money, date, today, moneyPlain, fixed } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { can, toast, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Icon from '../../components/Icon.svelte';
  let { id = 'new' }: { id?: string } = $props();
  const isNew = id === 'new';
  let s = $state<any>(null);
  let e = $state<any>(null);
  let f = $state<any>({ first_name: '', last_name: '', email: '', address: '', start_date: today(), termination_date: '', tax_id: '', tax_code: '', employment_type: 'FULLTIME', pay_calendar_id: null,
    pay_basis: 'SALARY', annual_salary: '', hourly_rate: '', hours_per_week: '38', super_rate: '', bank_account: '', template: [] });
  rpc('payroll.settings').then((x) => { s = x; if (isNew && !f.pay_calendar_id) f.pay_calendar_id = x.calendars[0]?.id ?? null; });
  if (!isNew) rpc('payroll.employee', { id: Number(id) }).then((x) => {
    e = x;
    f = { first_name: x.first_name, last_name: x.last_name, email: x.email, address: x.address, start_date: x.start_date, termination_date: x.termination_date ?? '', tax_id: x.tax_id, tax_code: x.tax_code,
      employment_type: x.employment_type, pay_calendar_id: x.pay_calendar_id, pay_basis: x.pay_basis, annual_salary: moneyPlain(x.annual_salary), hourly_rate: fixed(x.hourly_rate_e2, 4, 2),
      hours_per_week: fixed(x.hours_per_week_e2, 2), super_rate: x.super_rate_bp ? fixed(x.super_rate_bp, 2) : '', bank_account: x.bank_account, template: x.template ?? [] };
  }).catch(toastError);
  async function save() {
    try { const r = await rpc('payroll.saveEmployee', { id: isNew ? null : Number(id), employee: f }); toast('Employee saved'); if (isNew) navigate(`/payroll/employees/${r.id}`, { replace: true }); } catch (x) { toastError(x); }
  }
  const items = $derived((s?.pay_items ?? []).filter((p: any) => p.kind !== 'SUPER'));
</script>

<PageHeader title={isNew ? 'Add employee' : `${f.first_name} ${f.last_name}`} crumbs={[{ label: 'Employees', href: '/payroll/employees' }]} />
{#if e}
  <div class="mb-4 grid gap-3 sm:grid-cols-4">
    <div class="card p-4"><div class="text-[12px] text-muted">YTD gross</div><div class="num text-lg font-semibold">{money(e.ytd.gross)}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">YTD tax</div><div class="num text-lg font-semibold">{money(e.ytd.tax)}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">YTD {s?.retirement_label ?? 'retirement'}</div><div class="num text-lg font-semibold">{money(e.ytd.super)}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">Leave balances</div>{#each e.leave as l}<div class="text-[13px]">{l.name}: <b>{fixed(l.hours_e2, 2, 2)} h</b></div>{/each}</div>
  </div>
{/if}
<div class="grid gap-4 lg:grid-cols-2">
  <section class="card space-y-3 p-5">
    <h2 class="font-semibold">Personal details</h2>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="fn">First name</label><input id="fn" class="input" bind:value={f.first_name} /></div><div><label class="label" for="ln">Last name</label><input id="ln" class="input" bind:value={f.last_name} /></div></div>
    <div><label class="label" for="em">Email (for payslips)</label><input id="em" class="input" bind:value={f.email} /></div>
    <div><label class="label" for="ad">Address</label><textarea id="ad" rows="2" class="textarea" bind:value={f.address}></textarea></div>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="sd">Start date</label><input id="sd" type="date" class="input" bind:value={f.start_date} /></div><div><label class="label" for="td">Termination date</label><input id="td" type="date" class="input" bind:value={f.termination_date} /></div></div>
    <div><label class="label" for="ba">Bank account</label><input id="ba" class="input" bind:value={f.bank_account} /></div>
  </section>
  <section class="card space-y-3 p-5">
    <h2 class="font-semibold">Employment & tax</h2>
    <div class="grid grid-cols-2 gap-3">
      <div><span class="label">Employment basis</span><select class="select" bind:value={f.employment_type}><option value="FULLTIME">Full time</option><option value="PARTTIME">Part time</option><option value="CASUAL">Casual</option><option value="CONTRACTOR">Contractor</option></select></div>
      <div><span class="label">Pay calendar</span><select class="select" bind:value={f.pay_calendar_id}>{#each s?.calendars ?? [] as c}<option value={c.id}>{c.name}</option>{/each}</select></div>
      <div><label class="label" for="ti">Tax ID (TFN / NI / IRD / SSN)</label><input id="ti" class="input" bind:value={f.tax_id} /></div>
      <div><label class="label" for="tc">Tax code / declaration</label><input id="tc" class="input" bind:value={f.tax_code} placeholder={s?.jurisdiction === 'UK' ? '1257L' : s?.jurisdiction === 'AU' ? 'blank = tax-free threshold; NOTFT' : ''} /></div>
      <div><span class="label">Paid by</span><select class="select" bind:value={f.pay_basis}><option value="SALARY">Annual salary</option><option value="HOURLY">Hourly rate</option></select></div>
      {#if f.pay_basis === 'SALARY'}<div><label class="label" for="as">Annual salary</label><input id="as" class="input num" bind:value={f.annual_salary} /></div>
      {:else}<div><label class="label" for="hr">Hourly rate</label><input id="hr" class="input num" bind:value={f.hourly_rate} /></div>{/if}
      <div><label class="label" for="hw">Ordinary hours per week</label><input id="hw" class="input num" bind:value={f.hours_per_week} /></div>
      <div><label class="label" for="sr">{s?.retirement_label ?? 'Retirement'} rate % (blank = default)</label><input id="sr" class="input num" bind:value={f.super_rate} /></div>
    </div>
  </section>
  <section class="card space-y-3 p-5 lg:col-span-2">
    <div class="flex items-center justify-between"><h2 class="font-semibold">Pay template (recurring items each pay)</h2><button class="btn btn-sm" onclick={() => f.template.push({ pay_item_id: items[0]?.id, amount: '', units: '' })}><Icon name="plus" size={14} />Add item</button></div>
    {#each f.template as t, i}
      <div class="grid grid-cols-[2fr_1fr_1fr_auto] gap-2"><select class="select" bind:value={t.pay_item_id}>{#each items as p}<option value={p.id}>{p.name} ({p.kind.toLowerCase()})</option>{/each}</select>
        <input class="input num" placeholder="Units / hours" bind:value={t.units} /><input class="input num" placeholder="Fixed amount" bind:value={t.amount} /><button class="btn btn-ghost btn-sm" onclick={() => f.template.splice(i, 1)} aria-label="Remove"><Icon name="x" size={14} /></button></div>
    {:else}<div class="text-[13px] text-muted">Ordinary earnings are added automatically from the salary or hourly rate (and approved timesheets).</div>{/each}
  </section>
</div>
{#if can('payroll:write')}<div class="mt-4 flex justify-end gap-2"><a class="btn" href="/payroll/employees">Cancel</a><button class="btn btn-primary" onclick={save}>Save</button></div>{/if}
{#if e?.payslips?.length}
  <section class="card mt-4 overflow-x-auto"><div class="px-5 py-3 font-semibold">Payslips</div>
    <table class="tbl text-[13px]"><thead><tr><th>Period end</th><th>Paid</th><th class="n">Gross</th><th class="n">Tax</th><th class="n">Net</th><th></th></tr></thead>
      <tbody>{#each e.payslips as p}<tr><td>{date(p.period_end)}</td><td>{date(p.payment_date)}</td><td class="n">{money(p.gross)}</td><td class="n">{money(p.tax)}</td><td class="n">{money(p.net)}</td><td class="text-right"><button class="btn btn-sm" onclick={() => openView(`/view/payslip/${p.id}`)}>Payslip</button></td></tr>{/each}</tbody></table></section>
{/if}
