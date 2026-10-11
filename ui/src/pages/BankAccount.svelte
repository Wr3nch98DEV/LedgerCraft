<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, date } from '../lib/format';
  import { route, setQuery, navigate } from '../lib/router.svelte';
  import { can, toast, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Tabs from '../components/Tabs.svelte';
  import RecLine from '../components/RecLine.svelte';
  import Icon from '../components/Icon.svelte';
  import Empty from '../components/Empty.svelte';
  import Status from '../components/Status.svelte';

  let { id }: { id: string } = $props();
  let account = $state<any>(null);
  let tab = $state(route.query.get('tab') ?? 'reconcile');
  let lines = $state<any[] | null>(null);
  let stmt = $state<any[]>([]);
  let txs = $state<any[]>([]);
  let pageSize = $state(15);
  let busy = $state(false);

  async function loadAccount() { const all = await rpc('accounts.bank'); account = all.find((a: any) => a.id === Number(id)); }
  async function loadLines() { lines = await rpc('statements.lines', { account_id: Number(id), status: 'UNRECONCILED' }); }
  async function loadTab() {
    if (tab === 'statements') stmt = await rpc('statements.lines', { account_id: Number(id), status: 'ALL', limit: 1000 });
    if (tab === 'transactions') txs = await rpc('bank.transactions', { id: Number(id), limit: 1000 });
  }
  Promise.all([loadAccount(), loadLines()]).catch(toastError);
  $effect(() => { tab; loadTab().catch(toastError); });
  function done(lineId: number) { lines = (lines ?? []).filter((l) => l.id !== lineId); loadAccount(); }
  async function auto() {
    busy = true;
    try { const r = await rpc('rec.auto', { account_id: Number(id) }); toast(`${r.reconciled} line(s) reconciled automatically`); await loadLines(); await loadAccount(); } catch (e) { toastError(e); } finally { busy = false; }
  }
  async function unrec(l: any) { try { await rpc('rec.unreconcile', { id: l.id }); toast('Unreconciled'); loadTab(); loadLines(); loadAccount(); } catch (e) { toastError(e); } }
  function openTx(t: any) {
    if (t.kind === 'payment') navigate(`/doc/${t.document_id}`);
    else if (t.kind === 'banktx') navigate(`/banking/transaction/${t.id}`);
  }
  const shown = $derived((lines ?? []).slice(0, pageSize));
</script>

{#if account}
<PageHeader title={account.name} crumbs={[{ label: 'Bank accounts', href: '/banking' }]}>
  {#snippet sub()}{account.bank_account_number ?? ''} {account.currency}{/snippet}
  <a class="btn" href="/reports/bank-reconciliation?account_id={account.id}"><Icon name="chart" />Reconciliation report</a>
  {#if can('banking:write')}
    <a class="btn" href="/banking/{account.id}/import"><Icon name="upload" />Import statement</a>
    <a class="btn" href="/banking/spend?account={account.id}">Spend money</a>
    <a class="btn" href="/banking/receive?account={account.id}">Receive money</a>
    <a class="btn" href="/banking/transfer?from={account.id}"><Icon name="swap" />Transfer</a>
  {/if}
</PageHeader>

<div class="mb-4 grid gap-3 sm:grid-cols-3">
  <div class="card p-4"><div class="text-[12px] text-muted">Statement balance</div><div class="num text-lg font-semibold">{money(account.statement_balance)}</div></div>
  <div class="card p-4"><div class="text-[12px] text-muted">Balance in LedgerCraft</div><div class="num text-lg font-semibold">{money(account.ledger_balance_fx)}</div></div>
  <div class="card p-4"><div class="text-[12px] text-muted">Items to reconcile</div><div class="num text-lg font-semibold">{account.unreconciled_count}</div></div>
</div>

<Tabs tabs={[{ key: 'reconcile', label: 'Reconcile', count: lines?.length ?? null }, { key: 'statements', label: 'Bank statements' }, { key: 'transactions', label: 'Account transactions' }]}
  bind:value={tab} onchange={(k) => setQuery('tab', k === 'reconcile' ? null : k)} />

{#if tab === 'reconcile'}
  {#if lines === null}<div class="text-muted">Loading…</div>
  {:else if !lines.length}
    <div class="card"><Empty icon="check" title="All reconciled" text="Import a new statement to keep going.">{#if can('banking:write')}<a class="btn btn-primary" href="/banking/{account.id}/import">Import statement</a>{/if}</Empty></div>
  {:else}
    <div class="mb-3 flex items-center justify-between">
      <div class="hidden w-full grid-cols-[minmax(0,1fr)_96px_minmax(0,1.35fr)] text-[12px] font-semibold uppercase tracking-wide text-faint lg:grid"><span>Review your bank statement lines…</span><span></span><span>…then match with your transactions</span></div>
      {#if can('banking:write')}<button class="btn btn-sm shrink-0" disabled={busy} onclick={auto}><Icon name="sparkle" size={14} />Auto-reconcile exact matches</button>{/if}
    </div>
    <div class="space-y-3">
      {#each shown as l (l.id)}<RecLine line={l} {account} ondone={() => done(l.id)} />{/each}
    </div>
    {#if lines.length > pageSize}<div class="mt-4 text-center"><button class="btn" onclick={() => (pageSize += 15)}>Show more ({lines.length - pageSize} remaining)</button></div>{/if}
  {/if}
{:else if tab === 'statements'}
  <div class="card overflow-x-auto">
    <table class="tbl"><thead><tr><th>Date</th><th>Payee</th><th>Description</th><th>Reference</th><th class="n">Spent</th><th class="n">Received</th><th>Status</th><th></th></tr></thead>
      <tbody>{#each stmt.slice().reverse() as l}
        <tr><td class="whitespace-nowrap">{date(l.date)}</td><td>{l.payee}</td><td class="text-muted">{l.description}</td><td class="text-muted">{l.reference}</td>
          <td class="n">{l.amount < 0 ? money(-l.amount) : ''}</td><td class="n">{l.amount > 0 ? money(l.amount) : ''}</td><td><Status status={l.status} /></td>
          <td class="text-right">{#if l.status === 'RECONCILED' && can('banking:write')}<button class="btn btn-ghost btn-sm" onclick={() => unrec(l)}>Unreconcile</button>{/if}</td></tr>
      {:else}<tr><td colspan="8" class="py-8 text-center text-muted">No statement lines imported yet.</td></tr>{/each}</tbody></table>
  </div>
{:else}
  <div class="card overflow-x-auto">
    <table class="tbl"><thead><tr><th>Date</th><th>Description</th><th>Contact / account</th><th>Reference</th><th class="n">Spent</th><th class="n">Received</th><th>Reconciled</th></tr></thead>
      <tbody>{#each txs as t}
        <tr class="clickable" onclick={() => openTx(t)}><td class="whitespace-nowrap">{date(t.date)}</td><td>{t.description}</td><td>{t.contact ?? ''}</td><td class="text-muted">{t.reference}</td>
          <td class="n">{t.amount < 0 ? money(-t.amount) : ''}</td><td class="n">{t.amount > 0 ? money(t.amount) : ''}</td><td>{#if t.reconciled}<Icon name="check" class="text-ok" />{/if}</td></tr>
      {:else}<tr><td colspan="7" class="py-8 text-center text-muted">No transactions yet.</td></tr>{/each}</tbody></table>
  </div>
{/if}
{/if}
