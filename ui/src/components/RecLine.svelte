<script lang="ts">
  // One statement line in the reconcile view: statement on the left, match/create on the right.
  import { rpc } from '../lib/api';
  import { money, moneyPlain, date, DOC_LABEL } from '../lib/format';
  import { lookups, toast, toastError } from '../lib/stores.svelte';
  import Combo from './Combo.svelte';
  import ContactSelect from './ContactSelect.svelte';
  import Icon from './Icon.svelte';
  import { accountOptions, taxOptions, bankAccounts, trackingCategories, trackingOptions } from './Pickers';

  let { line, account, ondone }: { line: any; account: any; ondone: () => void } = $props();
  let tab = $state<'match' | 'create' | 'transfer' | 'discuss'>('match');
  let sugg = $state<any>(null);
  let busy = $state(false);
  let findOpen = $state(false);
  let picked = $state<Map<string, { kind: string; id: number; amount: number; label: string; doc?: boolean; pay?: string }>>(new Map());
  let all = $state<{ ledger: any[]; docs: any[] }>({ ledger: [], docs: [] });
  let findQ = $state('');

  const spent = line.amount < 0;
  const abs = Math.abs(line.amount);
  let create = $state({ contact_id: null as number | null, contact_name: '', account_id: null as number | null, tax_rate_id: null as number | null, description: line.description || line.payee, tracking1: null, tracking2: null, lines: null as any[] | null, kind: 'normal' });
  let transferTo = $state<number | null>(null);
  let note = $state(line.notes ?? '');

  async function load() {
    try {
      sugg = await rpc('rec.suggestions', { id: line.id });
      if (sugg.rule) applyRule(sugg.rule.rule);
      else if (!sugg.matches.length && !sugg.documents.length) tab = 'create';
    } catch (e) { toastError(e); }
  }
  load();

  function applyRule(r: any) {
    if (r.type === 'TRANSFER') { transferTo = r.transfer_account_id; tab = 'transfer'; return; }
    if (!sugg.matches.length && !sugg.documents.length) tab = 'create';
    create.contact_id = r.contact_id ?? null;
    const pl = r.percent_lines ?? [];
    const fl = r.fixed_lines ?? [];
    if (pl.length + fl.length === 1) {
      const l = pl[0] ?? fl[0];
      create.account_id = l.account_id; create.tax_rate_id = l.tax_rate_id ?? lookups.accounts.find((a) => a.id === l.account_id)?.tax_rate_id ?? null;
      if (l.description) create.description = l.description;
      create.tracking1 = l.tracking1 ?? null; create.tracking2 = l.tracking2 ?? null;
    } else {
      // split by fixed amounts first, remainder by percentage
      let remaining = abs;
      const lines: any[] = [];
      for (const l of fl) { const a = Math.round(Number(l.amount || 0) * 100); remaining -= a; lines.push({ ...l, amount: moneyPlain(a) }); }
      let used = 0;
      pl.forEach((l: any, i: number) => {
        const a = i === pl.length - 1 ? remaining - used : Math.round((remaining * Number(l.percent || 0)) / 100);
        used += a; lines.push({ ...l, amount: moneyPlain(a) });
      });
      create.lines = lines;
    }
  }

  async function okMatch(m: any) {
    busy = true;
    try { await rpc('rec.match', { id: line.id, targets: [{ kind: m.kind, id: m.id }] }); toast('Reconciled'); ondone(); } catch (e) { toastError(e); } finally { busy = false; }
  }
  async function okDoc(d: any) {
    busy = true;
    try { await rpc('rec.pay', { id: line.id, documents: [{ id: d.id, amount: moneyPlain(Math.min(abs, d.amount_due)) }] }); toast('Payment recorded and reconciled'); ondone(); }
    catch (e) { toastError(e); } finally { busy = false; }
  }
  async function okCreate() {
    busy = true;
    try {
      const lines = create.lines
        ? create.lines.map((l) => ({ description: l.description || create.description, account_id: l.account_id, tax_rate_id: l.tax_rate_id, amount: l.amount, tracking1: l.tracking1, tracking2: l.tracking2 }))
        : [{ description: create.description, account_id: create.account_id, tax_rate_id: create.tax_rate_id, tracking1: create.tracking1, tracking2: create.tracking2 }];
      await rpc('rec.create', { id: line.id, tx: { type: create.kind === 'overpayment' ? 'OVERPAYMENT' : create.kind === 'prepayment' ? 'PREPAYMENT' : '', contact_id: create.contact_id, line_amount_type: 'INCLUSIVE', reference: line.reference, lines } });
      toast('Transaction created and reconciled'); ondone();
    } catch (e) { toastError(e); } finally { busy = false; }
  }
  async function okTransfer() {
    busy = true;
    try { await rpc('rec.transfer', { id: line.id, account_id: transferTo }); toast('Transfer reconciled'); ondone(); } catch (e) { toastError(e); } finally { busy = false; }
  }
  async function saveNote() { await rpc('statements.note', { id: line.id, note }); toast('Note saved'); }
  async function del() { try { await rpc('statements.deleteLine', { id: line.id }); toast('Statement line deleted'); ondone(); } catch (e) { toastError(e); } }

  async function openFind() {
    findOpen = true;
    const [ledger, docs] = await Promise.all([
      rpc('rec.unreconciled', { account_id: account.id }),
      rpc('docs.list', { type: spent ? 'PURCHASES' : 'SALES', tab: 'awaiting', limit: 500 })
    ]);
    all = { ledger, docs: docs.rows };
  }
  function key(kind: string, id: number) { return kind + ':' + id; }
  function togglePick(kind: string, id: number, amount: number, label: string, doc = false) {
    const m = new Map(picked);
    const k = key(kind, id);
    if (m.has(k)) m.delete(k); else m.set(k, { kind, id, amount, label, doc, pay: doc ? moneyPlain(Math.abs(amount)) : undefined });
    picked = m;
  }
  function setPay(k: string, v: string) { const m = new Map(picked); const e = m.get(k); if (e) m.set(k, { ...e, pay: v }); picked = m; }
  const pickedTotal = $derived([...picked.values()].reduce((s, p) => s + (p.doc ? Math.round(Number(p.pay || 0) * 100) * Math.sign(p.amount) : p.amount), 0));
  async function okFind() {
    busy = true;
    try {
      const docs = [...picked.values()].filter((p) => p.doc).map((p) => ({ id: p.id, amount: p.pay }));
      const targets = [...picked.values()].filter((p) => !p.doc).map((p) => ({ kind: p.kind, id: p.id }));
      if (docs.length) await rpc('rec.pay', { id: line.id, documents: docs, targets });
      else await rpc('rec.match', { id: line.id, targets });
      toast('Reconciled'); ondone();
    } catch (e) { toastError(e); } finally { busy = false; }
  }
  const fq = $derived(findQ.toLowerCase());
  const ledgerShown = $derived(all.ledger.filter((x) => !fq || `${x.contact} ${x.reference} ${x.description} ${money(x.amount)}`.toLowerCase().includes(fq)));
  const docsShown = $derived(all.docs.filter((x) => !fq || `${x.contact_name} ${x.number} ${x.reference} ${money(x.amount_due)}`.toLowerCase().includes(fq)));
  const cats = trackingCategories();
  const best = $derived(sugg?.matches?.[0]);
  const bestDoc = $derived(sugg?.documents?.[0]);
  const canCreate = $derived(create.lines ? create.lines.every((l) => l.account_id) : !!create.account_id);
