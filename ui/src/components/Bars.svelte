<script lang="ts">
  // Lightweight SVG bar chart. series: [{key, label, color}] ; data: [{label, [key]: cents}]
  import { money } from '../lib/format';
  let { data, series, height = 150 }: { data: any[]; series: { key: string; label: string; color: string }[]; height?: number } = $props();
  let hover = $state<number | null>(null);
  const max = $derived(Math.max(1, ...data.flatMap((d) => series.map((s) => Math.abs(d[s.key] ?? 0)))));
  const W = 100;
  const gw = $derived(W / Math.max(1, data.length));
  const bw = $derived((gw * 0.7) / series.length);
  function short(c: number) {
    const v = Math.abs(c) / 100;
    if (v >= 1e6) return (v / 1e6).toFixed(1) + 'm';
    if (v >= 1e3) return (v / 1e3).toFixed(v >= 1e4 ? 0 : 1) + 'k';
    return v.toFixed(0);
  }
</script>

<div class="relative">
  <svg viewBox="0 0 {W} 100" preserveAspectRatio="none" style="height:{height}px" class="w-full overflow-visible" role="img" aria-label="Bar chart">
    {#each [0.25, 0.5, 0.75] as g}<line x1="0" x2={W} y1={100 - g * 92} y2={100 - g * 92} stroke="var(--line)" stroke-width="0.3" vector-effect="non-scaling-stroke" />{/each}
    {#each data as d, i}
      {#each series as s, j}
        {@const h = (Math.abs(d[s.key] ?? 0) / max) * 92}
        <rect x={i * gw + gw * 0.15 + j * bw} y={100 - h} width={bw * 0.9} height={Math.max(h, 0.6)} rx="0.6" fill={s.color}
          opacity={hover === null || hover === i ? 1 : 0.45} role="presentation" onmouseenter={() => (hover = i)} onmouseleave={() => (hover = null)} />
      {/each}
    {/each}
    <line x1="0" x2={W} y1="100" y2="100" stroke="var(--line2)" stroke-width="1" vector-effect="non-scaling-stroke" />
  </svg>
  <div class="mt-1.5 grid text-center text-[11px] text-muted" style="grid-template-columns:repeat({data.length},1fr)">
    {#each data as d}<div class="truncate">{d.label}</div>{/each}
  </div>
  {#if hover !== null && data[hover]}
    <div class="pointer-events-none absolute top-0 rounded-md border border-line2 bg-panel px-2.5 py-1.5 text-[12px] shadow-[var(--shadow)]" style="left:min(calc({((hover + 0.5) / data.length) * 100}% - 60px), calc(100% - 140px))">
      <div class="font-semibold">{data[hover].label}</div>
      {#each series as s}<div class="flex items-center gap-1.5"><span class="h-2 w-2 rounded-sm" style="background:{s.color}"></span>{s.label}: <span class="num">{money(data[hover][s.key])}</span></div>{/each}
    </div>
  {/if}
  <div class="sr-only">{#each data as d}{d.label}: {#each series as s}{s.label} {short(d[s.key] ?? 0)}; {/each}{/each}</div>
</div>
