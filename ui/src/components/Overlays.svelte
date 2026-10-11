<script lang="ts">
  // Toasts + confirm dialog, mounted once in App.
  import { toasts, confirmState } from '../lib/stores.svelte';
  import Icon from './Icon.svelte';
  function answer(v: boolean) { confirmState.resolve?.(v); confirmState.open = false; }
</script>

<div class="no-print pointer-events-none fixed bottom-4 right-4 z-[60] flex w-[360px] max-w-[calc(100vw-32px)] flex-col gap-2" aria-live="polite">
  {#each toasts as t (t.id)}
    <div class="pointer-events-auto flex items-start gap-2.5 rounded-lg border px-3.5 py-2.5 shadow-[var(--shadow)]
      {t.kind === 'error' ? 'border-bad/40 bg-panel text-text' : 'border-line2 bg-panel'}">
      <span class="mt-0.5 {t.kind === 'error' ? 'text-bad' : t.kind === 'info' ? 'text-info' : 'text-ok'}"><Icon name={t.kind === 'error' ? 'warn' : t.kind === 'info' ? 'info' : 'check'} /></span>
      <div class="flex-1 text-[14px]">{t.text}</div>
      {#if t.action}<button class="link text-[13px] font-semibold" onclick={() => t.action?.run()}>{t.action.label}</button>{/if}
    </div>
  {/each}
</div>

{#if confirmState.open}
  <div class="fixed inset-0 z-[70] flex items-start justify-center bg-black/55 p-4 pt-[18vh]" role="presentation" onmousedown={(e) => { if (e.target === e.currentTarget) answer(false); }}>
    <div class="card w-full max-w-md p-5 shadow-[var(--shadow)]" role="alertdialog" aria-modal="true">
      <h2 class="text-[16px] font-semibold">{confirmState.title}</h2>
      {#if confirmState.body}<p class="mt-2 text-muted">{confirmState.body}</p>{/if}
      <div class="mt-5 flex justify-end gap-2">
        <button class="btn" onclick={() => answer(false)}>Cancel</button>
        <!-- svelte-ignore a11y_autofocus -->
        <button class="btn {confirmState.danger ? 'btn-danger' : 'btn-primary'}" autofocus onclick={() => answer(true)}>{confirmState.ok}</button>
      </div>
    </div>
  </div>
{/if}
