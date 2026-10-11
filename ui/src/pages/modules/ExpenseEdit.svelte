<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, today, qty, unitPrice } from '../../lib/format';
  import { navigate, route } from '../../lib/router.svelte';
  import { can, toast, toastError } from '../../lib/stores.svelte';
  import { totals } from '../../lib/calc';
  import PageHeader from '../../components/PageHeader.svelte';
  import LineItems from '../../components/LineItems.svelte';
  import Status from '../../components/Status.svelte';
  import History from '../../components/History.svelte';
  import Icon from '../../components/Icon.svelte';
  let { id = undefined }: { id?: string } = $props();
  let r = $state<any>(null);
  let projects = $state<any[]>([]);
  rpc('projects.list', { status: 'INPROGRESS' }).then((x) => (projects = x.projects)).catch(() => {});
  let f = $state<any>({ supplier: '', date: today(), reference: '', kind: route.query.get('kind') === 'MILEAGE' ? 'MILEAGE' : 'RECEIPT', distance: '', rate: '0.88', line_amount_type: 'INCLUSIVE', lines: [], project_id: null });
  let file = $state<File | null>(null);
  if (id) rpc('receipts.get', { id: Number(id) }).then((x) => { r = x; f = { supplier: x.supplier, date: x.date, reference: x.reference, kind: x.kind, distance: x.distance_e4 ? qty(x.distance_e4) : '', rate: x.rate_e2 ? String(x.rate_e2 / 100) : '0.88', line_amount_type: x.line_amount_type, project_id: x.project_id,
    lines: x.lines.map((l: any) => ({ description: l.description, quantity: qty(l.qty_e4), unit_price: unitPrice(l.unit_e2), account_id: l.account_id, tax_rate_id: l.tax_rate_id })) }; }).catch(toastError);
  const t = $derived(totals(f.lines, f.line_amount_type));
  const editable = $derived(!r || ['DRAFT', 'SUBMITTED', 'DECLINED'].includes(r.status));
  async function save(submit: boolean) {
    try {
      const res = await rpc('receipts.save', { id: id ? Number(id) : null, receipt: { ...f, lines: f.lines.filter((l: any) => l.account_id || l.description || l.unit_price) } });
      if (file) { const data = await new Promise<string>((ok) => { const fr = new FileReader(); fr.onload = () => ok(String(fr.result)); fr.readAsDataURL(file!); }); await rpc('attachments.add', { entity: 'receipt', entity_id: res.id, filename: file.name, mime: file.type, data }); }
      if (submit) await rpc('receipts.setStatus', { id: res.id, status: 'SUBMITTED' });
      toast(submit ? 'Claim submitted for approval' : 'Claim saved');
      navigate('/expenses');
    } catch (e) { toastError(e); }
  }
</script>

<PageHeader title={f.kind === 'MILEAGE' ? 'Mileage claim' : 'Expense receipt'} crumbs={[{ label: 'Expense claims', href: '/expenses' }]}>
  {#snippet sub()}{#if r}<Status status={r.status} />{/if}{/snippet}
</PageHeader>
<div class="card p-5">
  <div class="grid grid-cols-1 gap-4 md:grid-cols-4">
    <div><label class="label" for="d">Date</label><input id="d" type="date" class="input" bind:value={f.date} disabled={!editable} /></div>
    {#if f.kind === 'MILEAGE'}
      <div><label class="label" for="km">Distance (km / miles)</label><input id="km" class="input num" bind:value={f.distance} disabled={!editable} /></div>
      <div><label class="label" for="rt">Rate per km / mile</label><input id="rt" class="input num" bind:value={f.rate} disabled={!editable} /></div>
      <div class="flex items-end"><div class="text-lg font-semibold">= {money(Math.round(Number(f.distance || 0) * Number(f.rate || 0) * 100))}</div></div>
      <div class="md:col-span-2"><label class="label" for="pu">Purpose of trip</label><input id="pu" class="input" bind:value={f.supplier} disabled={!editable} /></div>
    {:else}
      <div><label class="label" for="s">Spent at</label><input id="s" class="input" bind:value={f.supplier} disabled={!editable} /></div>
      <div><label class="label" for="rf">Reference</label><input id="rf" class="input" bind:value={f.reference} disabled={!editable} /></div>
      <div><label class="label" for="l">Amounts are</label><select id="l" class="select" bind:value={f.line_amount_type} disabled={!editable}><option value="INCLUSIVE">Tax inclusive</option><option value="EXCLUSIVE">Tax exclusive</option><option value="NOTAX">No tax</option></select></div>
    {/if}
    {#if projects.length}<div><span class="label">Assign to project</span><select class="select" bind:value={f.project_id}><option value={null}>None</option>{#each projects as p}<option value={p.id}>{p.name}</option>{/each}</select></div>{/if}
  </div>
  {#if f.kind !== 'MILEAGE'}
    <div class="mt-5"><LineItems bind:lines={f.lines} lat={f.line_amount_type} sales={false} showItems={false} readonly={!editable} accountFilter={(a) => a.class === 'EXPENSE' || !!a.show_in_expense_claims} /></div>
    <div class="mt-3 text-right text-lg font-semibold">Total <span class="num">{money(t.total)}</span></div>
  {/if}
  {#if editable}
    <label class="mt-4 flex cursor-pointer items-center gap-3 rounded-lg border-2 border-dashed border-line2 px-4 py-4 hover:border-accent">
      <Icon name="paperclip" class="text-accent" /><span>{file ? file.name : r?.attachments?.length ? `${r.attachments.length} file(s) attached — add another` : 'Attach a photo or PDF of the receipt'}</span>
      <input type="file" accept="image/*,application/pdf" class="hidden" onchange={(e) => (file = (e.target as HTMLInputElement).files?.[0] ?? null)} />
    </label>
  {/if}
  {#if r?.attachments?.length}<div class="mt-3 flex flex-wrap gap-2">{#each r.attachments as a}<a class="btn btn-sm" href="/download/attachment?id={a.id}" target="_blank"><Icon name="paperclip" size={13} />{a.filename}</a>{/each}</div>{/if}
</div>
{#if editable && can('expenses:write')}
  <div class="mt-4 flex justify-end gap-2"><a class="btn" href="/expenses">Cancel</a><button class="btn" onclick={() => save(false)}>Save draft</button><button class="btn btn-primary" onclick={() => save(true)}>Submit for approval</button></div>
{/if}
{#if r}<History entity="receipt" id={r.id} />{/if}
