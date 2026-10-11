<script lang="ts">
  import { rpc } from '../lib/api';
  import { navigate } from '../lib/router.svelte';
  import { toast, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Icon from '../components/Icon.svelte';

  let { id }: { id: string } = $props();
  let file = $state<File | null>(null);
  let text = $state('');
  let fmt = $state('');
  let rows = $state<string[][]>([]);
  let mappings = $state<any[]>([]);
  let m = $state({ has_header: true, date_col: 0, date_format: 'DMY', amount_col: 1 as number | null, debit_col: null as number | null, credit_col: null as number | null,
    payee_col: null as number | null, description_col: null as number | null, reference_col: null as number | null, negate: false });
  let split = $state(false);
  let saveAs = $state('');
  let busy = $state(false);
  let drag = $state(false);
  let accountName = $state('Account');
  rpc('statements.mappings', { account_id: Number(id) }).then((x) => (mappings = x)).catch(() => {});
  rpc('accounts.bank').then((all: any[]) => { const a = all.find((x) => x.id === Number(id)); if (a) accountName = a.name; }).catch(() => {});

  async function pick(f: File) {
    file = f; text = await f.text();
    const p = await rpc('statements.preview', { filename: f.name, text });
    fmt = p.format; rows = p.rows;
    if (fmt === 'CSV') guess();
  }
  function guess() {
    const h = (rows[0] ?? []).map((x) => x.toLowerCase());
    const find = (...ks: string[]) => { const i = h.findIndex((c) => ks.some((k) => c.includes(k))); return i >= 0 ? i : null; };
    m.has_header = h.some((c) => /date|amount|desc/.test(c));
    if (m.has_header) {
      m.date_col = find('date') ?? 0;
      const amt = find('amount', 'value');
      const dr = find('debit', 'withdraw', 'paid out'); const cr = find('credit', 'deposit', 'paid in');
      if (amt === null && dr !== null && cr !== null) { split = true; m.debit_col = dr; m.credit_col = cr; m.amount_col = null; } else { m.amount_col = amt ?? 1; }
      m.payee_col = find('payee', 'name', 'merchant', 'counterparty');
      m.description_col = find('description', 'narrative', 'details', 'memo', 'transaction');
      m.reference_col = find('reference', 'ref', 'cheque');
    }
    const sample = rows[m.has_header ? 1 : 0]?.[m.date_col] ?? '';
    if (/^\d{4}-/.test(sample)) m.date_format = 'YMD';
    if (mappings[0]) Object.assign(m, mappings[0].mapping);
  }
  async function doImport() {
    busy = true;
    try {
      const mapping = fmt === 'CSV' ? { ...m, amount_col: split ? null : m.amount_col, debit_col: split ? m.debit_col : null, credit_col: split ? m.credit_col : null } : null;
      const r = await rpc('statements.import', { account_id: Number(id), filename: file?.name ?? 'statement', text, format: fmt, mapping, save_mapping_as: saveAs || null, date_order: m.date_format });
      toast(`Imported ${r.added} line(s)${r.duplicates ? `, skipped ${r.duplicates} duplicate(s)` : ''}`);
      navigate(`/banking/${id}`);
    } catch (e) { toastError(e); } finally { busy = false; }
  }
  const cols = $derived((rows[0] ?? []).map((c, i) => ({ i, label: m.has_header ? c || `Column ${i + 1}` : `Column ${i + 1}` })));
</script>

<PageHeader title="Import a bank statement" crumbs={[{ label: 'Bank accounts', href: '/banking' }, { label: accountName, href: `/banking/${id}` }]} />

<div class="card max-w-4xl p-5">
  <label class="flex cursor-pointer flex-col items-center justify-center rounded-xl border-2 border-dashed px-6 py-10 text-center transition {drag ? 'border-accent bg-accent-soft' : 'border-line2 hover:border-accent'}"
    ondragover={(e) => { e.preventDefault(); drag = true; }} ondragleave={() => (drag = false)} ondrop={(e) => { e.preventDefault(); drag = false; const f = e.dataTransfer?.files[0]; if (f) pick(f); }}>
    <Icon name="upload" size={26} class="text-accent" />
    <div class="mt-2 font-semibold">{file ? file.name : 'Drop a statement file here, or click to browse'}</div>
    <div class="text-[13px] text-muted">CSV · OFX · QFX · QIF · CAMT.053 (XML) · MT940 — duplicates are skipped automatically</div>
    <input type="file" class="hidden" accept=".csv,.ofx,.qfx,.qif,.xml,.sta,.mt940,.txt" onchange={(e) => { const f = (e.target as HTMLInputElement).files?.[0]; if (f) pick(f); }} />
  </label>

  {#if file}
    <div class="mt-4 text-[13px] text-muted">Detected format: <b class="text-text">{fmt}</b></div>
    {#if fmt === 'CSV'}
      <div class="mt-4 grid grid-cols-2 gap-3 md:grid-cols-4">
        <label class="col-span-2 flex items-center gap-2 md:col-span-4"><input type="checkbox" bind:checked={m.has_header} class="accent-[var(--accent)]" />First row is a header</label>
        <div><span class="label">Date column</span><select class="select" bind:value={m.date_col}>{#each cols as c}<option value={c.i}>{c.label}</option>{/each}</select></div>
        <div><span class="label">Date format</span><select class="select" bind:value={m.date_format}><option value="DMY">Day/Month/Year</option><option value="MDY">Month/Day/Year</option><option value="YMD">Year-Month-Day</option></select></div>
        <label class="col-span-2 mt-5 flex items-center gap-2"><input type="checkbox" bind:checked={split} class="accent-[var(--accent)]" />Separate debit and credit columns</label>
        {#if split}
          <div><span class="label">Money out (debit)</span><select class="select" bind:value={m.debit_col}>{#each cols as c}<option value={c.i}>{c.label}</option>{/each}</select></div>
          <div><span class="label">Money in (credit)</span><select class="select" bind:value={m.credit_col}>{#each cols as c}<option value={c.i}>{c.label}</option>{/each}</select></div>
        {:else}
          <div><span class="label">Amount column</span><select class="select" bind:value={m.amount_col}>{#each cols as c}<option value={c.i}>{c.label}</option>{/each}</select></div>
          <label class="mt-5 flex items-center gap-2"><input type="checkbox" bind:checked={m.negate} class="accent-[var(--accent)]" />Flip signs</label>
        {/if}
        <div><span class="label">Payee</span><select class="select" bind:value={m.payee_col}><option value={null}>— none —</option>{#each cols as c}<option value={c.i}>{c.label}</option>{/each}</select></div>
        <div><span class="label">Description</span><select class="select" bind:value={m.description_col}><option value={null}>— none —</option>{#each cols as c}<option value={c.i}>{c.label}</option>{/each}</select></div>
        <div><span class="label">Reference</span><select class="select" bind:value={m.reference_col}><option value={null}>— none —</option>{#each cols as c}<option value={c.i}>{c.label}</option>{/each}</select></div>
        <div><span class="label">Save mapping as</span><input class="input" bind:value={saveAs} placeholder="e.g. My bank CSV" /></div>
      </div>
      <div class="mt-4 overflow-x-auto rounded-lg border border-line"><table class="tbl text-[12px]"><tbody>
        {#each rows as r, i}<tr class={i === 0 && m.has_header ? 'font-semibold text-muted' : ''}>{#each r as c}<td class="whitespace-nowrap">{c}</td>{/each}</tr>{/each}
      </tbody></table></div>
    {/if}
    <div class="mt-5 flex justify-end gap-2"><a class="btn" href="/banking/{id}">Cancel</a><button class="btn btn-primary" disabled={busy} onclick={doImport}>{busy ? 'Importing…' : 'Import'}</button></div>
  {/if}
</div>
