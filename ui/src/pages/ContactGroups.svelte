<script lang="ts">
  import { rpc } from '../lib/api';
  import { can, confirm, toast, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Icon from '../components/Icon.svelte';
  let groups = $state<any[]>([]);
  let name = $state('');
  const load = () => rpc('contactGroups.list').then((g) => (groups = g));
  load();
  async function add() { try { await rpc('contactGroups.create', { name }); name = ''; toast('Group created'); load(); } catch (e) { toastError(e); } }
  async function del(g: any) { if (await confirm(`Delete group "${g.name}"?`, 'Contacts are not deleted.', 'Delete', true)) { await rpc('contactGroups.delete', { id: g.id }); load(); } }
</script>

<PageHeader title="Contact groups" crumbs={[{ label: 'Contacts', href: '/contacts' }]} />
<div class="card max-w-2xl p-5">
  {#if can('contacts:write')}<div class="mb-4 flex gap-2"><input class="input" placeholder="New group name" bind:value={name} onkeydown={(e) => e.key === 'Enter' && add()} /><button class="btn btn-primary" onclick={add} disabled={!name.trim()}>Add group</button></div>{/if}
  <table class="tbl"><thead><tr><th>Group</th><th class="n">Members</th><th></th></tr></thead>
    <tbody>{#each groups as g}<tr><td>{g.name}</td><td class="n">{g.members}</td><td class="text-right">{#if can('contacts:write')}<button class="btn btn-ghost btn-sm" onclick={() => del(g)} aria-label="Delete"><Icon name="trash" size={14} /></button>{/if}</td></tr>
    {:else}<tr><td colspan="3" class="py-6 text-center text-muted">No groups yet. Groups let you filter contacts and send statements in bulk.</td></tr>{/each}</tbody></table>
</div>
