<script lang="ts">
  import { rpc, openView } from '../lib/api';
  import { money, moneyPlain, date, today, docPath, DOC_LABEL, qty, unitPrice, pct, rate as fmtRate, addDays, relative } from '../lib/format';
  import { navigate, addGuard, route } from '../lib/router.svelte';
  import { lookups, session, can, confirm, toast, toastError, pushRecent, setting } from '../lib/stores.svelte';
  import { totals } from '../lib/calc';
  import PageHeader from '../components/PageHeader.svelte';
  import LineItems from '../components/LineItems.svelte';
  import ContactSelect from '../components/ContactSelect.svelte';
  import Combo from '../components/Combo.svelte';
  import Status from '../components/Status.svelte';
  import History from '../components/History.svelte';
  import Attachments from '../components/Attachments.svelte';
  import Modal from '../components/Modal.svelte';
  import Icon from '../components/Icon.svelte';
  import { bankAccounts, currencyOptions } from '../components/Pickers';

  let { type, id = undefined }: { type: string; id?: string } = $props();
  const isNew = $derived(!id);
  const sales = $derived(['ACCREC', 'ACCRECCREDIT', 'QUOTE'].includes(type));
  const ledger = $derived(['ACCREC', 'ACCPAY', 'ACCRECCREDIT', 'ACCPAYCREDIT'].includes(type));
  const credit = $derived(type.endsWith('CREDIT'));
  const writePerm = $derived(sales ? 'sales:write' : 'purchases:write');
  const base = $derived(docPath(type, '').replace(/\/$/, ''));
  const listTitle: Record<string, string> = { ACCREC: 'Invoices', ACCPAY: 'Bills', QUOTE: 'Quotes', PO: 'Purchase orders', ACCRECCREDIT: 'Credit notes', ACCPAYCREDIT: 'Supplier credits' };
  const baseCur = setting('base_currency', 'AUD');

  let doc = $state<any>(null);           // server copy (for existing)
  let f = $state<any>(null);             // form state
  let editing = $state(false);
  let saving = $state(false);
  let dirty = $state(false);
  let errors = $state<Record<string, boolean>>({});

  function blankForm() {
    const lat = sales ? setting('default_sales_lat', 'EXCLUSIVE') : setting('default_purchases_lat', 'INCLUSIVE');
    return { contact_id: route.query.get('contact') ? Number(route.query.get('contact')) : null, date: today(), due_date: '', expiry_date: type === 'QUOTE' ? addDays(today(), 30) : '',
      delivery_date: '', delivery_address: type === 'PO' ? setting('address') : '', attention: '', number: '', reference: '', title: '', summary: '', terms: '',
      line_amount_type: lat, currency: baseCur, rate: '', branding_theme_id: null, lines: [] as any[] };
  }
  function fromDoc(d: any) {
    return { contact_id: d.contact_id, date: d.date, due_date: d.due_date ?? '', expiry_date: d.expiry_date ?? '', delivery_date: d.delivery_date ?? '', delivery_address: d.delivery_address,
      attention: d.attention, number: d.number, reference: d.reference, title: d.title, summary: d.summary, terms: d.terms, line_amount_type: d.line_amount_type,
      currency: d.currency, rate: d.currency !== baseCur ? fmtRate(d.rate_e8) : '', branding_theme_id: d.branding_theme_id,
      lines: d.lines.map((l: any) => ({ item_id: l.item_id, description: l.description, quantity: qty(l.qty_e4), unit_price: unitPrice(l.unit_e2), discount: l.discount_bp ? pct(l.discount_bp) : '',
        account_id: l.account_id, tax_rate_id: l.tax_rate_id, tracking1: l.tracking1, tracking2: l.tracking2 })) };
  }
  async function load() {
    if (isNew) {
      f = blankForm();
      const copyFrom = route.query.get('copy');
      if (copyFrom) { const d = await rpc('docs.get', { id: Number(copyFrom) }); f = { ...fromDoc(d), date: today(), due_date: '', number: '' }; }
      editing = true;
      return;
    }
    doc = await rpc('docs.get', { id: Number(id) });
    f = fromDoc(doc);
    editing = ['DRAFT', 'SUBMITTED'].includes(doc.status) && can(writePerm) && route.query.get('view') !== '1';
    pushRecent(`${DOC_LABEL[doc.type]} ${doc.number || ''} · ${doc.contact_name}`, location.pathname);
    dirty = false;
  }
  load().catch(toastError);

  $effect(() => { if (f) { JSON.stringify(f); dirty = true; } });
  const unguard = addGuard(() => !(editing && dirty && f && changedSinceLoad()) || window.confirm('You have unsaved changes. Leave anyway?'));
  let loadedSnapshot = '';
  $effect(() => { if (f && !loadedSnapshot) loadedSnapshot = JSON.stringify(f); });
  function changedSinceLoad() { return JSON.stringify(f) !== loadedSnapshot; }
  $effect(() => () => unguard());

  const t = $derived(f ? totals(f.lines, f.line_amount_type) : { subtotal: 0, tax: 0, total: 0, byRate: {} });
  const foreign = $derived(f && f.currency && f.currency !== baseCur);
  const taxLabel = setting('tax_label', 'Tax');

  async function onContact(cid: number | null, c: any) {
    errors.contact = false;
    if (!cid || !isNew) return;
    try {
      const full = await rpc('contacts.get', { id: cid });
      if (full.default_currency) f.currency = full.default_currency;
      const defAcct = sales ? full.sales_account_id : full.purchase_account_id;
      const defTax = sales ? full.sales_tax_rate_id : full.purchase_tax_rate_id;
      for (const l of f.lines) { if (!l.account_id && defAcct) l.account_id = defAcct; if (!l.tax_rate_id && defTax) l.tax_rate_id = defTax; if (sales && full.discount_bp && !l.discount) l.discount = String(full.discount_bp / 100); }
      if (defAcct && f.lines.length <= 1 && !f.lines[0]?.account_id) { f.lines = [{ item_id: null, description: '', quantity: '1', unit_price: '', discount: '', account_id: defAcct, tax_rate_id: defTax ?? lookups.accounts.find((a) => a.id === defAcct)?.tax_rate_id, tracking1: null, tracking2: null }]; }
    } catch {}
  }
  $effect(() => {
    if (f && foreign && !f.rate) rpc('rates.get', { code: f.currency, date: f.date }).then((r) => { if (r.rate_e8 && !f.rate) f.rate = fmtRate(r.rate_e8); }).catch(() => {});
  });

  function payload(status?: string) {
    return { type, contact_id: f.contact_id, date: f.date, due_date: f.due_date || null, expiry_date: f.expiry_date || null, delivery_date: f.delivery_date || null,
      delivery_address: f.delivery_address, attention: f.attention, number: f.number || null, reference: f.reference, title: f.title, summary: f.summary, terms: f.terms,
      line_amount_type: f.line_amount_type, currency: f.currency, rate: foreign ? f.rate : null, branding_theme_id: f.branding_theme_id,
      lines: f.lines.filter((l: any) => l.item_id || l.description || l.unit_price || l.account_id), status };
  }
  async function save(status?: string, then?: 'new' | 'print' | 'email' | 'stay') {
    errors = {};
    if (!f.contact_id) { errors.contact = true; toastError('Choose a contact'); return; }
    saving = true;
    try {
      const r = await rpc('docs.save', { id: isNew ? null : Number(id), doc: payload(status) });
      loadedSnapshot = JSON.stringify(f);
      const label = status === 'AUTHORISED' ? (type === 'PO' ? 'approved' : 'approved') : status === 'SUBMITTED' ? 'submitted for approval' : status === 'SENT' ? 'saved and marked as sent' : 'saved';
      toast(`${DOC_LABEL[type]} ${label}`);
      if (then === 'print') openView(`/view/doc/${r.id}`);
      if (then === 'new') { navigate(`${base}/new`); return; }
      if (then === 'email') { navigate(`${base}/${r.id}?view=1&email=1`, { replace: true }); return; }
      if (isNew) navigate(`${base}/${r.id}`, { replace: true }); else await load();
    } catch (e) { toastError(e); } finally { saving = false; }
  }
  async function setStatus(status: string, msg: string, danger = false) {
    if (danger && !(await confirm(`${msg}?`, status === 'VOIDED' ? 'This reverses the transaction in the ledger. It cannot be undone.' : '', msg, true))) return;
    try { await rpc('docs.setStatus', { id: Number(id), status }); toast(`${DOC_LABEL[type]} ${msg.toLowerCase().replace(/^mark as /, 'marked as ')}`); if (status === 'DELETED') navigate(base); else await load(); }
    catch (e) { toastError(e); }
  }
  async function copyTo(to: string) {
    try { const r = await rpc('docs.copy', { id: Number(id), to }); toast('Copied'); navigate(docPath(to, r.id)); } catch (e) { toastError(e); }
  }

  // ---------- payments ----------
  let pay = $state({ amount: '', date: today(), account_id: null as number | null, reference: '', rate: '' });
  $effect(() => { if (doc && doc.status === 'AUTHORISED') pay.amount = moneyPlain(doc.amount_due); });
  async function addPayment() {
    try { await rpc('payments.add', { document_id: Number(id), ...pay, rate: foreign ? pay.rate : null }); toast('Payment recorded'); pay.reference = ''; await load(); } catch (e) { toastError(e); }
  }
  async function removePayment(pid: number) {
    if (!(await confirm('Remove this payment?', 'The payment is reversed in the ledger. Reconciled payments must be unreconciled first.', 'Remove', true))) return;
    try { await rpc('payments.delete', { id: pid }); toast('Payment removed'); await load(); } catch (e) { toastError(e); }
  }

  // ---------- credits ----------
  let credits = $state<any[]>([]);
  let allocOpen = $state(false);
  let openDocs = $state<any[]>([]);
  let alloc = $state<Record<number, string>>({});
  $effect(() => { if (doc && doc.status === 'AUTHORISED' && !credit) rpc('contacts.credits', { id: doc.contact_id }).then((c) => (credits = c.filter((x: any) => (sales ? ['ACCRECCREDIT', 'RECEIVE-OVERPAYMENT', 'RECEIVE-PREPAYMENT'] : ['ACCPAYCREDIT', 'SPEND-OVERPAYMENT', 'SPEND-PREPAYMENT']).includes(x.type) && x.currency === doc.currency))); });
  async function applyCredit(c: any) {
    const amt = Math.min(c.remaining, doc.amount_due);
    try {
      if (c.kind === 'document') await rpc('credits.allocate', { credit_id: c.id, document_id: Number(id), amount: moneyPlain(amt) });
      else await rpc('prepayments.allocate', { transaction_id: c.id, document_id: Number(id), amount: moneyPlain(amt) });
      toast('Credit applied'); await load();
    } catch (e) { toastError(e); }
  }
  async function openAllocate() {
    const r = await rpc('docs.list', { type: sales ? 'ACCREC' : 'ACCPAY', tab: 'awaiting', contact_id: doc.contact_id });
    openDocs = r.rows.filter((x: any) => x.currency === doc.currency);
    alloc = {};
    allocOpen = true;
  }
  async function doAllocate() {
    try {
      for (const [did, amt] of Object.entries(alloc)) if (amt && Number(amt) > 0) await rpc('credits.allocate', { credit_id: Number(id), document_id: Number(did), amount: amt });
      allocOpen = false; toast('Credit allocated'); await load();
    } catch (e) { toastError(e); }
  }
  async function removeAllocation(aid: number) { try { await rpc('credits.deleteAllocation', { id: aid }); toast('Allocation removed'); await load(); } catch (e) { toastError(e); } }

  // ---------- email ----------
  let emailOpen = $state(false);
  let email = $state({ to: '', subject: '', body: '' });
  async function openEmail() { try { email = await rpc('docs.emailPreview', { id: Number(id) }); emailOpen = true; } catch (e) { toastError(e); } }
  async function sendEmail() {
    try { const r = await rpc('docs.email', { id: Number(id), ...email }); emailOpen = false; toast(r.queued ? 'Email queued for sending' : 'Saved to outbox (configure SMTP in Settings to send automatically)', r.queued ? 'ok' : 'info'); await load(); }
    catch (e) { toastError(e); }
  }
  $effect(() => { if (doc && route.query.get('email') === '1' && !emailOpen) { openEmail(); } });

  function keys(e: KeyboardEvent) {
    if (!editing) return;
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') { e.preventDefault(); save(type === 'QUOTE' ? undefined : 'DRAFT'); }
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') { e.preventDefault(); save(type === 'QUOTE' ? 'SENT' : 'AUTHORISED'); }
  }
  const title = $derived(isNew ? `New ${DOC_LABEL[type].toLowerCase()}` : `${DOC_LABEL[type]} ${doc?.number ?? ''}`);
  const themes = $state<any[]>([]);
  rpc('branding.list').then((r) => themes.splice(0, themes.length, ...r)).catch(() => {});
  const otherSide = $derived(sales ? 'customer' : 'supplier');
