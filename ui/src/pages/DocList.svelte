<script lang="ts">
  import { rpc, openView } from '../lib/api';
  import { money, date, docPath, DOC_LABEL, relative, today } from '../lib/format';
  import { route, setQuery, navigate } from '../lib/router.svelte';
  import { can, confirm, toast, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Tabs from '../components/Tabs.svelte';
  import Status from '../components/Status.svelte';
  import Icon from '../components/Icon.svelte';
  import Empty from '../components/Empty.svelte';
  import Modal from '../components/Modal.svelte';
  import Combo from '../components/Combo.svelte';
  import { bankAccounts } from '../components/Pickers';

  let { type }: { type: string } = $props();
  const sales = $derived(['ACCREC', 'ACCRECCREDIT', 'QUOTE'].includes(type));
  const titles: Record<string, string> = { ACCREC: 'Invoices', ACCPAY: 'Bills', QUOTE: 'Quotes', PO: 'Purchase orders', ACCRECCREDIT: 'Credit notes', ACCPAYCREDIT: 'Supplier credit notes' };
  const base = $derived(docPath(type, '').replace(/\/$/, ''));
  const writePerm = $derived(sales ? 'sales:write' : 'purchases:write');

  let tab = $state(route.query.get('tab') ?? 'all');
  let q = $state(route.query.get('q') ?? '');
  let data = $state<any>(null);
  let selected = $state<Set<number>>(new Set());
  let timer: any;

  async function load() {
    try { data = await rpc('docs.list', { type, tab, q, limit: 500 }); selected = new Set(); } catch (e) { toastError(e); }
  }
  $effect(() => { tab; load(); });
  function search() { clearTimeout(timer); timer = setTimeout(() => { setQuery('q', q); load(); }, 200); }

  const c = $derived(data?.counts ?? {});
  const tabs = $derived.by(() => {
    if (type === 'QUOTE') return [{ key: 'all', label: 'All', count: c.all_count }, { key: 'draft', label: 'Draft', count: c.draft }, { key: 'sent', label: 'Sent', count: c.sent },
      { key: 'accepted', label: 'Accepted', count: c.accepted }, { key: 'declined', label: 'Declined', count: c.declined }, { key: 'invoiced', label: 'Invoiced', count: c.invoiced }];
    if (type === 'PO') return [{ key: 'all', label: 'All', count: c.all_count }, { key: 'draft', label: 'Draft', count: c.draft }, { key: 'submitted', label: 'Awaiting approval', count: c.submitted },
      { key: 'authorised', label: 'Approved', count: c.awaiting }, { key: 'billed', label: 'Billed', count: c.billed }];
    return [{ key: 'all', label: 'All', count: c.all_count }, { key: 'draft', label: 'Draft', count: c.draft }, { key: 'submitted', label: 'Awaiting approval', count: c.submitted },
      { key: 'awaiting', label: type.endsWith('CREDIT') ? 'Unallocated' : 'Awaiting payment', count: c.awaiting },
      ...(type.endsWith('CREDIT') ? [] : [{ key: 'overdue', label: 'Overdue', count: c.overdue }]), { key: 'paid', label: type.endsWith('CREDIT') ? 'Allocated' : 'Paid', count: c.paid }];
  });
  const rows = $derived(data?.rows ?? []);
  const allSel = $derived(rows.length > 0 && rows.every((r: any) => selected.has(r.id)));
  function toggleAll() { selected = allSel ? new Set() : new Set(rows.map((r: any) => r.id)); }
  function toggle(id: number) { const s = new Set(selected); s.has(id) ? s.delete(id) : s.add(id); selected = s; }
  const selRows = $derived(rows.filter((r: any) => selected.has(r.id)));

  async function bulk(status: string, label: string) {
    const ok = await confirm(`${label} ${selected.size} item${selected.size === 1 ? '' : 's'}?`, '', label, status === 'DELETED' || status === 'VOIDED');
    if (!ok) return;
    let n = 0, errs: string[] = [];
    for (const r of selRows) { try { await rpc('docs.setStatus', { id: r.id, status }); n++; } catch (e: any) { errs.push(`${r.number}: ${e.message}`); } }
    toast(`${n} updated${errs.length ? `, ${errs.length} skipped` : ''}`, errs.length ? 'info' : 'ok');
    if (errs.length) toastError(errs.slice(0, 3).join('; '));
    load();
  }
  async function markSent() { for (const r of selRows) await rpc('docs.markSent', { id: r.id }).catch(() => {}); toast('Marked as sent'); load(); }

  // batch payment
  let payOpen = $state(false);
  let payAcct = $state<number | null>(null);
  let payDate = $state(today());
  let payRef = $state('');
  let payAmounts = $state<Record<number, string>>({});
  function openPay() {
    payAmounts = Object.fromEntries(selRows.filter((r: any) => r.status === 'AUTHORISED').map((r: any) => [r.id, (r.amount_due / 100).toFixed(2)]));
    payOpen = true;
  }
  async function doPay() {
    try {
      await rpc('batch.create', { account_id: payAcct, date: payDate, reference: payRef, items: Object.entries(payAmounts).map(([id, amount]) => ({ document_id: Number(id), amount })) });
      toast('Batch payment recorded'); payOpen = false; load();
    } catch (e) { toastError(e); }
  }
  const payTotal = $derived(Object.values(payAmounts).reduce((s, a) => s + Math.round(Number(a || 0) * 100), 0));
  const printable = $derived(true);
</script>

<PageHeader title={titles[type]} crumbs={[{ label: sales ? 'Sales' : 'Purchases', href: sales ? '/sales' : '/purchases' }]}>
  {#if type === 'ACCREC' || type === 'ACCPAY'}<a class="btn" href={type === 'ACCREC' ? '/sales/repeating' : '/purchases/repeating'}><Icon name="refresh" />Repeating</a>{/if}
  {#if can(writePerm)}<a class="btn btn-primary" href="{base}/new"><Icon name="plus" />New {DOC_LABEL[type].toLowerCase()}</a>{/if}
</PageHeader>

{#if (type === 'ACCREC' || type === 'ACCPAY') && data}
  <div class="mb-4 grid gap-3 sm:grid-cols-3">
    <button class="card p-4 text-left hover:border-line2" onclick={() => (tab = 'draft')}><div class="text-[12px] text-muted">Draft</div><div class="num text-lg font-semibold">{money(c.draft_total)}</div></button>
    <button class="card p-4 text-left hover:border-line2" onclick={() => (tab = 'awaiting')}><div class="text-[12px] text-muted">{type === 'ACCREC' ? 'Awaiting payment' : 'To pay'}</div><div class="num text-lg font-semibold">{money(c.awaiting_total)}</div></button>
    <button class="card p-4 text-left hover:border-line2" onclick={() => (tab = 'overdue')}><div class="text-[12px] text-bad">Overdue</div><div class="num text-lg font-semibold">{money(c.overdue_total)}</div></button>
  </div>
{/if}

<Tabs {tabs} bind:value={tab} onchange={(k) => setQuery('tab', k === 'all' ? null : k)} />

<div class="mb-3 flex flex-wrap items-center gap-2">
  <div class="relative w-72"><Icon name="search" class="absolute left-2.5 top-1/2 -translate-y-1/2 text-faint" /><input class="input pl-8" placeholder="Search number, reference, contact, amount" bind:value={q} oninput={search} /></div>
  {#if selected.size}
    <span class="ml-2 text-muted">{selected.size} selected</span>
    {#if can(writePerm)}
      {#if selRows.some((r: any) => r.status === 'DRAFT' || r.status === 'SUBMITTED') && type !== 'QUOTE'}<button class="btn btn-sm" onclick={() => bulk(type === 'PO' ? 'AUTHORISED' : 'AUTHORISED', 'Approve')}><Icon name="check" size={14} />Approve</button>{/if}
      {#if selRows.some((r: any) => r.status === 'DRAFT') && type !== 'QUOTE'}<button class="btn btn-sm" onclick={() => bulk('SUBMITTED', 'Submit for approval')}>Submit</button>{/if}
      {#if (type === 'ACCREC' || type === 'ACCPAY') && selRows.some((r: any) => r.status === 'AUTHORISED')}<button class="btn btn-sm" onclick={openPay}><Icon name="dollar" size={14} />Batch payment</button>{/if}
      <button class="btn btn-sm" onclick={markSent}>Mark as sent</button>
      {#if selRows.every((r: any) => ['DRAFT', 'SUBMITTED', 'SENT'].includes(r.status))}<button class="btn btn-sm btn-danger" onclick={() => bulk('DELETED', 'Delete')}><Icon name="trash" size={14} />Delete</button>{/if}
    {/if}
    {#if printable}<button class="btn btn-sm" onclick={() => selRows.forEach((r: any) => openView(`/view/doc/${r.id}`))}><Icon name="print" size={14} />Print</button>{/if}
  {/if}
</div>

<div class="card overflow-x-auto">
  {#if !data}
    <div class="p-8 text-center text-muted">Loading…</div>
  {:else if !rows.length}
    <Empty icon="doc" title={q ? 'No matches' : `No ${titles[type].toLowerCase()} here yet`} text={q ? 'Try a different search.' : ''}>
      {#if can(writePerm) && !q}<a class="btn btn-primary" href="{base}/new">New {DOC_LABEL[type].toLowerCase()}</a>{/if}
    </Empty>
  {:else}
    <table class="tbl">
      <thead><tr>
        <th class="w-8"><input type="checkbox" checked={allSel} onchange={toggleAll} aria-label="Select all" class="accent-[var(--accent)]" /></th>
        <th>Number</th><th>Ref</th><th>{sales ? 'To' : 'From'}</th><th>Date</th>
        {#if type === 'QUOTE'}<th>Expiry</th><th class="n">Total</th>
        {:else if type === 'PO'}<th>Delivery</th><th class="n">Total</th>
        {:else}<th>Due date</th><th class="n">Paid</th><th class="n">Due</th>{/if}
        <th>Status</th>
      </tr></thead>
      <tbody>
        {#each rows as r (r.id)}
          <tr class="clickable {selected.has(r.id) ? 'sel' : ''}" onclick={() => navigate(`${base}/${r.id}`)}>
            <td onclick={(e) => e.stopPropagation()}><input type="checkbox" checked={selected.has(r.id)} onchange={() => toggle(r.id)} aria-label="Select" class="accent-[var(--accent)]" /></td>
            <td class="font-medium">{r.number || '—'}</td>
            <td class="max-w-[180px] truncate text-muted">{r.reference || r.title}</td>
            <td class="max-w-[220px] truncate">{r.contact_name}</td>
            <td class="whitespace-nowrap">{date(r.date)}</td>
            {#if type === 'QUOTE'}<td class="whitespace-nowrap">{date(r.expiry_date)}</td><td class="n">{money(r.total)}</td>
            {:else if type === 'PO'}<td class="whitespace-nowrap">{date(r.delivery_date)}</td><td class="n">{money(r.total)}</td>
            {:else}
              <td class="whitespace-nowrap {r.overdue ? 'text-bad' : ''}">{date(r.due_date)}{#if r.overdue}<span class="ml-1 text-[11px]">({relative(r.due_date)})</span>{/if}</td>
              <td class="n text-muted">{money(r.amount_paid + r.amount_credited, { blankZero: true })}</td>
              <td class="n font-medium">{money(r.status === 'PAID' ? 0 : r.amount_due)}</td>
            {/if}
            <td><Status status={r.status} overdue={!!r.overdue} />{#if r.sent && r.status === 'AUTHORISED'}<span class="ml-1 text-[11px] text-faint">Sent</span>{/if}{#if r.currency !== (data?.base ?? r.currency)}{/if}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>

<Modal bind:open={payOpen} title="Batch payment" width={640}>
  <div class="mb-3 grid grid-cols-3 gap-3">
    <div><label class="label" for="bpa">Pay from</label><Combo bind:value={payAcct} options={bankAccounts()} placeholder="Bank account" /></div>
    <div><label class="label" for="bpd">Date</label><input id="bpd" type="date" class="input" bind:value={payDate} /></div>
    <div><label class="label" for="bpr">Reference</label><input id="bpr" class="input" bind:value={payRef} /></div>
  </div>
  <table class="tbl text-[13px]"><thead><tr><th>Document</th><th>Contact</th><th class="n">Due</th><th class="n">Payment</th></tr></thead>
    <tbody>{#each selRows.filter((r: any) => r.status === 'AUTHORISED') as r}
      <tr><td>{r.number}</td><td>{r.contact_name}</td><td class="n">{money(r.amount_due)}</td><td class="n"><input class="input num w-28 text-right" bind:value={payAmounts[r.id]} /></td></tr>{/each}</tbody>
  </table>
  <div class="mt-3 text-right font-semibold">Total <span class="num">{money(payTotal)}</span></div>
  {#snippet footer()}<button class="btn" onclick={() => (payOpen = false)}>Cancel</button><button class="btn btn-primary" disabled={!payAcct} onclick={doPay}>Make payments</button>{/snippet}
</Modal>
