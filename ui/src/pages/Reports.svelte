<script lang="ts">
  import { rpc } from '../lib/api';
  import PageHeader from '../components/PageHeader.svelte';
  import Icon from '../components/Icon.svelte';
  let cat = $state<any[]>([]);
  let q = $state('');
  let favs = $state<string[]>([]);
  try { favs = JSON.parse(localStorage.getItem('lc-fav-reports') ?? '["profit-and-loss","balance-sheet","aged-receivables","aged-payables"]'); } catch { favs = []; }
  rpc('reports.catalogue').then((c) => (cat = c));
  function fav(k: string) { favs = favs.includes(k) ? favs.filter((x) => x !== k) : [...favs, k]; try { localStorage.setItem('lc-fav-reports', JSON.stringify(favs)); } catch {} }
  const groups = $derived.by(() => {
    const g: Record<string, any[]> = {};
    for (const r of cat.filter((r) => !q || (r.title + r.description).toLowerCase().includes(q.toLowerCase()))) (g[r.category] ??= []).push(r);
    return Object.entries(g);
  });
</script>

<PageHeader title="Reports">
  <a class="btn" href="/settings/report-packs"><Icon name="doc" />Report packs</a>
  <div class="relative w-64"><Icon name="search" class="absolute left-2.5 top-1/2 -translate-y-1/2 text-faint" /><input class="input pl-8" placeholder="Find a report" bind:value={q} /></div>
</PageHeader>

{#if favs.length && !q}
  <section class="mb-6">
    <h2 class="mb-2 text-[13px] font-semibold uppercase tracking-wide text-faint">Favourites</h2>
    <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
      {#each cat.filter((r) => favs.includes(r.key)) as r}<a href="/reports/{r.key}" class="card p-4 hover:border-accent"><div class="font-semibold">{r.title}</div><div class="text-[13px] text-muted">{r.description}</div></a>{/each}
    </div>
  </section>
{/if}
<div class="grid gap-5 md:grid-cols-2 xl:grid-cols-3">
  {#each groups as [g, list]}
    <section class="card p-4">
      <h2 class="mb-2 font-semibold">{g}</h2>
      {#each list as r}
        <div class="group flex items-start justify-between gap-2 rounded-lg px-2 py-1.5 hover:bg-hover">
          <a href="/reports/{r.key}" class="min-w-0 flex-1"><div class="text-accent">{r.title}</div><div class="truncate text-[12px] text-muted">{r.description}</div></a>
          <button class="mt-0.5 {favs.includes(r.key) ? 'text-warn' : 'text-faint opacity-0 group-hover:opacity-100'}" title="Favourite" onclick={() => fav(r.key)}><Icon name="star" size={15} /></button>
        </div>
      {/each}
    </section>
  {/each}
</div>
