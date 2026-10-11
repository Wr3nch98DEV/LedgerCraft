<script lang="ts">
  import { rpc } from '../lib/api';
  import { route } from '../lib/router.svelte';
  import { toast, toastError, loadLookups } from '../lib/stores.svelte';
  import Icon from '../components/Icon.svelte';

  const kinds = [
    ['contacts', 'Contacts', 'ContactName, EmailAddress, FirstName, LastName, POAddressLine1…, TaxNumber, AccountNumber'],
    ['accounts', 'Chart of accounts', '*Code, *Name, *Type, *Tax Code, Description'],
    ['items', 'Products & services', '*ItemCode, ItemName, PurchasesDescription, PurchasesUnitPrice, SalesDescription, SalesUnitPrice'],
    ['invoices', 'Sales invoices', '*ContactName, *InvoiceNumber, *InvoiceDate, *DueDate, Description, *Quantity, *UnitAmount, *AccountCode, *TaxType'],
    ['bills', 'Bills', '*ContactName, *InvoiceNumber, *InvoiceDate, *DueDate, Description, *Quantity, *UnitAmount, *AccountCode, *TaxType'],
    ['journals', 'Manual journals', '*Narration, *Date, Description, *AccountCode, *TaxRate, *Amount'],
    ['trial-balance', 'Opening trial balance', 'AccountCode / Account, Debit, Credit  (posted as conversion balances)'],
  ];
  let what = $state(route.query.get('what') ?? 'contacts');
  let source = $state('XERO');
  let text = $state('');
  let fname = $state('');
  let preview = $state<any>(null);
  let busy = $state(false);
  let convDate = $state('');
  async function pick(f: File) { fname = f.name; text = await f.text(); await dry(); }
  async function dry() {
    if (!text) return;
    try { preview = await rpc('import.run', { what, source, text, dry_run: true, date: convDate || null }); } catch (e) { toastError(e); preview = null; }
  }
  async function run() {
    busy = true;
    try { const r = await rpc('import.run', { what, source, text, dry_run: false, date: convDate || null }); toast(`Imported ${r.created} record(s)${r.skipped ? `, ${r.skipped} skipped` : ''}`); preview = null; text = ''; fname = ''; loadLookups(); }
    catch (e) { toastError(e); } finally { busy = false; }
  }
</script>

<section class="card mb-4 space-y-4 p-5">
  <h2 class="text-lg font-semibold">Import</h2>
  <p class="text-[13px] text-muted">Move from Xero, QuickBooks Online, MYOB or Wave using their CSV exports. Columns are matched by name (case-insensitive); a dry run shows exactly what will be created before anything is saved.</p>
  <div class="grid gap-3 sm:grid-cols-3">
    <div><span class="label">What are you importing?</span><select class="select" bind:value={what} onchange={dry}>{#each kinds as [k, l]}<option value={k}>{l}</option>{/each}</select></div>
    <div><span class="label">From</span><select class="select" bind:value={source} onchange={dry}><option value="XERO">Xero CSV export</option><option value="QBO">QuickBooks Online CSV</option><option value="MYOB">MYOB CSV</option><option value="WAVE">Wave CSV</option><option value="GENERIC">Other / generic CSV</option></select></div>
    {#if what === 'trial-balance'}<div><label class="label" for="cd">Balances as at</label><input id="cd" type="date" class="input" bind:value={convDate} /></div>{/if}
  </div>
  <div class="rounded-lg bg-bg2 p-3 text-[12px] text-muted">Expected columns: {kinds.find((k) => k[0] === what)?.[2]}</div>
  <label class="flex cursor-pointer items-center gap-3 rounded-lg border-2 border-dashed border-line2 px-4 py-5 hover:border-accent">
    <Icon name="upload" class="text-accent" /><span>{fname || 'Choose a CSV file'}</span>
    <input type="file" accept=".csv,.txt" class="hidden" onchange={(e) => { const f = (e.target as HTMLInputElement).files?.[0]; if (f) pick(f); }} />
  </label>
  {#if preview}
    <div class="rounded-lg border border-line p-3 text-[13px]">
      <div class="mb-2 font-semibold">Dry run: {preview.created} to create{preview.skipped ? `, ${preview.skipped} skipped` : ''}</div>
      {#each (preview.messages ?? []).slice(0, 12) as m}<div class="text-muted">• {m}</div>{/each}
      {#if (preview.errors ?? []).length}<div class="mt-2 text-bad">{#each preview.errors.slice(0, 10) as e}<div>• {e}</div>{/each}</div>{/if}
    </div>
    <div class="flex justify-end"><button class="btn btn-primary" disabled={busy || !preview.created} onclick={run}>{busy ? 'Importing…' : `Import ${preview.created} record(s)`}</button></div>
  {/if}
</section>
<section class="card space-y-3 p-5">
  <h2 class="text-lg font-semibold">Export</h2>
  <div class="flex flex-wrap gap-2">
    <a class="btn" href="/download/contacts.csv" download><Icon name="download" />Contacts (CSV)</a>
    <a class="btn" href="/download/accounts.csv" download><Icon name="download" />Chart of accounts (CSV)</a>
    <a class="btn" href="/download/items.csv" download><Icon name="download" />Items (CSV)</a>
    <a class="btn" href="/download/invoices.csv" download><Icon name="download" />Invoices (CSV)</a>
    <a class="btn" href="/download/bills.csv" download><Icon name="download" />Bills (CSV)</a>
    <a class="btn" href="/download/journals.csv" download><Icon name="download" />General ledger (CSV)</a>
    <a class="btn btn-primary" href="/download/archive" download><Icon name="download" />Everything (JSON archive)</a>
  </div>
</section>
