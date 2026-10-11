<script lang="ts">
  import { rpc } from '../../lib/api';
  import { fixed, date } from '../../lib/format';
  import { can, toast, toastError, loadSession } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Modal from '../../components/Modal.svelte';
  import Combo from '../../components/Combo.svelte';
  import Icon from '../../components/Icon.svelte';
  import { accountOptions } from '../../components/Pickers';
  let s = $state<any>(null);
  const load = () => rpc('payroll.settings').then((x) => { s = x; brackets = x.brackets; }).catch(toastError);
  load();
  let brackets = $state('');
  async function setJur(j: string) { await rpc('settings.update', { payroll_jurisdiction: j }); await loadSession(); load(); toast('Payroll jurisdiction updated'); }
  async function saveBrackets() { try { JSON.parse(brackets); await rpc('settings.update', { payroll_brackets: brackets }); toast('Saved'); } catch (e) { toastError('Brackets must be JSON like [[20000, 0], [60000, 20], [null, 30]]'); } }
  let itemOpen = $state(false);
  let it = $state<any>({});
  async function saveItem() { try { await rpc('payroll.savePayItem', { id: it.id ?? null, item: it }); itemOpen = false; load(); } catch (e) { toastError(e); } }
  let cal = $state<any>({ open: false, name: '', frequency: 'FORTNIGHTLY', start_date: '' });
  async function saveCal() { try { await rpc('payroll.saveCalendar', { id: cal.id ?? null, name: cal.name, frequency: cal.frequency, start_date: cal.start_date }); cal.open = false; load(); } catch (e) { toastError(e); } }
  let lt = $state<any>({ open: false, name: '', hours: '' });
  async function saveLt() { try { await rpc('payroll.saveLeaveType', { id: lt.id ?? null, name: lt.name, hours: lt.hours }); lt.open = false; load(); } catch (e) { toastError(e); } }
</script>

