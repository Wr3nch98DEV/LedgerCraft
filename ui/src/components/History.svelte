<script lang="ts">
  import { rpc } from '../lib/api';
  import { dateTime } from '../lib/format';
  import { toastError } from '../lib/stores.svelte';
  let { entity, id, items = $bindable(null) }: { entity: string; id: number; items?: any[] | null } = $props();
  let note = $state('');
  let open = $state(false);
  async function load() { items = await rpc('history.get', { entity, id }); }
  if (!items) load();
  async function add() {
    if (!note.trim()) return;
    try { await rpc('notes.add', { entity, id, text: note.trim() }); note = ''; await load(); } catch (e) { toastError(e); }
  }
</script>

<section class="card mt-6">
  <button class="flex w-full items-center justify-between px-5 py-3.5 text-left" onclick={() => (open = !open)}>
    <span class="font-semibold">History & notes <span class="ml-1 text-muted">({items?.length ?? 0})</span></span>
    <span class="text-muted">{open ? 'Hide' : 'Show'}</span>
  </button>
  {#if open}
    <div class="border-t border-line px-5 py-4">
      <div class="mb-4 flex gap-2"><input class="input" placeholder="Add a note…" bind:value={note} onkeydown={(e) => e.key === 'Enter' && add()} /><button class="btn" onclick={add}>Add note</button></div>
      <table class="tbl text-[13px]">
        <thead><tr><th>Date</th><th>User</th><th>Details</th></tr></thead>
        <tbody>{#each items ?? [] as h}<tr><td class="whitespace-nowrap text-muted">{dateTime(h.at)}</td><td class="whitespace-nowrap">{h.user ?? 'System'}</td><td>{#if h.kind === 'NOTE'}<span class="chip mr-1 bg-info-soft text-info">Note</span>{/if}{h.text}</td></tr>{/each}</tbody>
      </table>
    </div>
  {/if}
</section>
