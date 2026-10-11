<script lang="ts">
  import { rpc } from '../lib/api';
  import { dateTime } from '../lib/format';
  import { toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  let log = $state<any[]>([]);
  let entity = $state('');
  let health = $state<any>(null);
  $effect(() => { rpc('audit.list', { limit: 500, entity: entity || null }).then((l) => (log = l)).catch(toastError); });
  rpc('assurance.health').then((h) => (health = h)).catch(() => {});
  const users = $derived.by(() => { const m: Record<string, number> = {}; for (const l of log) m[l.user ?? 'System'] = (m[l.user ?? 'System'] ?? 0) + 1; return Object.entries(m); });
</script>

<PageHeader title="Assurance dashboard" crumbs={[{ label: 'Accounting', href: '/reports' }]} />
{#if health}
  <div class="mb-4 grid gap-3 sm:grid-cols-4">
    <div class="card p-4"><div class="text-[12px] text-muted">Ledger integrity</div><div class="text-lg font-semibold {health.imbalance === 0 ? 'text-ok' : 'text-bad'}">{health.imbalance === 0 ? 'Balanced' : 'Out of balance!'}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">Posted journals</div><div class="text-lg font-semibold">{health.journals}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">Changes to approved items (30 days)</div><div class="text-lg font-semibold">{health.edits_30d}</div></div>
    <div class="card p-4"><div class="text-[12px] text-muted">Bank lines unreconciled &gt; 30 days</div><div class="text-lg font-semibold {health.stale_unreconciled ? 'text-warn' : ''}">{health.stale_unreconciled}</div></div>
  </div>
  {#if health.flags?.length}<div class="card mb-4 p-4"><div class="mb-2 font-semibold">Unusual activity</div>{#each health.flags as f}<div class="border-t border-line py-1.5 text-[13px] first:border-0">⚠ {f}</div>{/each}</div>{/if}
{/if}
<div class="grid gap-4 lg:grid-cols-[1fr_260px]">
  <div class="card overflow-x-auto">
    <div class="flex items-center justify-between px-4 py-3"><span class="font-semibold">Audit log</span>
      <select class="select w-48" bind:value={entity}><option value="">All records</option>{#each ['document', 'payment', 'contact', 'account', 'bank_transaction', 'manual_journal', 'setting', 'user', 'statement', 'tax_rate', 'item'] as e}<option value={e}>{e.replace('_', ' ')}</option>{/each}</select></div>
    <table class="tbl text-[13px]"><thead><tr><th>When</th><th>User</th><th>Action</th><th>Record</th><th>Details</th></tr></thead>
      <tbody>{#each log as l}<tr><td class="whitespace-nowrap text-muted">{dateTime(l.at)}</td><td>{l.user ?? 'System'}</td><td>{l.action}</td><td>{l.entity}{l.entity_id ? ' #' + l.entity_id : ''}</td><td class="max-w-[420px] truncate font-mono text-[11px] text-muted" title={l.after}>{l.after ?? ''}</td></tr>{/each}</tbody></table>
  </div>
  <div class="card h-fit p-4"><div class="mb-2 font-semibold">Activity by user</div>{#each users as [u, n]}<div class="flex justify-between py-1 text-[13px]"><span>{u}</span><span class="num">{n}</span></div>{/each}
    <p class="mt-3 text-[12px] text-muted">The audit log is append-only; the database rejects edits and deletions.</p></div>
</div>
