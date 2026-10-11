<script lang="ts">
  // Notification centre + help drawer.
  import { ui } from '../lib/stores.svelte';
  import { rpc } from '../lib/api';
  import { navigate } from '../lib/router.svelte';
  import Icon from './Icon.svelte';
  let items = $state<any[]>([]);
  $effect(() => { if (ui.notifications) rpc('notifications').then((r) => (items = r)).catch(() => {}); });
  const shortcuts = [
    ['Ctrl + K  or  /', 'Search & jump anywhere'], ['Alt + T', 'Toggle light / dark mode'], ['Ctrl + S', 'Save the current form'],
    ['Ctrl + Enter', 'Approve / submit the current form'], ['Esc', 'Close dialogs'], ['Enter / ↑ ↓', 'Pick options in dropdowns'],
  ];
</script>

{#if ui.notifications}
  <div class="no-print fixed inset-0 z-40" role="presentation" onmousedown={() => (ui.notifications = false)}></div>
  <aside class="no-print fixed right-4 top-16 z-50 w-[380px] max-w-[calc(100vw-32px)] rounded-xl border border-line2 bg-panel shadow-[var(--shadow)]">
    <div class="flex items-center justify-between border-b border-line px-4 py-3"><b>Notifications</b><button class="btn btn-ghost btn-sm" onclick={() => (ui.notifications = false)}><Icon name="x" /></button></div>
    <div class="max-h-[60vh] overflow-y-auto p-2">
      {#each items as n}
        <button class="flex w-full items-start gap-3 rounded-lg px-3 py-2.5 text-left hover:bg-hover" onclick={() => { ui.notifications = false; navigate(n.link); }}>
          <span class="mt-0.5 {n.level === 'warn' ? 'text-warn' : 'text-info'}"><Icon name={n.level === 'warn' ? 'warn' : 'info'} /></span>
          <span>{n.text}</span>
        </button>
      {:else}
        <div class="px-3 py-8 text-center text-muted">You're all caught up.</div>
      {/each}
    </div>
  </aside>
{/if}

{#if ui.help}
  <div class="no-print fixed inset-0 z-40 bg-black/40" role="presentation" onmousedown={() => (ui.help = false)}></div>
  <aside class="no-print fixed bottom-0 right-0 top-0 z-50 w-[420px] max-w-full overflow-y-auto border-l border-line2 bg-panel p-5 shadow-[var(--shadow)]">
    <div class="mb-4 flex items-center justify-between"><h2 class="text-lg font-semibold">Help</h2><button class="btn btn-ghost btn-sm" onclick={() => (ui.help = false)}><Icon name="x" /></button></div>
    <h3 class="mb-2 font-semibold">Keyboard shortcuts</h3>
    <div class="mb-5 grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-[13px]">
      {#each shortcuts as [k, d]}<kbd class="rounded border border-line px-1.5 font-mono text-[12px] text-muted">{k}</kbd><span>{d}</span>{/each}
    </div>
    <h3 class="mb-2 font-semibold">Getting around</h3>
    <ul class="mb-5 list-disc space-y-1.5 pl-5 text-[13px] text-muted">
      <li><b class="text-text">Dashboard</b> shows bank balances, money owed to and by you, and cash flow.</li>
      <li><b class="text-text">Business</b> holds invoices, quotes, bills, purchase orders, expense claims, products and projects.</li>
      <li><b class="text-text">Accounting</b> holds bank reconciliation, reports, tax returns and adviser tools (journals, fixed assets, find & recode, budgets).</li>
      <li>Approving an invoice or bill posts it to the ledger. Edits to approved items are recorded as reversals so your audit trail stays intact.</li>
      <li>Every record has <b class="text-text">History & notes</b> at the bottom of its page.</li>
    </ul>
    <h3 class="mb-2 font-semibold">Your data</h3>
    <p class="mb-5 text-[13px] text-muted">Each organisation is a single file in your LedgerCraft data folder. Use <b class="text-text">Settings → Backup</b> to download a copy at any time.</p>
    <div class="rounded-lg border border-warn/30 bg-warn-soft p-3 text-[12px] text-muted">
      LedgerCraft is open-source software and is not affiliated with or endorsed by Xero Limited. Payroll and tax calculations must be verified by a qualified professional; statutory e-filing requires separate government registration.
    </div>
  </aside>
{/if}
