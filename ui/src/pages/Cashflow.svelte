<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, date, today } from '../lib/format';
  import { toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Icon from '../components/Icon.svelte';
  let days = $state(30);
  let adj = $state<{ date: string; label: string; amount: string }[]>([]);
  let d = $state<any>(null);
  async function load() { try { d = await rpc('cashflow.forecast', { days, adjustments: adj.filter((a) => a.amount) }); } catch (e) { toastError(e); } }
  $effect(() => { days; load(); });
  const max = $derived(d ? Math.max(1, ...d.series.map((s: any) => Math.abs(s.balance))) : 1);
  const min = $derived(d ? Math.min(0, ...d.series.map((s: any) => s.balance)) : 0);
  const pts = $derived(d ? d.series.map((s: any, i: number) => `${(i / Math.max(1, d.series.length - 1)) * 100},${100 - ((s.balance - min) / (max - min || 1)) * 90 - 5}`).join(' ') : '');
  const zeroY = $derived(100 - ((0 - min) / (max - min || 1)) * 90 - 5);
</script>

<PageHeader title="Short-term cash flow" crumbs={[{ label: 'Dashboard', href: '/' }]}>
  <div class="flex rounded-lg border border-line p-0.5">{#each [7, 30, 60, 90] as n}<button class="rounded-md px-3 py-1 text-[13px] {days === n ? 'bg-accent-soft text-accent' : 'text-muted'}" onclick={() => (days = n)}>{n} days</button>{/each}</div>
</PageHeader>
{#if d}
  <div class="mb-4 grid gap-3 sm:grid-cols-4">
    <div class="card p-4"><div class="text-[12px] text-muted">Cash today</div><div class="num text-lg font-semibold">{money(d.opening)}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">Money in</div><div class="num text-lg font-semibold text-ok">{money(d.total_in)}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">Money out</div><div class="num text-lg font-semibold text-bad">{money(d.total_out)}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">Projected in {days} days</div><div class="num text-lg font-semibold {d.closing < 0 ? 'text-bad' : ''}">{money(d.closing)}</div></div>
  </div>
  <div class="card mb-4 p-5">
    <svg viewBox="0 0 100 100" preserveAspectRatio="none" class="h-56 w-full" role="img" aria-label="Projected balance">
      <line x1="0" x2="100" y1={zeroY} y2={zeroY} stroke="var(--line2)" stroke-dasharray="1 1" vector-effect="non-scaling-stroke" />
      <polyline points={pts} fill="none" stroke="var(--accent)" stroke-width="2" vector-effect="non-scaling-stroke" />
    </svg>
    <div class="flex justify-between text-[12px] text-muted"><span>{date(d.series[0]?.date)}</span><span>{date(d.series[d.series.length - 1]?.date)}</span></div>
  </div>
  <div class="grid gap-4 lg:grid-cols-[1fr_380px]">
    <div class="card overflow-x-auto"><div class="px-4 py-3 font-semibold">Scheduled</div>
      <table class="tbl text-[13px]"><thead><tr><th>Date</th><th>Item</th><th class="n">In</th><th class="n">Out</th></tr></thead>
        <tbody>{#each d.events as e}<tr><td>{date(e.date)}</td><td>{e.label}</td><td class="n text-ok">{e.amount > 0 ? money(e.amount) : ''}</td><td class="n text-bad">{e.amount < 0 ? money(-e.amount) : ''}</td></tr>
        {:else}<tr><td colspan="4" class="py-6 text-center text-muted">Nothing scheduled in this window.</td></tr>{/each}</tbody></table></div>
    <div class="card h-fit p-4">
      <div class="mb-2 font-semibold">What if…</div>
      <p class="mb-3 text-[13px] text-muted">Add one-off amounts (negative for money out) to see the effect. Change planned dates on bills (Planned payment date) to move them.</p>
      {#each adj as a, i}<div class="mb-2 grid grid-cols-[120px_1fr_90px_auto] gap-1"><input type="date" class="input" bind:value={a.date} /><input class="input" bind:value={a.label} placeholder="Label" /><input class="input num" bind:value={a.amount} placeholder="-500" /><button class="btn btn-ghost btn-sm" onclick={() => { adj.splice(i, 1); load(); }} aria-label="Remove"><Icon name="x" size={14} /></button></div>{/each}
      <div class="flex gap-2"><button class="btn btn-sm" onclick={() => adj.push({ date: today(), label: '', amount: '' })}><Icon name="plus" size={14} />Add</button><button class="btn btn-sm btn-primary" onclick={load}>Recalculate</button></div>
    </div>
  </div>
{/if}
