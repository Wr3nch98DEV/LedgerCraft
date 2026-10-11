<script lang="ts">
  import Icon from './Icon.svelte';
  let { open = $bindable(false), title = '', width = 560, onclose, children, footer }: { open?: boolean; title?: string; width?: number; onclose?: () => void; children?: any; footer?: any } = $props();
  function close() { open = false; onclose?.(); }
  function key(e: KeyboardEvent) { if (e.key === 'Escape' && open) { e.stopPropagation(); close(); } }
</script>

<svelte:window onkeydown={key} />
{#if open}
  <div class="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black/55 p-4 pt-[8vh]" role="presentation" onmousedown={(e) => { if (e.target === e.currentTarget) close(); }}>
    <div class="card w-full shadow-[var(--shadow)]" style="max-width:{width}px" role="dialog" aria-modal="true" aria-label={title}>
      <div class="flex items-center justify-between border-b border-line px-5 py-3.5">
        <h2 class="text-[16px] font-semibold">{title}</h2>
        <button class="btn btn-ghost btn-sm" onclick={close} aria-label="Close"><Icon name="x" /></button>
      </div>
      <div class="px-5 py-4">{@render children?.()}</div>
      {#if footer}<div class="flex justify-end gap-2 border-t border-line px-5 py-3">{@render footer()}</div>{/if}
    </div>
  </div>
{/if}