</script>

<div class="grid grid-cols-1 gap-0 overflow-hidden rounded-xl border border-line bg-panel lg:grid-cols-[minmax(0,1fr)_96px_minmax(0,1.35fr)]">
  <!-- Statement side -->
  <div class="border-b border-line p-4 lg:border-b-0 lg:border-r">
    <div class="flex items-start justify-between gap-3">
      <div class="min-w-0">
        <div class="text-[13px] text-muted">{date(line.date)}</div>
        <div class="truncate font-semibold">{line.payee || line.description}</div>
        {#if line.payee && line.description}<div class="truncate text-[13px] text-muted">{line.description}</div>{/if}
        {#if line.reference}<div class="truncate text-[12px] text-faint">Ref: {line.reference}</div>{/if}
      </div>
      <div class="text-right">
        <div class="text-[11px] uppercase tracking-wide text-faint">{spent ? 'Spent' : 'Received'}</div>
        <div class="num text-lg font-semibold {spent ? '' : 'text-ok'}">{money(abs)}</div>
      </div>
    </div>
    <div class="mt-3 flex gap-3 text-[12px]">
      <button class="text-faint hover:text-bad" onclick={del}>Delete line</button>
      {#if line.notes}<span class="text-info">Has note</span>{/if}
    </div>
  </div>

  <!-- OK -->
  <div class="flex items-center justify-center border-b border-line bg-bg2 p-3 lg:border-b-0 lg:border-r">
    {#if tab === 'match' && best && best.amount === line.amount}
      <button class="btn btn-primary w-full justify-center" disabled={busy} onclick={() => okMatch(best)}>OK</button>
    {:else if tab === 'match' && bestDoc}
      <button class="btn btn-primary w-full justify-center" disabled={busy} onclick={() => okDoc(bestDoc)}>OK</button>
    {:else if tab === 'create'}
      <button class="btn btn-primary w-full justify-center" disabled={busy || !canCreate} onclick={okCreate}>OK</button>
    {:else if tab === 'transfer'}
      <button class="btn btn-primary w-full justify-center" disabled={busy || !transferTo} onclick={okTransfer}>OK</button>
    {:else}<span class="text-[12px] text-faint">—</span>{/if}
  </div>

  <!-- Ledger side -->
  <div class="p-4">
    <div class="mb-3 flex gap-1 text-[13px]">
      {#each [['match', 'Match'], ['create', 'Create'], ['transfer', 'Transfer'], ['discuss', 'Discuss']] as [k, l]}
        <button class="rounded-md px-2.5 py-1 font-medium {tab === k ? 'bg-accent-soft text-accent' : 'text-muted hover:bg-hover'}" onclick={() => (tab = k as any)}>{l}</button>
      {/each}
      {#if sugg?.rule}<span class="chip ml-auto self-center bg-info-soft text-info">Rule: {sugg.rule.rule.name}</span>{/if}
    </div>

    {#if !sugg}
      <div class="text-muted">Finding matches…</div>
    {:else if tab === 'match'}
      {#if best && best.amount === line.amount}
        <div class="rounded-lg border border-ok/30 bg-ok-soft px-3 py-2.5">
          <div class="flex justify-between"><span class="font-medium">{best.contact ?? best.description}</span><span class="num font-semibold">{money(Math.abs(best.amount))}</span></div>
          <div class="text-[12px] text-muted">{date(best.date)} · {best.description}{best.reference ? ' · ' + best.reference : ''}</div>
        </div>
      {:else if bestDoc}
        <div class="rounded-lg border border-info/30 bg-info-soft px-3 py-2.5">
          <div class="flex justify-between"><span class="font-medium">{DOC_LABEL[bestDoc.type]} {bestDoc.number} · {bestDoc.contact}</span><span class="num font-semibold">{money(bestDoc.amount_due)}</span></div>
          <div class="text-[12px] text-muted">Due {date(bestDoc.due_date)}{bestDoc.reference ? ' · ' + bestDoc.reference : ''} — OK records the payment</div>
        </div>
      {:else}
        <div class="text-[13px] text-muted">No suggested match. Use <b>Find & match</b>, or <button class="link" onclick={() => (tab = 'create')}>create a transaction</button>.</div>
      {/if}
      <button class="btn btn-sm mt-3" onclick={openFind}><Icon name="search" size={14} />Find & match</button>
    {:else if tab === 'create'}
      {#if create.lines}
        <div class="space-y-2">
          <ContactSelect bind:value={create.contact_id} placeholder="Who (contact)" />
          {#each create.lines as l}
            <div class="grid grid-cols-[1fr_1fr_100px] gap-2"><Combo bind:value={l.account_id} options={accountOptions((a) => a.type !== 'BANK')} placeholder="Account" /><Combo bind:value={l.tax_rate_id} options={taxOptions()} placeholder="Tax" /><input class="input num text-right" bind:value={l.amount} /></div>
          {/each}
          <div class="text-[12px] text-muted">Split from bank rule. <button class="link" onclick={() => (create.lines = null)}>Use a single line</button></div>
        </div>
      {:else}
        <div class="grid grid-cols-2 gap-2">
          <div class="col-span-2 sm:col-span-1"><ContactSelect bind:value={create.contact_id} placeholder="Who (contact)" /></div>
          <div class="col-span-2 sm:col-span-1"><Combo bind:value={create.account_id} options={accountOptions((a) => a.type !== 'BANK')} placeholder="What (account)" onchange={(v) => { if (v) create.tax_rate_id = lookups.accounts.find((a) => a.id === v)?.tax_rate_id ?? create.tax_rate_id; }} /></div>
          <input class="input col-span-2 sm:col-span-1" bind:value={create.description} placeholder="Why (description)" />
          <div class="col-span-2 sm:col-span-1"><Combo bind:value={create.tax_rate_id} options={taxOptions()} placeholder="Tax rate" /></div>
          {#each cats as c, k}<div><Combo value={k === 0 ? create.tracking1 : create.tracking2} options={trackingOptions(c)} placeholder={c.name} onchange={(v) => { if (k === 0) create.tracking1 = v; else create.tracking2 = v; }} /></div>{/each}
        </div>
        <div class="mt-2 flex items-center gap-3 text-[12px] text-muted">
          <label class="flex items-center gap-1"><input type="radio" bind:group={create.kind} value="normal" class="accent-[var(--accent)]" />{spent ? 'Spend money' : 'Receive money'}</label>
          <label class="flex items-center gap-1"><input type="radio" bind:group={create.kind} value="overpayment" class="accent-[var(--accent)]" />Overpayment</label>
          <label class="flex items-center gap-1"><input type="radio" bind:group={create.kind} value="prepayment" class="accent-[var(--accent)]" />Prepayment</label>
          <a class="ml-auto link" href="/banking/rules?from={line.id}">Create rule</a>
        </div>
      {/if}
    {:else if tab === 'transfer'}
      <div class="grid grid-cols-[1fr] gap-2">
        <Combo bind:value={transferTo} options={bankAccounts().filter((o) => o.value !== account.id)} placeholder={spent ? 'Transfer to which account?' : 'Transfer from which account?'} />
        <div class="text-[12px] text-muted">Creates a bank transfer of {money(abs)} {spent ? 'to' : 'from'} the selected account.</div>
      </div>
    {:else}
      <textarea class="textarea" rows="2" bind:value={note} placeholder="Add a note for your accountant or team"></textarea>
      <button class="btn btn-sm mt-2" onclick={saveNote}>Save note</button>
    {/if}
  </div>
</div>

{#if findOpen}
  <div class="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black/55 p-4 pt-[5vh]" role="presentation" onmousedown={(e) => { if (e.target === e.currentTarget) findOpen = false; }}>
    <div class="card w-full max-w-4xl shadow-[var(--shadow)]">
      <div class="flex items-center justify-between border-b border-line px-5 py-3">
        <div><div class="font-semibold">Find & match</div><div class="text-[13px] text-muted">{line.payee} · {date(line.date)} · {spent ? 'spent' : 'received'} <span class="num">{money(abs)}</span></div></div>
        <button class="btn btn-ghost btn-sm" onclick={() => (findOpen = false)}><Icon name="x" /></button>
      </div>
      <div class="p-5">
        <input class="input mb-3" placeholder="Search by name, reference or amount" bind:value={findQ} />
        <div class="mb-1 text-[12px] font-semibold uppercase tracking-wide text-faint">Unreconciled transactions</div>
        <div class="mb-4 max-h-60 overflow-y-auto rounded-lg border border-line">
          <table class="tbl text-[13px]"><tbody>
            {#each ledgerShown as x}
              <tr class="clickable {picked.has(key(x.kind, x.id)) ? 'sel' : ''}" onclick={() => togglePick(x.kind, x.id, x.amount, `${x.contact ?? ''} ${x.reference ?? ''}`)}>
                <td class="w-8"><input type="checkbox" checked={picked.has(key(x.kind, x.id))} class="accent-[var(--accent)]" /></td><td>{date(x.date)}</td><td>{x.contact}</td><td class="text-muted">{x.description} {x.reference}</td>
                <td class="n">{x.amount < 0 ? money(-x.amount) : ''}</td><td class="n">{x.amount > 0 ? money(x.amount) : ''}</td></tr>
            {:else}<tr><td class="py-4 text-center text-muted">None</td></tr>{/each}
          </tbody></table>
        </div>
        <div class="mb-1 text-[12px] font-semibold uppercase tracking-wide text-faint">Outstanding {spent ? 'bills' : 'invoices'} (records a payment)</div>
        <div class="max-h-60 overflow-y-auto rounded-lg border border-line">
          <table class="tbl text-[13px]"><tbody>
            {#each docsShown as x}
              {@const sign = x.type === 'ACCREC' || x.type === 'ACCPAYCREDIT' ? 1 : -1}
              <tr class="{picked.has(key('DOC', x.id)) ? 'sel' : ''}">
                <td class="w-8"><input type="checkbox" checked={picked.has(key('DOC', x.id))} onchange={() => togglePick('DOC', x.id, sign * x.amount_due, `${x.number} ${x.contact_name}`, true)} class="accent-[var(--accent)]" /></td>
                <td>{date(x.due_date)}</td><td>{x.contact_name}</td><td class="text-muted">{DOC_LABEL[x.type]} {x.number} {x.reference}</td><td class="n">{money(x.amount_due)}</td>
                <td class="n w-32">{#if picked.has(key('DOC', x.id))}<input class="input num h-8 text-right" value={picked.get(key('DOC', x.id))!.pay} oninput={(e) => setPay(key('DOC', x.id), (e.target as HTMLInputElement).value)} />{/if}</td></tr>
            {:else}<tr><td class="py-4 text-center text-muted">None</td></tr>{/each}
          </tbody></table>
        </div>
      </div>
      <div class="flex items-center justify-between border-t border-line px-5 py-3">
        <div class="text-[13px]">Selected <span class="num font-semibold">{money(Math.abs(pickedTotal))}</span> of <span class="num">{money(abs)}</span>
          {#if pickedTotal !== line.amount && picked.size}<span class="ml-2 text-warn">Difference {money(Math.abs(line.amount - pickedTotal))}</span>{/if}</div>
        <div class="flex gap-2"><button class="btn" onclick={() => (findOpen = false)}>Cancel</button><button class="btn btn-primary" disabled={busy || !picked.size || pickedTotal !== line.amount} onclick={okFind}>Reconcile</button></div>
      </div>
    </div>
  </div>
{/if}
