<script lang="ts">
  let { title, crumbs = [], children, sub }: { title: string; crumbs?: { label: string; href: string }[]; children?: any; sub?: any } = $props();
  $effect(() => { document.title = `${title} · LedgerCraft`; });
</script>

<div class="mb-5 flex flex-wrap items-end justify-between gap-3">
  <div class="min-w-0">
    {#if crumbs.length}
      <div class="mb-1 flex flex-wrap items-center gap-1.5 text-[13px] text-muted">
        {#each crumbs as c, i}<a class="hover:text-accent" href={c.href}>{c.label}</a><span class="text-faint">›</span>{/each}
      </div>
    {/if}
    <h1 class="truncate text-[22px] font-semibold tracking-tight">{title}</h1>
    {#if sub}<div class="mt-1 text-muted">{@render sub()}</div>{/if}
  </div>
  <div class="flex flex-wrap items-center gap-2">{@render children?.()}</div>
</div>