<PageHeader title="Payroll settings" crumbs={[{ label: 'Payroll', href: '/payroll' }]} />
{#if s}
  <section class="card mb-4 p-5">
    <h2 class="mb-2 font-semibold">Tax jurisdiction</h2>
    <div class="mb-3 flex flex-wrap gap-2">{#each [['AU', 'Australia'], ['NZ', 'New Zealand'], ['UK', 'United Kingdom'], ['US', 'United States'], ['GENERIC', 'Generic bracket table']] as [k, l]}<button class="btn btn-sm {s.jurisdiction === k ? 'btn-primary' : ''}" disabled={!can('payroll:write')} onclick={() => setJur(k)}>{l}</button>{/each}</div>
    <p class="text-[13px] text-muted">{s.jurisdiction_name}. {s.disclaimer}</p>
    {#if s.jurisdiction === 'GENERIC'}<div class="mt-3"><label class="label" for="br">Annual tax brackets [upper limit, rate %] (null = no limit)</label><textarea id="br" rows="2" class="textarea font-mono" bind:value={brackets}></textarea><button class="btn btn-sm mt-2" onclick={saveBrackets}>Save brackets</button></div>{/if}
  </section>
  <div class="grid gap-4 lg:grid-cols-2">
    <section class="card p-5"><div class="mb-2 flex items-center justify-between"><h2 class="font-semibold">Pay items</h2>{#if can('payroll:write')}<button class="btn btn-sm" onclick={() => { it = { kind: 'EARNINGS', name: '', rate_type: 'FIXED', taxable: true, super_eligible: false }; itemOpen = true; }}><Icon name="plus" size={14} />Add</button>{/if}</div>
      <table class="tbl text-[13px]"><thead><tr><th>Name</th><th>Type</th><th>Account</th></tr></thead><tbody>{#each s.pay_items as p}<tr class="clickable" onclick={() => { it = { ...p, taxable: !!p.taxable, super_eligible: !!p.super_eligible }; itemOpen = true; }}><td>{p.name}</td><td class="text-muted">{p.kind.toLowerCase()}</td><td class="text-muted">{p.account_name ?? p.liability_name ?? ''}</td></tr>{/each}</tbody></table></section>
    <section class="space-y-4">
      <div class="card p-5"><div class="mb-2 flex items-center justify-between"><h2 class="font-semibold">Pay calendars</h2>{#if can('payroll:write')}<button class="btn btn-sm" onclick={() => (cal = { open: true, name: '', frequency: 'WEEKLY', start_date: '' })}><Icon name="plus" size={14} />Add</button>{/if}</div>
        {#each s.calendars as c}<button class="flex w-full justify-between border-t border-line py-2 text-left text-[13px] first:border-0" onclick={() => (cal = { ...c, open: true })}><span>{c.name}</span><span class="text-muted">{c.frequency.toLowerCase()} from {date(c.start_date)}</span></button>{/each}</div>
      <div class="card p-5"><div class="mb-2 flex items-center justify-between"><h2 class="font-semibold">Leave types</h2>{#if can('payroll:write')}<button class="btn btn-sm" onclick={() => (lt = { open: true, name: '', hours: '' })}><Icon name="plus" size={14} />Add</button>{/if}</div>
        {#each s.leave_types as t}<button class="flex w-full justify-between border-t border-line py-2 text-left text-[13px] first:border-0" onclick={() => (lt = { ...t, hours: fixed(t.accrual_hours_per_year_e2, 2), open: true })}><span>{t.name}</span><span class="text-muted">{fixed(t.accrual_hours_per_year_e2, 2)} h / year (full time)</span></button>{/each}</div>
    </section>
  </div>
{/if}
<Modal bind:open={itemOpen} title="Pay item">
  <div class="space-y-3">
    <div class="grid grid-cols-2 gap-3"><div><span class="label">Type</span><select class="select" bind:value={it.kind} disabled={!!it.id}><option value="EARNINGS">Earnings</option><option value="DEDUCTION">Deduction</option><option value="REIMBURSEMENT">Reimbursement</option></select></div>
      <div><label class="label" for="pn">Name</label><input id="pn" class="input" bind:value={it.name} /></div></div>
    {#if it.kind !== 'DEDUCTION'}<div><span class="label">Expense account</span><Combo bind:value={it.account_id} options={accountOptions((a) => a.class === 'EXPENSE')} /></div>{/if}
    {#if it.kind === 'DEDUCTION'}<div><span class="label">Liability account</span><Combo bind:value={it.liability_account_id} options={accountOptions((a) => a.class === 'LIABILITY')} /></div>{/if}
    {#if it.kind === 'EARNINGS'}<div class="flex gap-5"><label class="flex items-center gap-2"><input type="checkbox" bind:checked={it.taxable} class="accent-[var(--accent)]" />Taxable</label><label class="flex items-center gap-2"><input type="checkbox" bind:checked={it.super_eligible} class="accent-[var(--accent)]" />Retirement contributions apply</label></div>{/if}
  </div>
  {#snippet footer()}<button class="btn" onclick={() => (itemOpen = false)}>Cancel</button><button class="btn btn-primary" onclick={saveItem}>Save</button>{/snippet}
</Modal>
<Modal bind:open={cal.open} title="Pay calendar">
  <div class="space-y-3"><div><label class="label" for="cn">Name</label><input id="cn" class="input" bind:value={cal.name} /></div>
    <div class="grid grid-cols-2 gap-3"><div><span class="label">Frequency</span><select class="select" bind:value={cal.frequency}><option value="WEEKLY">Weekly</option><option value="FORTNIGHTLY">Fortnightly</option><option value="FOURWEEKLY">Four-weekly</option><option value="MONTHLY">Monthly</option><option value="QUARTERLY">Quarterly</option></select></div>
      <div><label class="label" for="cs">First period starts</label><input id="cs" type="date" class="input" bind:value={cal.start_date} /></div></div></div>
  {#snippet footer()}<button class="btn" onclick={() => (cal.open = false)}>Cancel</button><button class="btn btn-primary" onclick={saveCal}>Save</button>{/snippet}
</Modal>
<Modal bind:open={lt.open} title="Leave type">
  <div class="grid grid-cols-2 gap-3"><div><label class="label" for="ltn">Name</label><input id="ltn" class="input" bind:value={lt.name} /></div><div><label class="label" for="lth">Hours accrued per year (full time)</label><input id="lth" class="input num" bind:value={lt.hours} /></div></div>
  {#snippet footer()}<button class="btn" onclick={() => (lt.open = false)}>Cancel</button><button class="btn btn-primary" onclick={saveLt}>Save</button>{/snippet}
</Modal>