</script>

<svelte:window onkeydown={keys} />

{#if f}
<PageHeader {title} crumbs={[{ label: sales ? 'Sales' : 'Purchases', href: sales ? '/sales' : '/purchases' }, { label: listTitle[type], href: base }]}>
  {#snippet sub()}
    {#if doc}<span class="mr-2"><Status status={doc.status} overdue={!!doc.overdue} /></span>
      {#if doc.overdue}<span class="text-bad">Due {relative(doc.due_date)}</span>{/if}
      {#if doc.sent}<span class="ml-2 text-[13px] text-faint">· Sent</span>{/if}
      {#if doc.source_document_id}<a class="ml-2 text-[13px] link" href="/doc/{doc.source_document_id}">· Copied from source</a>{/if}
    {/if}
  {/snippet}
  {#if doc}
    <button class="btn" onclick={() => openView(`/view/doc/${doc.id}`)}><Icon name="print" />Print / PDF</button>
    {#if doc.status !== 'DRAFT' && doc.status !== 'DELETED' && (sales || type === 'PO')}<button class="btn" onclick={openEmail}><Icon name="mail" />Email</button>{/if}
    {#if (type === 'ACCREC' || type === 'QUOTE') && doc.status !== 'DRAFT'}<a class="btn" href="/online/{doc.online_token}" target="_blank" rel="noopener"><Icon name="globe" />Online {type === 'QUOTE' ? 'quote' : 'invoice'}</a>{/if}
  {/if}
</PageHeader>

{#if editing}
  <!-- ================= EDIT MODE ================= -->
  <div class="card p-5">
    <div class="grid grid-cols-1 gap-4 md:grid-cols-12">
      <div class="md:col-span-4"><label class="label" for="cb-docedit-sales-to-from-1">{sales ? 'To' : 'From'}</label><ContactSelect id="cb-docedit-sales-to-from-1" bind:value={f.contact_id} onchange={onContact} invalid={errors.contact} kind={otherSide} /></div>
      <div class="md:col-span-2"><label class="label" for="dt">{type === 'QUOTE' ? 'Date' : type === 'PO' ? 'Date' : credit ? 'Date' : 'Issue date'}</label><input id="dt" type="date" class="input" bind:value={f.date} /></div>
      {#if type === 'QUOTE'}<div class="md:col-span-2"><label class="label" for="ex">Expiry</label><input id="ex" type="date" class="input" bind:value={f.expiry_date} /></div>
      {:else if type === 'PO'}<div class="md:col-span-2"><label class="label" for="dd">Delivery date</label><input id="dd" type="date" class="input" bind:value={f.delivery_date} /></div>
      {:else if !credit}<div class="md:col-span-2"><label class="label" for="du">Due date</label><input id="du" type="date" class="input" bind:value={f.due_date} placeholder="From terms" /></div>
      {:else}<div class="md:col-span-2"></div>{/if}
      <div class="md:col-span-2"><label class="label" for="nm">{type === 'ACCPAY' ? 'Bill number' : 'Number'}</label><input id="nm" class="input" bind:value={f.number} placeholder={type === 'ACCPAY' ? 'Supplier invoice #' : 'Auto'} /></div>
      <div class="md:col-span-2"><label class="label" for="rf">Reference</label><input id="rf" class="input" bind:value={f.reference} /></div>
      {#if type === 'QUOTE'}
        <div class="md:col-span-4"><label class="label" for="qt">Title</label><input id="qt" class="input" bind:value={f.title} /></div>
        <div class="md:col-span-8"><label class="label" for="qs">Summary</label><input id="qs" class="input" bind:value={f.summary} /></div>
      {/if}
      {#if type === 'PO'}
        <div class="md:col-span-6"><label class="label" for="da">Delivery address</label><textarea id="da" rows="2" class="textarea" bind:value={f.delivery_address}></textarea></div>
        <div class="md:col-span-3"><label class="label" for="at">Attention</label><input id="at" class="input" bind:value={f.attention} /></div>
      {/if}
      <div class="md:col-span-2"><label class="label" for="cu">Currency</label><Combo id="cu" bind:value={f.currency} options={currencyOptions()} clearable={false} /></div>
      {#if foreign}<div class="md:col-span-2"><label class="label" for="rt">Rate (1 {baseCur} =)</label><input id="rt" class="input num" bind:value={f.rate} placeholder="0.0000" /></div>{/if}
      <div class="md:col-span-3"><label class="label" for="lat">Amounts are</label>
        <select id="lat" class="select" bind:value={f.line_amount_type}><option value="EXCLUSIVE">Tax exclusive</option><option value="INCLUSIVE">Tax inclusive</option><option value="NOTAX">No tax</option></select></div>
      {#if sales && themes.length > 1}<div class="md:col-span-3"><label class="label" for="bt">Branding theme</label><select id="bt" class="select" bind:value={f.branding_theme_id}>{#each themes as th}<option value={th.id}>{th.name}</option>{/each}</select></div>{/if}
    </div>

    <div class="mt-5"><LineItems bind:lines={f.lines} lat={f.line_amount_type} {sales} /></div>

    <div class="mt-4 flex flex-col gap-4 md:flex-row md:justify-between">
      <div class="md:w-1/2"><label class="label" for="tm">{type === 'QUOTE' ? 'Terms' : 'Notes / terms (shown on the document)'}</label><textarea id="tm" rows="3" class="textarea" bind:value={f.terms}></textarea></div>
      <div class="w-full max-w-sm space-y-1.5 text-[14px]">
        {#if f.line_amount_type === 'INCLUSIVE'}
          <div class="flex justify-between"><span class="text-muted">Subtotal (excl. {taxLabel})</span><span class="num">{money(t.subtotal)}</span></div>
          {#each Object.entries(t.byRate) as [n, v]}<div class="flex justify-between text-muted"><span>Includes {n}</span><span class="num">{money(v)}</span></div>{/each}
        {:else}
          <div class="flex justify-between"><span class="text-muted">Subtotal</span><span class="num">{money(t.subtotal)}</span></div>
          {#each Object.entries(t.byRate) as [n, v]}<div class="flex justify-between text-muted"><span>Total {n}</span><span class="num">{money(v)}</span></div>{/each}
        {/if}
        <div class="flex justify-between border-t-2 border-text pt-2 text-[17px] font-semibold"><span>Total {f.currency}</span><span class="num">{money(t.total)}</span></div>
      </div>
    </div>
  </div>

  <div class="sticky bottom-0 z-20 -mx-4 mt-4 flex flex-wrap items-center justify-between gap-2 border-t border-line bg-bg/95 px-4 py-3 backdrop-blur sm:-mx-6 sm:px-6">
    <div class="flex gap-2">
      {#if doc && ['DRAFT', 'SUBMITTED', 'SENT'].includes(doc.status)}<button class="btn btn-danger" onclick={() => setStatus('DELETED', 'Delete', true)}><Icon name="trash" />Delete</button>{/if}
      {#if doc && doc.status === 'AUTHORISED'}<button class="btn" onclick={() => { editing = false; load(); }}>Cancel edit</button>{/if}
    </div>
    <div class="flex flex-wrap gap-2">
      {#if type === 'QUOTE'}
        <button class="btn" disabled={saving} onclick={() => save(doc?.status === 'SENT' ? 'SENT' : undefined)}>Save</button>
        <button class="btn btn-primary" disabled={saving} onclick={() => save('SENT', 'email')}>Save & send</button>
        <button class="btn" disabled={saving} onclick={() => save('SENT')}>Mark as sent</button>
      {:else}
        <button class="btn" disabled={saving} onclick={() => save(doc?.status === 'AUTHORISED' ? 'AUTHORISED' : 'DRAFT')} title={(doc?.status === 'AUTHORISED' ? 'Save changes' : 'Save draft') + ' (Ctrl+S)'}>{doc?.status === 'AUTHORISED' ? 'Save changes' : 'Save draft'}</button>
        {#if doc?.status !== 'AUTHORISED'}
          <button class="btn" disabled={saving} onclick={() => save('SUBMITTED')}>Submit for approval</button>
          <button class="btn" disabled={saving} onclick={() => save('AUTHORISED', 'new')}>Approve & add another</button>
          {#if sales || type === 'PO'}<button class="btn" disabled={saving} onclick={() => save('AUTHORISED', 'email')}>Approve & email</button>{/if}
          <button class="btn" disabled={saving} onclick={() => save('AUTHORISED', 'print')}>Approve & print</button>
          <button class="btn btn-primary" disabled={saving} onclick={() => save('AUTHORISED')} title="Approve (Ctrl+Enter)">Approve</button>
        {/if}
      {/if}
    </div>
  </div>
{:else if doc}
  <!-- ================= VIEW MODE ================= -->
  <div class="grid gap-4 lg:grid-cols-[1fr_340px]">
    <div class="card p-5">
      <div class="mb-5 grid grid-cols-2 gap-4 sm:grid-cols-4">
        <div><div class="label">{sales ? 'To' : 'From'}</div><a class="link font-medium" href="/contacts/{doc.contact_id}">{doc.contact_name}</a></div>
        <div><div class="label">Date</div>{date(doc.date)}</div>
        {#if type === 'QUOTE'}<div><div class="label">Expiry</div>{date(doc.expiry_date)}</div>
        {:else if type === 'PO'}<div><div class="label">Delivery date</div>{date(doc.delivery_date)}</div>
        {:else if !credit}<div><div class="label">Due date</div><span class={doc.overdue ? 'text-bad' : ''}>{date(doc.due_date)}</span></div>{/if}
        <div><div class="label">Reference</div>{doc.reference || '—'}</div>
        {#if doc.currency !== baseCur}<div><div class="label">Currency</div>{doc.currency} @ {fmtRate(doc.rate_e8)}</div>{/if}
      </div>
      {#if type === 'QUOTE' && doc.title}<div class="mb-4"><div class="font-semibold">{doc.title}</div><div class="text-muted">{doc.summary}</div></div>{/if}
      <LineItems lines={f.lines} lat={f.line_amount_type} {sales} readonly />
      <div class="mt-4 flex justify-end">
        <div class="w-full max-w-sm space-y-1.5">
          <div class="flex justify-between"><span class="text-muted">Subtotal</span><span class="num">{money(doc.subtotal)}</span></div>
          {#each doc.tax_summary as tx}<div class="flex justify-between text-muted"><span>{doc.line_amount_type === 'INCLUSIVE' ? 'Includes' : 'Total'} {tx.name}</span><span class="num">{money(tx.tax)}</span></div>{/each}
          <div class="flex justify-between border-t-2 border-text pt-2 text-[17px] font-semibold"><span>Total {doc.currency}</span><span class="num">{money(doc.total)}</span></div>
          {#if doc.currency !== baseCur && doc.rate_e8}<div class="flex justify-between text-[12px] text-faint"><span>{baseCur} equivalent at {fmtRate(doc.rate_e8)}</span><span class="num">{money(Math.round((doc.total * 1e8) / doc.rate_e8))}</span></div>{/if}
          {#if ledger}
            {#each doc.payments as p}<div class="flex justify-between text-muted"><span>Less {credit ? 'refund' : 'payment'} {date(p.date)}</span><span class="num">{money(-p.amount)}</span></div>{/each}
            {#each doc.allocations as a}<div class="flex justify-between text-muted"><span>{a.credit_note_id === doc.id ? `Allocated to ${a.document_number}` : `Less credit ${a.credit_number}`}</span><span class="num">{money(-a.amount)}</span></div>{/each}
            <div class="flex justify-between border-t border-line pt-2 text-[16px] font-semibold"><span>{credit ? 'Remaining credit' : 'Amount due'}</span><span class="num">{money(doc.amount_due)}</span></div>
          {/if}
        </div>
      </div>
      {#if doc.terms}<div class="mt-4 whitespace-pre-wrap text-[13px] text-muted">{doc.terms}</div>{/if}
    </div>

    <aside class="space-y-4">
      {#if can(writePerm)}
        <div class="card p-4">
          <div class="mb-2 font-semibold">Actions</div>
          <div class="flex flex-col gap-2">
            {#if ledger && doc.status === 'AUTHORISED' && doc.amount_paid === 0 && doc.amount_credited === 0}<button class="btn justify-center" onclick={() => (editing = true)}><Icon name="edit" />Edit</button>{/if}
            {#if ['DRAFT', 'SUBMITTED', 'SENT'].includes(doc.status) || (type === 'PO' && doc.status === 'AUTHORISED')}<button class="btn justify-center" onclick={() => (editing = true)}><Icon name="edit" />Edit</button>{/if}
            {#if type === 'QUOTE'}
              {#if doc.status === 'DRAFT'}<button class="btn btn-primary justify-center" onclick={() => setStatus('SENT', 'Mark as sent')}>Mark as sent</button>{/if}
              {#if doc.status === 'SENT'}<button class="btn btn-primary justify-center" onclick={() => setStatus('ACCEPTED', 'Mark as accepted')}>Mark as accepted</button><button class="btn justify-center" onclick={() => setStatus('DECLINED', 'Mark as declined')}>Mark as declined</button>{/if}
              {#if ['SENT', 'ACCEPTED'].includes(doc.status)}<button class="btn justify-center" onclick={() => copyTo('ACCREC')}>Create invoice</button>{/if}
            {/if}
            {#if type === 'PO'}
              {#if ['DRAFT', 'SUBMITTED'].includes(doc.status)}<button class="btn btn-primary justify-center" onclick={() => setStatus('AUTHORISED', 'Approve')}>Approve</button>{/if}
              {#if doc.status === 'AUTHORISED'}<button class="btn btn-primary justify-center" onclick={() => copyTo('ACCPAY')}>Copy to bill</button><button class="btn justify-center" onclick={() => setStatus('BILLED', 'Mark as billed')}>Mark as billed</button>{/if}
            {/if}
            {#if type === 'ACCREC'}<button class="btn justify-center" onclick={() => copyTo('ACCRECCREDIT')}>Create credit note</button>{/if}
            {#if type === 'ACCPAY'}<button class="btn justify-center" onclick={() => copyTo('ACCPAYCREDIT')}>Create supplier credit</button>{/if}
            <button class="btn justify-center" onclick={() => copyTo(type)}><Icon name="copy" />Copy</button>
            {#if type === 'ACCREC' && doc.status === 'AUTHORISED'}<button class="btn justify-center" onclick={async () => { await rpc('docs.markSent', { id: doc.id }); toast('Marked as sent'); load(); }}>Mark as sent</button>{/if}
            {#if ledger && doc.status === 'AUTHORISED'}<button class="btn btn-danger justify-center" onclick={() => setStatus('VOIDED', 'Void', true)}>Void</button>{/if}
          </div>
        </div>
      {/if}

      {#if (type === 'ACCREC' || type === 'ACCPAY') && doc.status === 'AUTHORISED'}
        <div class="card p-4">
          <label class="label" for="pdt">{type === 'ACCREC' ? 'Expected payment date' : 'Planned payment date'}</label>
          <input id="pdt" type="date" class="input" value={type === 'ACCREC' ? doc.expected_payment_date ?? '' : doc.planned_payment_date ?? ''}
            onchange={async (e) => { const v = (e.target as HTMLInputElement).value; try { await rpc('docs.paymentDates', type === 'ACCREC' ? { id: doc.id, expected: v, planned: doc.planned_payment_date } : { id: doc.id, expected: doc.expected_payment_date, planned: v }); toast('Saved — used by the cash flow forecast'); } catch (x) { toastError(x); } }} />
        </div>
      {/if}
      {#if ledger && doc.status === 'AUTHORISED' && can('banking:write')}
        <div class="card p-4">
          <div class="mb-3 font-semibold">{credit ? 'Record refund' : 'Record payment'}</div>
          <div class="space-y-2.5">
            <div><label class="label" for="pa">Amount {doc.currency}</label><input id="pa" class="input num" bind:value={pay.amount} /></div>
            <div><label class="label" for="pd">Date</label><input id="pd" type="date" class="input" bind:value={pay.date} /></div>
            <div><label class="label" for="cb-docedit-credit-sales-paid-fr-2">{credit ? (sales ? 'Paid from' : 'Received into') : sales ? 'Paid to' : 'Paid from'}</label><Combo id="cb-docedit-credit-sales-paid-fr-2" bind:value={pay.account_id} options={bankAccounts()} placeholder="Bank account" /></div>
            {#if doc.currency !== baseCur}<div><label class="label" for="pr">Rate (1 {baseCur} =)</label><input id="pr" class="input num" bind:value={pay.rate} placeholder={fmtRate(doc.rate_e8)} /></div>{/if}
            <div><label class="label" for="pref">Reference</label><input id="pref" class="input" bind:value={pay.reference} /></div>
            <button class="btn btn-primary w-full justify-center" disabled={!pay.account_id} onclick={addPayment}>Add {credit ? 'refund' : 'payment'}</button>
          </div>
        </div>
        {#if credit}
          <div class="card p-4"><div class="mb-2 font-semibold">Allocate credit</div><p class="mb-3 text-[13px] text-muted">Apply this credit to outstanding {sales ? 'invoices' : 'bills'} for {doc.contact_name}.</p><button class="btn w-full justify-center" onclick={openAllocate}>Allocate credit</button></div>
        {:else if credits.length}
          <div class="card p-4">
            <div class="mb-2 font-semibold">Available credits</div>
            {#each credits as c}
              <div class="flex items-center justify-between border-t border-line py-2 text-[13px] first:border-0"><span>{c.reference || c.type} · <span class="num">{money(c.remaining)}</span></span><button class="btn btn-sm" onclick={() => applyCredit(c)}>Apply</button></div>
            {/each}
          </div>
        {/if}
      {/if}

      {#if doc.payments.length || doc.allocations.length}
        <div class="card p-4">
          <div class="mb-2 font-semibold">Payments & credits</div>
          {#each doc.payments as p}
            <div class="flex items-center justify-between border-t border-line py-2 text-[13px] first:border-0">
              <div><div>{date(p.date)} · {p.account_name}</div><div class="text-faint">{p.reference}{p.reconciled ? ' · Reconciled' : ''}</div></div>
              <div class="flex items-center gap-2"><span class="num font-medium">{money(p.amount)}</span>{#if can('banking:write') && !p.reconciled}<button class="text-faint hover:text-bad" title="Remove payment" onclick={() => removePayment(p.id)}><Icon name="trash" size={14} /></button>{/if}</div>
            </div>
          {/each}
          {#each doc.allocations as a}
            <div class="flex items-center justify-between border-t border-line py-2 text-[13px]">
              <div>{date(a.date)} · {a.credit_note_id === doc.id ? `to ${a.document_number}` : `credit ${a.credit_number}`}</div>
              <div class="flex items-center gap-2"><span class="num">{money(a.amount)}</span><button class="text-faint hover:text-bad" title="Remove" onclick={() => removeAllocation(a.id)}><Icon name="trash" size={14} /></button></div>
            </div>
          {/each}
        </div>
      {/if}
    </aside>
  </div>
{/if}

{#if doc}<Attachments entity="document" id={doc.id} />{#key doc}<History entity="document" id={doc.id} />{/key}{/if}
{/if}

<Modal bind:open={emailOpen} title="Send {DOC_LABEL[type].toLowerCase()}" width={620}>
  <div class="space-y-3">
    <div><label class="label" for="eto">To</label><input id="eto" class="input" bind:value={email.to} /></div>
    <div><label class="label" for="esu">Subject</label><input id="esu" class="input" bind:value={email.subject} /></div>
    <div><label class="label" for="ebo">Message</label><textarea id="ebo" rows="10" class="textarea" bind:value={email.body}></textarea></div>
    <p class="text-[12px] text-muted">A link to the online {DOC_LABEL[type].toLowerCase()} is included. Without SMTP settings, messages are saved to Settings → Outbox where you can download them.</p>
  </div>
  {#snippet footer()}<button class="btn" onclick={() => (emailOpen = false)}>Cancel</button><button class="btn btn-primary" onclick={sendEmail}><Icon name="mail" />Send</button>{/snippet}
</Modal>

<Modal bind:open={allocOpen} title="Allocate credit" width={620}>
  {#if openDocs.length}
    <table class="tbl text-[13px]"><thead><tr><th>Number</th><th>Date</th><th class="n">Due</th><th class="n">Allocate</th></tr></thead>
      <tbody>{#each openDocs as o}<tr><td>{o.number}</td><td>{date(o.date)}</td><td class="n">{money(o.amount_due)}</td><td class="n"><input class="input num w-28 text-right" bind:value={alloc[o.id]} placeholder="0.00" /></td></tr>{/each}</tbody></table>
    <div class="mt-3 text-right text-muted">Remaining credit: <span class="num font-semibold text-text">{money(doc?.amount_due ?? 0)}</span></div>
  {:else}<div class="text-muted">No outstanding {sales ? 'invoices' : 'bills'} for this contact in {doc?.currency}.</div>{/if}
  {#snippet footer()}<button class="btn" onclick={() => (allocOpen = false)}>Cancel</button><button class="btn btn-primary" onclick={doAllocate}>Allocate</button>{/snippet}
</Modal>
