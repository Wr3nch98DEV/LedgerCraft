<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, date, today, addDays } from '../lib/format';
  import { can, confirm, toast, toastError, setting } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Modal from '../components/Modal.svelte';
  import LineItems from '../components/LineItems.svelte';
  import ContactSelect from '../components/ContactSelect.svelte';
  import Status from '../components/Status.svelte';
  import Icon from '../components/Icon.svelte';

  let { kind }: { kind: 'ACCREC' | 'ACCPAY' | 'MJ' } = $props();
  const sales = kind === 'ACCREC';
  const title = kind === 'ACCREC' ? 'Repeating invoices' : kind === 'ACCPAY' ? 'Repeating bills' : 'Repeating journals';
  let rows = $state<any[]>([]);
  const load = () => rpc('repeating.list', { kind }).then((r) => (rows = r)).catch(toastError);
  load();
  let open = $state(false);
  let editId = $state<number | null>(null);
  const blank = () => ({ start_date: addDays(today(), 1), next_date: '', end_date: '', period_unit: 'MONTHLY', period_count: 1, due_terms_type: 'DAYSAFTERBILLDATE', due_terms_days: Number(setting(sales ? 'sales_terms_days' : 'bill_terms_days', '14')), save_as: 'DRAFT',
    template: { contact_id: null, reference: '', line_amount_type: sales ? 'EXCLUSIVE' : 'INCLUSIVE', lines: [] as any[] } });
  let f = $state<any>(blank());
  function add() { editId = null; f = blank(); open = true; }
  function edit(r: any) { editId = r.id; f = { ...r, end_date: r.end_date ?? '', template: { ...r.template, lines: r.template.lines ?? [] } }; open = true; }
  async function save() {
    const body = { ...f, kind, template: { ...f.template, lines: f.template.lines.filter((l: any) => l.item_id || l.description || l.unit_price || l.account_id) } };
    try { await rpc('repeating.save', { id: editId, repeating: body }); toast('Repeating template saved'); open = false; load(); } catch (e) { toastError(e); }
  }
  async function setStatus(r: any, status: string) {
    if (status === 'DELETED' && !(await confirm('Delete this repeating template?', 'Transactions already created are not affected.', 'Delete', true))) return;
    await rpc('repeating.setStatus', { id: r.id, status }); load();
  }
  async function runNow() { try { const r = await rpc('repeating.run', { date: today() }); const ok = r.filter((x: any) => !x.error).length; toast(`${ok} transaction(s) created`); r.filter((x: any) => x.error).forEach((x: any) => toastError(x.error)); load(); } catch (e) { toastError(e); } }
</script>

<PageHeader {title} crumbs={[{ label: sales ? 'Sales' : 'Purchases', href: sales ? '/sales' : '/purchases' }]}>
  <button class="btn" onclick={runNow}><Icon name="refresh" />Run due now</button>
  {#if can(sales ? 'sales:write' : 'purchases:write')}<button class="btn btn-primary" onclick={add}><Icon name="plus" />New repeating {kind === 'MJ' ? 'journal' : sales ? 'invoice' : 'bill'}</button>{/if}
</PageHeader>
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th>Contact</th><th>Reference</th><th>Repeats</th><th>Next date</th><th>End</th><th>Save as</th><th class="n">Amount</th><th>Status</th><th></th></tr></thead>
    <tbody>{#each rows as r}<tr class="clickable" onclick={() => edit(r)}>
      <td class="font-medium">{r.contact_name ?? '—'}</td><td class="text-muted">{r.reference ?? ''}</td>
      <td>Every {r.period_count > 1 ? r.period_count + ' ' : ''}{r.period_unit === 'WEEKLY' ? 'week' : 'month'}{r.period_count > 1 ? 's' : ''}</td>
      <td>{date(r.next_date)}</td><td class="text-muted">{date(r.end_date) || '—'}</td>
      <td class="text-muted">{r.save_as === 'DRAFT' ? 'Draft' : r.save_as === 'AUTHORISED' ? 'Approved' : 'Approved & emailed'}</td>
      <td class="n">{money(r.estimated_total)}</td><td><Status status={r.status} /></td>
      <td class="whitespace-nowrap text-right" onclick={(e) => e.stopPropagation()}>{#if r.status === 'ACTIVE'}<button class="btn btn-ghost btn-sm" onclick={() => setStatus(r, 'PAUSED')}>Pause</button>{:else}<button class="btn btn-ghost btn-sm" onclick={() => setStatus(r, 'ACTIVE')}>Resume</button>{/if}<button class="btn btn-ghost btn-sm" onclick={() => setStatus(r, 'DELETED')} aria-label="Delete"><Icon name="trash" size={14} /></button></td></tr>
    {:else}<tr><td colspan="9" class="py-10 text-center text-muted">No repeating templates. Repeating {sales ? 'invoices are' : 'bills are'} created automatically on schedule — use [Month] and [Year] in descriptions and references.</td></tr>{/each}</tbody></table>
</div>

<Modal bind:open title={editId ? 'Edit repeating template' : 'New repeating template'} width={1100}>
  <div class="grid grid-cols-2 gap-3 md:grid-cols-6">
    <div class="col-span-2"><span class="label">{sales ? 'To' : 'From'}</span><ContactSelect bind:value={f.template.contact_id} /></div>
    <div><label class="label" for="sd">{editId ? 'Next date' : 'Start date'}</label>{#if editId}<input id="sd" type="date" class="input" bind:value={f.next_date} />{:else}<input id="sd" type="date" class="input" bind:value={f.start_date} />{/if}</div>
    <div><span class="label">Repeat every</span><div class="flex gap-1"><input type="number" min="1" class="input w-16" bind:value={f.period_count} /><select class="select" bind:value={f.period_unit}><option value="MONTHLY">month(s)</option><option value="WEEKLY">week(s)</option></select></div></div>
    <div><label class="label" for="ed">End date (optional)</label><input id="ed" type="date" class="input" bind:value={f.end_date} /></div>
    <div><span class="label">Due</span><div class="flex items-center gap-1"><input type="number" class="input w-16" bind:value={f.due_terms_days} /><span class="text-[13px] text-muted">days after</span></div></div>
    <div class="col-span-2"><label class="label" for="rf">Reference</label><input id="rf" class="input" bind:value={f.template.reference} placeholder="e.g. Retainer [Month] [Year]" /></div>
    <div class="col-span-2"><span class="label">Save as</span><select class="select" bind:value={f.save_as}><option value="DRAFT">Draft (for review)</option><option value="AUTHORISED">Approved</option>{#if sales}<option value="AUTHORISED_SEND">Approved & emailed</option>{/if}</select></div>
    <div class="col-span-2"><span class="label">Amounts are</span><select class="select" bind:value={f.template.line_amount_type}><option value="EXCLUSIVE">Tax exclusive</option><option value="INCLUSIVE">Tax inclusive</option><option value="NOTAX">No tax</option></select></div>
  </div>
  <div class="mt-4"><LineItems bind:lines={f.template.lines} lat={f.template.line_amount_type} {sales} /></div>
  {#snippet footer()}<button class="btn" onclick={() => (open = false)}>Cancel</button><button class="btn btn-primary" onclick={save}>Save</button>{/snippet}
</Modal>
