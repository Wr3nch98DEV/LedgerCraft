<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, date } from '../lib/format';
  import { navigate } from '../lib/router.svelte';
  import { session, toastError } from '../lib/stores.svelte';
  import Bars from '../components/Bars.svelte';
  import Icon from '../components/Icon.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Modal from '../components/Modal.svelte';

  let d = $state<any>(null);
  rpc('dashboard').then((r) => (d = r)).catch(toastError);
  const cur = $derived(d?.base_currency ?? '');
  const greeting = $derived.by(() => { const h = new Date().getHours(); return h < 12 ? 'Good morning' : h < 18 ? 'Good afternoon' : 'Good evening'; });
  const first = $derived((session.data?.user?.name ?? '').split(' ')[0]);

  // "Edit homepage": show/hide and reorder widgets. A per-browser convenience, so localStorage is fine.
  const WIDGETS = [
    { id: 'banks', label: 'Bank accounts' }, { id: 'sales', label: 'Invoices owed to you' }, { id: 'purchases', label: 'Bills you need to pay' },
    { id: 'cash', label: 'Total cash in and out' }, { id: 'profit', label: 'Profit this month' }, { id: 'watchlist', label: 'Account watchlist' },
  ];
  const LAYOUT_KEY = 'lc.dashboard.layout';
  function loadLayout(): { order: string[]; hidden: string[] } {
    const ids = WIDGETS.map((w) => w.id);
    try {
      const v = JSON.parse(localStorage.getItem(LAYOUT_KEY) ?? 'null');
      if (v && Array.isArray(v.order)) {
        const order = v.order.filter((x: string) => ids.includes(x));
        return { order: [...order, ...ids.filter((x) => !order.includes(x))], hidden: (v.hidden ?? []).filter((x: string) => ids.includes(x)) };
      }
    } catch {}
    return { order: ids, hidden: [] };
  }
  let layout = $state(loadLayout());
  let editing = $state(false);
  function persist() { try { localStorage.setItem(LAYOUT_KEY, JSON.stringify(layout)); } catch {} }
  function move(i: number, by: number) { const o = [...layout.order]; const j = i + by; if (j < 0 || j >= o.length) return; [o[i], o[j]] = [o[j], o[i]]; layout.order = o; persist(); }
  function toggle(id: string) { layout.hidden = layout.hidden.includes(id) ? layout.hidden.filter((x) => x !== id) : [...layout.hidden, id]; persist(); }
  function reset() { layout = { order: WIDGETS.map((w) => w.id), hidden: [] }; persist(); }
  const labelOf = (id: string) => WIDGETS.find((w) => w.id === id)?.label ?? id;

  // Getting-started checklist, hidden once complete or dismissed (remembered per organisation in this browser).
  const gsKey = $derived(`lc.gettingStarted.hidden.${session.data?.org?.file ?? session.data?.org?.name ?? ''}`);
  let gsHidden = $state(false);
  $effect(() => { try { gsHidden = localStorage.getItem(gsKey) === '1'; } catch { gsHidden = false; } });
  function hideGs() { gsHidden = true; try { localStorage.setItem(gsKey, '1'); } catch {} }
  const gs = $derived<any[]>(d?.getting_started ?? []);
  const gsDone = $derived(gs.filter((x) => x.done).length);
</script>

<PageHeader title={`${greeting}${first && first !== 'Owner' ? ', ' + first : ''}`}>
  <a class="btn" href="/cashflow"><Icon name="chart" />Cash flow forecast</a>
  <a class="btn" href="/snapshot"><Icon name="sparkle" />Business snapshot</a>
  <button class="btn" onclick={() => (editing = true)}><Icon name="edit" />Edit homepage</button>
</PageHeader>

