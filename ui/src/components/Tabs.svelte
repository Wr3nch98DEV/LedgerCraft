<script lang="ts">
  let { tabs, value = $bindable(), onchange }: { tabs: { key: string; label: string; count?: number | null }[]; value: string; onchange?: (k: string) => void } = $props();
</script>

<div class="mb-4 flex gap-1 overflow-x-auto border-b border-line" role="tablist">
  {#each tabs as t}
    <button role="tab" aria-selected={value === t.key}
      class="-mb-px whitespace-nowrap border-b-2 px-3 py-2 text-[14px] font-medium transition-colors {value === t.key ? 'border-accent text-text' : 'border-transparent text-muted hover:text-text'}"
      onclick={() => { value = t.key; onchange?.(t.key); }}>
      {t.label}{#if t.count !== undefined && t.count !== null}<span class="ml-1.5 rounded-full bg-hover px-1.5 text-[12px] text-muted">{t.count}</span>{/if}
    </button>
  {/each}
</div>
