<script lang="ts">
  import { rpc } from '../lib/api';
  import { money } from '../lib/format';
  import { route, setQuery, navigate } from '../lib/router.svelte';
  import { can, confirm, toast, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Tabs from '../components/Tabs.svelte';
  import Icon from '../components/Icon.svelte';
  import Empty from '../components/Empty.svelte';

  let filter = $state(route.query.get('filter') ?? 'all');
  let q = $state('');
  let rows = $state<any[] | null>(null);
  let sel = $state<Set<number>>(new Set());
  let timer: any;
  async function load() { try { rows = await rpc('contacts.list', { filter, q }); sel = new Set(); } catch (e) { toastError(e); } }
  $effect(() => { filter; load(); });
  function search() { clearTimeout(timer); timer = setTimeout(load, 180); }
  function toggle(id: number) { const s = new Set(sel); s.has(id) ? s.delete(id) : s.add(id); sel = s; }
  async function archive(status: string) {
    if (!(await confirm(`${status === 'ARCHIVED' ? 'Archive' : 'Restore'} ${sel.size} contact(s)?`, '', status === 'ARCHIVED' ? 'Archive' : 'Restore'))) return;
    for (const id of sel) await rpc('contacts.setStatus', { id, status }).catch(toastError);
    toast('Contacts updated'); load();
  }
  async function merge() {
    const ids = [...sel];
    if (ids.length !== 2) return;
    const [a, b] = ids.map((id) => rows!.find((r) => r.id === id));
    if (!(await confirm(`Merge "${a.name}" into "${b.name}"?`, `All transactions for ${a.name} will move to ${b.name}, and ${a.name} will be archived.`, 'Merge'))) return;
    try { await rpc('contacts.merge', { from: a.id, into: b.id }); toast('Contacts merged'); load(); } catch (e) { toastError(e); }
  }
  function exportCsv() { location.href = `/download/contacts.csv`; }
</script>

<PageHeader title="Contacts">
  <a class="btn" href="/settings/import?what=contacts"><Icon name="upload" />Import</a>
  <button class="btn" onclick={exportCsv}><Icon name="download" />Export</button>
  {#if can('contacts:write')}<a class="btn btn-primary" href="/contacts/new"><Icon name="plus" />New contact</a>{/if}
</PageHeader>

<Tabs tabs={[{ key: 'all', label: 'All' }, { key: 'customers', label: 'Customers' }, { key: 'suppliers', label: 'Suppliers' }, { key: 'employees', label: 'Employees' }, { key: 'archived', label: 'Archived' }]}
  bind:value={filter} onchange={(k) => setQuery('filter', k === 'all' ? null : k)} />

<div class="mb-3 flex flex-wrap items-center gap-2">
  <div class="relative w-72"><Icon name="search" class="absolute left-2.5 top-1/2 -translate-y-1/2 text-faint" /><input class="input pl-8" placeholder="Search name, email, account number" bind:value={q} oninput={search} /></div>
  {#if sel.size && can('contacts:write')}
    <span class="ml-2 text-muted">{sel.size} selected</span>
    {#if filter === 'archived'}<button class="btn btn-sm" onclick={() => archive('ACTIVE')}>Restore</button>{:else}<button class="btn btn-sm" onclick={() => archive('ARCHIVED')}>Archive</button>{/if}
    {#if sel.size === 2}<button class="btn btn-sm" onclick={merge}>Merge</button>{/if}
  {/if}
</div>

<div class="card overflow-x-auto">
  {#if rows === null}<div class="p-8 text-center text-muted">Loading…</div>
  {:else if !rows.length}<Empty icon="users" title="No contacts found">{#if can('contacts:write')}<a class="btn btn-primary" href="/contacts/new">Add a contact</a>{/if}</Empty>
  {:else}
    <table class="tbl">
      <thead><tr><th class="w-8"></th><th>Name</th><th>Email</th><th>Phone</th><th class="n">You owe them</th><th class="n">They owe you</th><th class="n">Overdue</th></tr></thead>
      <tbody>
        {#each rows as c (c.id)}
          <tr class="clickable {sel.has(c.id) ? 'sel' : ''}" onclick={() => navigate(`/contacts/${c.id}`)}>
            <td onclick={(e) => e.stopPropagation()}><input type="checkbox" checked={sel.has(c.id)} onchange={() => toggle(c.id)} class="accent-[var(--accent)]" aria-label="Select" /></td>
            <td><div class="flex items-center gap-2.5"><span class="grid h-7 w-7 shrink-0 place-items-center rounded-full bg-panel2 text-[11px] font-bold ring-1 ring-line">{c.name.split(' ').map((s: string) => s[0]).join('').slice(0, 2).toUpperCase()}</span>
              <span class="font-medium">{c.name}</span>{#if c.is_customer}<span class="chip bg-accent-soft text-accent">Customer</span>{/if}{#if c.is_supplier}<span class="chip bg-info-soft text-info">Supplier</span>{/if}</div></td>
            <td class="text-muted">{c.email}</td><td class="text-muted">{c.phone}</td>
            <td class="n">{money(c.payable, { blankZero: true })}</td><td class="n">{money(c.receivable, { blankZero: true })}</td>
            <td class="n text-bad">{money(c.overdue, { blankZero: true })}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>
