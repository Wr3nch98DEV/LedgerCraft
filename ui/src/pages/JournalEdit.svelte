<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, today, parseMoney, moneyPlain } from '../lib/format';
  import { navigate } from '../lib/router.svelte';
  import { can, confirm, lookups, toast, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Combo from '../components/Combo.svelte';
  import History from '../components/History.svelte';
  import Status from '../components/Status.svelte';
  import Icon from '../components/Icon.svelte';
  import { accountOptions, taxOptions, trackingCategories, trackingOptions } from '../components/Pickers';

  let { id = undefined }: { id?: string } = $props();
  let mj = $state<any>(null);
  const blankLine = () => ({ description: '', account_id: null, tax_rate_id: null, debit: '', credit: '', tracking1: null, tracking2: null });
  let f = $state<any>({ date: today(), narration: '', line_amount_type: 'NOTAX', auto_reverse_date: '', show_on_cash_basis: true, lines: [blankLine(), blankLine()] });
  if (id) rpc('mj.get', { id: Number(id) }).then((m) => {
    mj = m;
    f = { date: m.date, narration: m.narration, line_amount_type: m.line_amount_type, auto_reverse_date: m.auto_reverse_date ?? '', show_on_cash_basis: !!m.show_on_cash_basis,
      lines: m.lines.map((l: any) => { const v = l.net_amount + l.tax_amount; const amt = m.line_amount_type === 'EXCLUSIVE' ? l.net_amount : v; return { description: l.description, account_id: l.account_id, tax_rate_id: l.tax_rate_id,
        debit: amt > 0 ? moneyPlain(amt) : '', credit: amt < 0 ? moneyPlain(-amt) : '', tracking1: l.tracking1, tracking2: l.tracking2 }; }) };
  }).catch(toastError);
  const readonly = $derived(mj && mj.status !== 'DRAFT' && mj.status !== 'POSTED');
  const totals = $derived.by(() => {
    let d = 0, c = 0;
    for (const l of f.lines) {
      let dv = parseMoney(l.debit) ?? 0, cv = parseMoney(l.credit) ?? 0;
      const tr = lookups.taxRates.find((t) => t.id === l.tax_rate_id);
      if (f.line_amount_type === 'EXCLUSIVE' && tr) { dv += Math.round((dv * tr.rate_bp) / 10000); cv += Math.round((cv * tr.rate_bp) / 10000); }
      d += dv; c += cv;
    }
    return { d, c };
  });
  async function save(status: string) {
    try {
      const r = await rpc('mj.save', { id: id ? Number(id) : null, journal: { ...f, status, lines: f.lines.filter((l: any) => l.account_id || l.debit || l.credit) } });
      toast(status === 'POSTED' ? 'Journal posted' : 'Draft saved');
      navigate(`/accounting/journals/${r.id}`, { replace: true });
      if (id) location.reload();
    } catch (e) { toastError(e); }
  }
  async function voidIt() {
    if (!(await confirm('Void this journal?', 'It will be reversed in the ledger.', 'Void', true))) return;
    try { await rpc('mj.void', { id: Number(id) }); toast('Journal voided'); navigate('/accounting/journals'); } catch (e) { toastError(e); }
  }
  const cats = trackingCategories();
</script>

<PageHeader title={id ? 'Manual journal' : 'New manual journal'} crumbs={[{ label: 'Manual journals', href: '/accounting/journals' }]}>
  {#snippet sub()}{#if mj}<Status status={mj.status} />{/if}{/snippet}
  {#if mj?.journal_id}<a class="btn" href="/journal/{mj.journal_id}">View posted journal</a>{/if}
</PageHeader>
<div class="card p-5">
  <div class="grid grid-cols-1 gap-4 md:grid-cols-12">
    <div class="md:col-span-5"><label class="label" for="n">Narration *</label><input id="n" class="input" bind:value={f.narration} disabled={readonly} /></div>
    <div class="md:col-span-2"><label class="label" for="d">Date</label><input id="d" type="date" class="input" bind:value={f.date} disabled={readonly} /></div>
    <div class="md:col-span-2"><label class="label" for="r">Auto reversing date</label><input id="r" type="date" class="input" bind:value={f.auto_reverse_date} disabled={readonly} /></div>
    <div class="md:col-span-3"><label class="label" for="l">Amounts are</label><select id="l" class="select" bind:value={f.line_amount_type} disabled={readonly}><option value="NOTAX">No tax</option><option value="EXCLUSIVE">Tax exclusive</option><option value="INCLUSIVE">Tax inclusive</option></select></div>
  </div>
  <div class="mt-5 overflow-x-auto rounded-xl border border-line">
    <table class="tbl min-w-[900px]"><thead><tr><th>Description</th><th class="w-[220px]">Account</th>{#if f.line_amount_type !== 'NOTAX'}<th class="w-[180px]">Tax rate</th>{/if}{#each cats as c}<th class="w-[140px]">{c.name}</th>{/each}<th class="w-[120px] !text-right">Debit</th><th class="w-[120px] !text-right">Credit</th><th class="w-10"></th></tr></thead>
      <tbody>{#each f.lines as l, i}
        <tr><td class="!p-1"><input class="grid-in" bind:value={l.description} disabled={readonly} /></td>
          <td class="!p-1"><Combo compact bind:value={l.account_id} options={accountOptions()} disabled={readonly} /></td>
          {#if f.line_amount_type !== 'NOTAX'}<td class="!p-1"><Combo compact bind:value={l.tax_rate_id} options={taxOptions()} disabled={readonly} /></td>{/if}
          {#each cats as c, k}<td class="!p-1">{#if k === 0}<Combo compact bind:value={l.tracking1} options={trackingOptions(c)} />{:else}<Combo compact bind:value={l.tracking2} options={trackingOptions(c)} />{/if}</td>{/each}
          <td class="!p-1"><input class="grid-in num text-right" bind:value={l.debit} oninput={() => (l.credit = '')} disabled={readonly} /></td>
          <td class="!p-1"><input class="grid-in num text-right" bind:value={l.credit} oninput={() => (l.debit = '')} disabled={readonly} /></td>
          <td class="!p-1">{#if !readonly}<button class="text-faint hover:text-bad" onclick={() => f.lines.splice(i, 1)} aria-label="Remove line"><Icon name="trash" size={14} /></button>{/if}</td></tr>{/each}
        <tr class="font-semibold"><td colspan={2 + (f.line_amount_type !== 'NOTAX' ? 1 : 0) + cats.length} class="text-right">Total</td><td class="n">{money(totals.d)}</td><td class="n">{money(totals.c)}</td><td></td></tr>
        {#if totals.d !== totals.c}<tr><td colspan={4 + (f.line_amount_type !== 'NOTAX' ? 1 : 0) + cats.length} class="text-right text-bad">Out of balance by {money(Math.abs(totals.d - totals.c))}</td><td></td></tr>{/if}
      </tbody></table>
  </div>
  {#if !readonly}<button class="btn btn-sm mt-2" onclick={() => f.lines.push(blankLine())}><Icon name="plus" size={14} />Add a new line</button>{/if}
  <label class="mt-4 flex items-center gap-2 text-[13px]"><input type="checkbox" bind:checked={f.show_on_cash_basis} disabled={readonly} class="accent-[var(--accent)]" />Show journal on cash basis reports</label>
</div>
{#if can('accounting:write') && !readonly}
  <div class="mt-4 flex justify-between">
    <div>{#if mj}<button class="btn btn-danger" onclick={voidIt}>{mj.status === 'DRAFT' ? 'Delete' : 'Void'}</button>{/if}</div>
    <div class="flex gap-2"><button class="btn" onclick={() => save('DRAFT')}>Save as draft</button><button class="btn btn-primary" disabled={totals.d !== totals.c || !totals.d} onclick={() => save('POSTED')}>Post</button></div>
  </div>
{/if}
{#if mj}<History entity="manual_journal" id={mj.id} />{/if}
