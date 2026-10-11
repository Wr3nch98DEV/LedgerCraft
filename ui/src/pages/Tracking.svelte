<script lang="ts">
  import { rpc } from '../lib/api';
  import { can, toast, toastError, loadLookups } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Icon from '../components/Icon.svelte';
  let cats = $state<any[]>([]);
  let newCat = $state('');
  let newOpt = $state<Record<number, string>>({});
  const load = () => rpc('tracking.list').then((c) => (cats = c)).catch(toastError);
  load();
  async function addCat() { try { await rpc('tracking.save', { name: newCat, options: [] }); newCat = ''; load(); loadLookups(); } catch (e) { toastError(e); } }
  async function addOpt(c: any) { try { await rpc('tracking.save', { id: c.id, name: c.name, options: [newOpt[c.id]] }); newOpt[c.id] = ''; load(); loadLookups(); } catch (e) { toastError(e); } }
  async function rename(c: any) { await rpc('tracking.save', { id: c.id, name: c.name, options: [] }).catch(toastError); toast('Saved'); loadLookups(); }
  async function optStatus(o: any) { await rpc('tracking.optionStatus', { id: o.id, status: o.status === 'ACTIVE' ? 'ARCHIVED' : 'ACTIVE' }); load(); loadLookups(); }
  async function catStatus(c: any) { try { await rpc('tracking.categoryStatus', { id: c.id, status: c.status === 'ACTIVE' ? 'ARCHIVED' : 'ACTIVE' }); load(); loadLookups(); } catch (e) { toastError(e); } }
</script>

<PageHeader title="Tracking categories" crumbs={[{ label: 'Accounting', href: '/reports' }]} />
<p class="mb-4 max-w-2xl text-muted">Track income and expenses by department, location, project or anything else. Up to two categories can be active at a time; each can have unlimited options. Run reports filtered by any option.</p>
<div class="grid gap-4 md:grid-cols-2">
  {#each cats as c}
    <section class="card p-5 {c.status !== 'ACTIVE' ? 'opacity-60' : ''}">
      <div class="mb-3 flex gap-2"><input class="input font-semibold" bind:value={c.name} onblur={() => rename(c)} disabled={!can('accounting:write')} /><button class="btn btn-sm" onclick={() => catStatus(c)}>{c.status === 'ACTIVE' ? 'Archive' : 'Restore'}</button></div>
      {#each c.options as o}<div class="flex items-center justify-between border-t border-line py-1.5 {o.status !== 'ACTIVE' ? 'text-faint line-through' : ''}"><span>{o.name}</span>{#if can('accounting:write')}<button class="text-[12px] text-muted hover:text-text" onclick={() => optStatus(o)}>{o.status === 'ACTIVE' ? 'Archive' : 'Restore'}</button>{/if}</div>{/each}
      {#if can('accounting:write')}<div class="mt-3 flex gap-2"><input class="input" placeholder="New option" bind:value={newOpt[c.id]} onkeydown={(e) => e.key === 'Enter' && addOpt(c)} /><button class="btn" onclick={() => addOpt(c)}>Add</button></div>{/if}
    </section>
  {/each}
  {#if can('accounting:write') && cats.filter((c) => c.status === 'ACTIVE').length < 2}
    <section class="card flex flex-col justify-center gap-2 p-5"><div class="font-semibold">Add a tracking category</div><div class="flex gap-2"><input class="input" placeholder="e.g. Department" bind:value={newCat} /><button class="btn btn-primary" disabled={!newCat} onclick={addCat}><Icon name="plus" />Add</button></div></section>
  {/if}
</div>
