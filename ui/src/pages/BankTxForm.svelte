<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, today, qty, unitPrice } from '../lib/format';
  import { navigate, route } from '../lib/router.svelte';
  import { can, confirm, toast, toastError, setting } from '../lib/stores.svelte';
  import { totals } from '../lib/calc';
  import PageHeader from '../components/PageHeader.svelte';
  import LineItems from '../components/LineItems.svelte';
  import ContactSelect from '../components/ContactSelect.svelte';
  import Combo from '../components/Combo.svelte';
  import History from '../components/History.svelte';
  import Attachments from '../components/Attachments.svelte';
  import Icon from '../components/Icon.svelte';
  import { bankAccounts } from '../components/Pickers';

  let { kind = 'SPEND', id = undefined }: { kind?: string; id?: string } = $props();
  let tx = $state<any>(null);
  let f = $state<any>({ type: kind, account_id: route.query.get('account') ? Number(route.query.get('account')) : (bankAccounts()[0]?.value ?? null), contact_id: null, date: today(), reference: '', line_amount_type: 'INCLUSIVE', rate: '', lines: [] });
  let busy = $state(false);
  if (id) rpc('banktx.get', { id: Number(id) }).then((t) => {
    tx = t;
    f = { type: t.type, account_id: t.account_id, contact_id: t.contact_id, date: t.date, reference: t.reference, line_amount_type: t.line_amount_type, rate: '',
      lines: t.lines.map((l: any) => ({ description: l.description, quantity: qty(l.qty_e4), unit_price: unitPrice(l.unit_e2), account_id: l.account_id, tax_rate_id: l.tax_rate_id, tracking1: l.tracking1, tracking2: l.tracking2 })) };
  }).catch(toastError);
  const receive = $derived(f.type.startsWith('RECEIVE'));
  const credit = $derived(f.type.includes('PAYMENT'));
  const t = $derived(totals(f.lines, f.line_amount_type));
  async function save() {
    busy = true;
    try {
      const r = await rpc('banktx.save', { id: id ? Number(id) : null, tx: { ...f, lines: f.lines.filter((l: any) => l.account_id || l.description || l.unit_price) } });
      toast(receive ? 'Money received' : 'Money spent');
      navigate(`/banking/${f.account_id}?tab=transactions`);
      return r;
    } catch (e) { toastError(e); } finally { busy = false; }
  }
  async function del() {
    if (!(await confirm('Remove this transaction?', 'It will be reversed in the ledger.', 'Remove', true))) return;
    try { await rpc('banktx.void', { id: Number(id) }); toast('Transaction removed'); navigate(`/banking/${f.account_id}?tab=transactions`); } catch (e) { toastError(e); }
  }
</script>

<PageHeader title={id ? (receive ? 'Receive money' : 'Spend money') + (credit ? ' (credit)' : '') : receive ? 'Receive money' : 'Spend money'} crumbs={[{ label: 'Bank accounts', href: '/banking' }]}>
  {#if tx}{#if tx.reconciled}<span class="chip bg-ok-soft text-ok">Reconciled</span>{/if}{/if}
</PageHeader>
<div class="card p-5">
  <div class="grid grid-cols-1 gap-4 md:grid-cols-12">
    <div class="md:col-span-3"><label class="label" for="cb-banktxfo-receive-received-int-1">{receive ? 'Received into' : 'Paid from'}</label><Combo id="cb-banktxfo-receive-received-int-1" bind:value={f.account_id} options={bankAccounts()} clearable={false} /></div>
    <div class="md:col-span-3"><label class="label" for="cb-banktxfo-receive-from-to-2">{receive ? 'From' : 'To'}</label><ContactSelect id="cb-banktxfo-receive-from-to-2" bind:value={f.contact_id} /></div>
    <div class="md:col-span-2"><label class="label" for="d">Date</label><input id="d" type="date" class="input" bind:value={f.date} /></div>
    <div class="md:col-span-2"><label class="label" for="r">Reference</label><input id="r" class="input" bind:value={f.reference} /></div>
    <div class="md:col-span-2"><label class="label" for="l">Amounts are</label><select id="l" class="select" bind:value={f.line_amount_type}><option value="INCLUSIVE">Tax inclusive</option><option value="EXCLUSIVE">Tax exclusive</option><option value="NOTAX">No tax</option></select></div>
  </div>
  {#if credit}
    <div class="mt-4 rounded-lg bg-info-soft p-3 text-[13px]">This is an {f.type.includes('OVER') ? 'overpayment' : 'prepayment'} credit of <b class="num">{money(tx?.total)}</b> ({money(tx?.remaining_credit)} unallocated). Apply it from the contact's outstanding {receive ? 'invoices' : 'bills'}.</div>
  {:else}
    <div class="mt-5"><LineItems bind:lines={f.lines} lat={f.line_amount_type} sales={receive} readonly={!!tx?.reconciled} /></div>
    <div class="mt-4 flex justify-end"><div class="w-full max-w-xs space-y-1">
      <div class="flex justify-between"><span class="text-muted">Subtotal</span><span class="num">{money(t.subtotal)}</span></div>
      {#each Object.entries(t.byRate) as [n, v]}<div class="flex justify-between text-muted"><span>{f.line_amount_type === 'INCLUSIVE' ? 'Includes' : 'Total'} {n}</span><span class="num">{money(v)}</span></div>{/each}
      <div class="flex justify-between border-t-2 border-text pt-2 text-[17px] font-semibold"><span>Total</span><span class="num">{money(t.total)}</span></div>
    </div></div>
  {/if}
</div>
<div class="mt-4 flex justify-between">
  <div>{#if id && can('banking:write') && !tx?.reconciled}<button class="btn btn-danger" onclick={del}><Icon name="trash" />Remove</button>{/if}</div>
  {#if can('banking:write') && !credit && !tx?.reconciled}<div class="flex gap-2"><a class="btn" href="/banking">Cancel</a><button class="btn btn-primary" disabled={busy || !f.account_id} onclick={save}>Save</button></div>{/if}
</div>
{#if tx}<Attachments entity="banktx" id={tx.id} /><History entity="banktx" id={tx.id} />{/if}
