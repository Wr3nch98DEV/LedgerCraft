<script lang="ts">
  // Generic renderer for report JSON produced by the server.
  import { money, qty, fixed, date } from '../lib/format';
  import { navigate } from '../lib/router.svelte';
  let { report, drill = true, showPct = false }: { report: any; drill?: boolean; showPct?: boolean } = $props();
  let expanded = $state<Set<string>>(new Set());
  const cols: string[] = $derived(report.columns ?? []);
  const qtyCols: number[] = $derived(report.qty_cols ?? []);
  const countCols: number[] = $derived(report.count_cols ?? []);
  const income: number[] | null = $derived(showPct ? report.income ?? null : null);
  function cell(v: any, i: number, row: any) {
    if (v === null || v === undefined) return '';
    if (typeof v === 'string') return v;
    if (row?.format === 'pct') return fixed(v, 2) + '%';
    if (row?.format === 'pctx') return v + '%';
    if (row?.format === 'num' || countCols.includes(i)) return String(v);
    if (qtyCols.includes(i)) return qty(v);
    return money(v);
  }
  function go(row: any) {
    if (!drill) return;
    if (row.document_id) navigate(`/doc/${row.document_id}`);
    else if (row.source_type && row.source_id) navigate(sourcePath(row.source_type, row.source_id, row.journal_id));
    else if (row.account_id && report.title !== 'Account Transactions') navigate(`/reports/account-transactions?account_id=${row.account_id}`);
    else if (row.contact_id) navigate(`/contacts/${row.contact_id}`);
    else if (row.item_id) navigate(`/products/${row.item_id}`);
  }
  function sourcePath(t: string, id: number, jid?: number) {
    if (['ACCREC', 'ACCPAY', 'ACCRECCREDIT', 'ACCPAYCREDIT', 'PAYMENT'].includes(t)) return t === 'PAYMENT' ? `/journal/${jid}` : `/doc/${id}`;
    if (t === 'CASHREC' || t === 'CASHPAID') return `/banking/transaction/${id}`;
    if (t === 'MANJOURNAL') return `/accounting/journals/${id}`;
    return `/journal/${jid}`;
  }
  const clickable = (r: any) => drill && (r.document_id || r.account_id || r.contact_id || r.item_id || (r.source_type && r.source_id));
</script>

<div class="overflow-x-auto">
  <table class="tbl report text-[14px]">
    <thead><tr><th class="min-w-[260px]"></th>{#if report.detail}<th>Date</th><th>Source</th><th>Reference</th>{/if}{#each cols as c}<th class="!text-right">{c}</th>{/each}{#if income}<th class="!text-right">% of income</th>{/if}{#if report.has_budget}<th class="!text-right">Budget</th><th class="!text-right">Variance</th>{/if}</tr></thead>
    <tbody>
      {#each report.sections as s, si}
        {#if s.title}<tr><td colspan={cols.length + 1 + (report.detail ? 3 : 0) + (income ? 1 : 0) + (report.has_budget ? 2 : 0)} class="!border-0 !pb-1 !pt-5 text-[15px] font-semibold">{s.title}</td></tr>{/if}
        {#each s.rows ?? [] as r, ri}
          {#if r.heading}<tr><td colspan={cols.length + 1} class="!pt-4 font-semibold">{r.label}</td></tr>
          {:else if r.text}<tr><td colspan={cols.length + 1} class="whitespace-pre-wrap italic text-muted">{r.label}</td></tr>
          {:else}
          <tr class="{clickable(r) ? 'clickable' : ''} {r.bold ? 'font-semibold' : ''}" onclick={() => go(r)}>
            <td class="{r.children ? 'font-medium' : ''}">
              {#if r.children?.length}<button class="mr-1 text-faint" onclick={(e) => { e.stopPropagation(); const k = si + ':' + ri; const n = new Set(expanded); n.has(k) ? n.delete(k) : n.add(k); expanded = n; }}>{expanded.has(si + ':' + ri) ? '▾' : '▸'}</button>{/if}
              <span class={clickable(r) ? 'text-accent' : ''}>{r.label ?? ''}</span>{#if r.contact}<span class="ml-2 text-[12px] text-muted">{r.contact}</span>{/if}{#if r.flag}<span class="chip ml-2 bg-warn-soft text-warn">{r.flag}</span>{/if}
            </td>
            {#if report.detail}<td class="whitespace-nowrap text-muted">{date(r.date)}</td><td class="whitespace-nowrap text-muted">{r.source_label ?? ''}</td><td class="text-muted">{r.reference ?? ''}</td>{/if}
            {#each r.values ?? [] as v, i}<td class="n">{cell(v, i, r)}</td>{/each}
            {#if income}<td class="n text-muted">{income[0] ? fixed(Math.round((r.values[0] / income[0]) * 10000), 2) + '%' : ''}</td>{/if}
            {#if report.has_budget}<td class="n text-muted">{money(r.budget)}</td><td class="n {r.variance < 0 ? 'text-bad' : 'text-ok'}">{money(r.variance)}</td>{/if}
          </tr>
          {/if}
          {#if r.children && expanded.has(si + ':' + ri)}
            {#each r.children as c}
              <tr class="clickable text-[13px]" onclick={() => go(c)}><td class="pl-10 text-accent">{c.label}{#if c.due_date}<span class="ml-2 text-muted">due {date(c.due_date)}</span>{/if}</td>{#each c.values as v, i}<td class="n">{cell(v, i, c)}</td>{/each}</tr>
            {/each}
          {/if}
        {/each}
        {#if s.total}
          <tr class="{s.grand ? 'text-[15px]' : ''}">
            <td class="font-semibold {s.summary ? '' : '!border-t-2 !border-t-[var(--line2)]'} {s.grand ? '!border-y-2 !border-y-[var(--text)]' : ''}">{s.total.label}</td>
            {#if report.detail}<td class={s.grand ? '!border-y-2 !border-y-[var(--text)]' : ''} colspan="3"></td>{/if}
            {#each s.total.values ?? [] as v, i}<td class="n font-semibold {s.summary ? '' : '!border-t-2 !border-t-[var(--line2)]'} {s.grand ? '!border-y-2 !border-y-[var(--text)]' : ''}">{cell(v, i, null)}</td>{/each}
            {#if income}<td class="n text-muted">{income[0] ? fixed(Math.round(((s.total.values[0] ?? 0) / income[0]) * 10000), 2) + '%' : ''}</td>{/if}
            {#if report.has_budget}<td class="n font-semibold text-muted">{money(s.total.budget)}</td><td class="n font-semibold {s.total.variance < 0 ? 'text-bad' : 'text-ok'}">{money(s.total.variance)}</td>{/if}
          </tr>
        {/if}
      {/each}
    </tbody>
  </table>
</div>