<Modal bind:open={editing} title="Edit homepage" width={460}>
  <p class="mb-3 text-[13px] text-muted">Choose which widgets to show and the order they appear in.</p>
  <ul class="space-y-1.5">
    {#each layout.order as id, i}
      <li class="flex items-center gap-2 rounded-lg border border-line px-3 py-2">
        <label class="flex flex-1 items-center gap-2"><input type="checkbox" class="accent-[var(--accent)]" checked={!layout.hidden.includes(id)} onchange={() => toggle(id)} />{labelOf(id)}</label>
        <button class="btn btn-ghost btn-sm" aria-label="Move {labelOf(id)} up" disabled={i === 0} onclick={() => move(i, -1)}>↑</button>
        <button class="btn btn-ghost btn-sm" aria-label="Move {labelOf(id)} down" disabled={i === layout.order.length - 1} onclick={() => move(i, 1)}>↓</button>
      </li>
    {/each}
  </ul>
  {#snippet footer()}<button class="btn" onclick={reset}>Reset to default</button><button class="btn btn-primary" onclick={() => (editing = false)}>Done</button>{/snippet}
</Modal>

{#if !d}
  <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">{#each Array(6) as _}<div class="card h-56 animate-pulse"></div>{/each}</div>
{:else}
  {#if gs.length && gsDone < gs.length && !gsHidden}
    <section class="card mb-4 p-5">
      <div class="mb-3 flex items-center justify-between gap-3">
        <div><div class="text-[15px] font-semibold">Get set up</div><div class="text-[13px] text-muted">{gsDone} of {gs.length} done — each step ticks itself off as you go.</div></div>
        <button class="btn btn-ghost btn-sm" onclick={hideGs}>Hide</button>
      </div>
      <div class="mb-3 h-1.5 overflow-hidden rounded-full bg-bg2"><div class="h-full rounded-full bg-accent" style="width:{(gsDone / gs.length) * 100}%"></div></div>
      <ul class="grid gap-1 sm:grid-cols-2">
        {#each gs as step}
          <li><a href={step.link} class="flex items-center gap-2 rounded-lg px-2 py-1.5 text-[14px] hover:bg-hover {step.done ? 'text-muted line-through' : ''}">
            <span class="grid h-5 w-5 shrink-0 place-items-center rounded-full border {step.done ? 'border-accent bg-accent text-accent-ink' : 'border-line2'}">{#if step.done}<Icon name="check" size={12} />{/if}</span>{step.text}</a></li>
        {/each}
      </ul>
    </section>
  {/if}
  {#if d.tasks.length}
    <div class="mb-4 flex flex-wrap gap-2">
      {#each d.tasks as t}
        <a href={t.link} class="flex items-center gap-2 rounded-lg border px-3 py-1.5 text-[13px] {t.level === 'warn' ? 'border-warn/30 bg-warn-soft' : 'border-info/30 bg-info-soft'}">
          <Icon name={t.level === 'warn' ? 'warn' : 'info'} size={14} class={t.level === 'warn' ? 'text-warn' : 'text-info'} />{t.text}
        </a>
      {/each}
    </div>
  {/if}

  {#snippet w_banks()}
    {#each d.banks.filter((b: any) => b.status === 'ACTIVE') as b}
      <section class="card flex flex-col p-5">
        <div class="flex items-start justify-between">
          <div><a href="/banking/{b.id}" class="text-[15px] font-semibold hover:text-accent">{b.name}</a><div class="text-[12px] text-faint">{b.bank_account_number ?? ''} {b.currency !== cur ? b.currency : ''}</div></div>
          <Icon name="bank" class="text-muted" />
        </div>
        <div class="mt-4 grid grid-cols-2 gap-3">
          <div><div class="text-[12px] text-muted">Balance in LedgerCraft</div><div class="num text-xl font-semibold">{money(b.ledger_balance_fx)}</div></div>
          {#if b.last_statement_date}
            <div><div class="text-[12px] text-muted">Statement balance</div><div class="num text-xl font-semibold text-muted">{money(b.statement_balance)}</div>
              <div class="text-[11px] text-faint">as of {date(b.last_statement_date)}</div></div>
          {:else}
            <div><div class="text-[12px] text-muted">Statement balance</div><div class="text-[13px] text-faint">No statement lines imported yet</div></div>
          {/if}
        </div>
        <div class="mt-auto pt-4">
          {#if b.unreconciled_count > 0}
            <a class="btn btn-primary w-full justify-center" href="/banking/{b.id}">Reconcile {b.unreconciled_count} item{b.unreconciled_count === 1 ? '' : 's'}</a>
          {:else}
            <div class="flex gap-2"><a class="btn flex-1 justify-center" href="/banking/{b.id}/import"><Icon name="upload" />Import statement</a><a class="btn flex-1 justify-center" href="/banking/{b.id}">Transactions</a></div>
          {/if}
        </div>
      </section>
    {/each}
  {/snippet}
  {#snippet w_sales()}
    <section class="card p-5">
      <div class="mb-3 flex items-center justify-between"><a href="/sales/invoices" class="text-[15px] font-semibold hover:text-accent">Invoices owed to you</a><a class="btn btn-sm" href="/sales/invoices/new"><Icon name="plus" size={14} />New</a></div>
      <div class="mb-3 grid grid-cols-3 gap-2 text-[13px]">
        <a href="/sales/invoices?tab=draft" class="rounded-lg bg-bg2 px-2.5 py-2 hover:bg-hover"><div class="text-muted">{d.sales.draft_count} draft</div><div class="num font-semibold">{money(d.sales.draft_total)}</div></a>
        <a href="/sales/invoices?tab=awaiting" class="rounded-lg bg-bg2 px-2.5 py-2 hover:bg-hover"><div class="text-muted">{d.sales.awaiting_count} awaiting</div><div class="num font-semibold">{money(d.sales.awaiting_total)}</div></a>
        <a href="/sales/invoices?tab=overdue" class="rounded-lg bg-bad-soft px-2.5 py-2 hover:brightness-110"><div class="text-bad">{d.sales.overdue_count} overdue</div><div class="num font-semibold">{money(d.sales.overdue_total)}</div></a>
      </div>
      <Bars data={d.sales_buckets} series={[{ key: 'amount', label: 'Due', color: 'var(--accent)' }]} height={110} />
    </section>
  {/snippet}
  {#snippet w_purchases()}
    <section class="card p-5">
      <div class="mb-3 flex items-center justify-between"><a href="/purchases/bills" class="text-[15px] font-semibold hover:text-accent">Bills you need to pay</a><a class="btn btn-sm" href="/purchases/bills/new"><Icon name="plus" size={14} />New</a></div>
      <div class="mb-3 grid grid-cols-3 gap-2 text-[13px]">
        <a href="/purchases/bills?tab=draft" class="rounded-lg bg-bg2 px-2.5 py-2 hover:bg-hover"><div class="text-muted">{d.purchases.draft_count} draft</div><div class="num font-semibold">{money(d.purchases.draft_total)}</div></a>
        <a href="/purchases/bills?tab=awaiting" class="rounded-lg bg-bg2 px-2.5 py-2 hover:bg-hover"><div class="text-muted">{d.purchases.awaiting_count} to pay</div><div class="num font-semibold">{money(d.purchases.awaiting_total)}</div></a>
        <a href="/purchases/bills?tab=overdue" class="rounded-lg bg-bad-soft px-2.5 py-2 hover:brightness-110"><div class="text-bad">{d.purchases.overdue_count} overdue</div><div class="num font-semibold">{money(d.purchases.overdue_total)}</div></a>
      </div>
      <Bars data={d.purchase_buckets} series={[{ key: 'amount', label: 'Due', color: 'var(--info)' }]} height={110} />
    </section>
  {/snippet}
  {#snippet w_cash()}
    <section class="card p-5">
      <div class="mb-1 flex items-center justify-between"><span class="text-[15px] font-semibold">Total cash in and out</span><span class="text-[12px] text-muted">Last 6 months</span></div>
      <div class="mb-3 text-[13px] text-muted">Total cash <span class="num font-semibold text-text">{cur} {money(d.total_cash)}</span></div>
      <Bars data={d.cash_flow} series={[{ key: 'in', label: 'Cash in', color: 'var(--ok)' }, { key: 'out', label: 'Cash out', color: 'var(--bad)' }]} height={130} />
    </section>
  {/snippet}
  {#snippet w_profit()}
    <section class="card p-5">
      <div class="mb-4 flex items-center justify-between"><a href="/reports/profit-and-loss" class="text-[15px] font-semibold hover:text-accent">Profit this month</a><span class="text-[12px] text-muted">Accrual</span></div>
      <div class="space-y-3">
        <div class="flex justify-between"><span class="text-muted">Income</span><span class="num font-medium">{money(d.pl_month.income)}</span></div>
        <div class="flex justify-between"><span class="text-muted">Expenses</span><span class="num font-medium">{money(d.pl_month.expenses)}</span></div>
        <div class="h-2 overflow-hidden rounded-full bg-bg2"><div class="h-full rounded-full {d.pl_month.net >= 0 ? 'bg-ok' : 'bg-bad'}" style="width:{d.pl_month.income ? Math.min(100, Math.abs(d.pl_month.net) / Math.max(d.pl_month.income, 1) * 100) : 0}%"></div></div>
        <div class="flex justify-between border-t border-line pt-3"><span class="font-semibold">Net profit</span><span class="num text-lg font-semibold {d.pl_month.net >= 0 ? 'text-ok' : 'text-bad'}">{money(d.pl_month.net)}</span></div>
      </div>
    </section>
  {/snippet}
  {#snippet w_watchlist()}
    <section class="card p-5">
      <div class="mb-3 flex items-center justify-between"><span class="text-[15px] font-semibold">Account watchlist</span><a class="text-[12px] text-muted hover:text-accent" href="/accounting/chart">Edit</a></div>
      {#if d.watchlist.length}
        <table class="tbl text-[13px]"><thead><tr><th>Account</th><th class="n">This month</th><th class="n">YTD</th></tr></thead>
          <tbody>{#each d.watchlist as w}
            {@const s = w.class === 'REVENUE' || w.class === 'LIABILITY' || w.class === 'EQUITY' ? -1 : 1}
            <tr class="clickable" onclick={() => navigate(`/reports/account-transactions?account_id=${w.id}`)}><td>{w.name}</td><td class="n">{money(s * w.this_month)}</td><td class="n">{money(s * w.ytd)}</td></tr>{/each}</tbody></table>
      {:else}<div class="text-muted">Tick "Show on dashboard watchlist" on any account to track it here.</div>{/if}
    </section>
  {/snippet}

  <div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
    {#each layout.order.filter((x) => !layout.hidden.includes(x)) as id}
      {#if id === 'banks'}{@render w_banks()}{:else if id === 'sales'}{@render w_sales()}{:else if id === 'purchases'}{@render w_purchases()}
      {:else if id === 'cash'}{@render w_cash()}{:else if id === 'profit'}{@render w_profit()}{:else if id === 'watchlist'}{@render w_watchlist()}{/if}
    {/each}
  </div>
{/if}
