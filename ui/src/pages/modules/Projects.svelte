<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, date } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { can, toast, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Modal from '../../components/Modal.svelte';
  import ContactSelect from '../../components/ContactSelect.svelte';
  import Tabs from '../../components/Tabs.svelte';
  import Icon from '../../components/Icon.svelte';
  let d = $state<any>(null);
  let tab = $state('INPROGRESS');
  const load = () => rpc('projects.list', { status: tab }).then((r) => (d = r)).catch(toastError);
  $effect(() => { tab; load(); });
  let open = $state(false);
  let f = $state<any>({ name: '', contact_id: null, deadline: '', estimate: '' });
  async function create() { try { const r = await rpc('projects.save', { project: f }); open = false; navigate(`/projects/${r.id}`); } catch (e) { toastError(e); } }
  async function stop() { try { const r = await rpc('projects.stopTimer'); toast(`Timer stopped (${r.minutes} min)`); load(); } catch (e) { toastError(e); } }
  const hm = (m: number) => `${Math.floor(m / 60)}h ${m % 60}m`;
</script>

<PageHeader title="Projects">
  <a class="btn" href="/reports/project-profitability">Profitability report</a>
  {#if can('projects:write')}<button class="btn btn-primary" onclick={() => { f = { name: '', contact_id: null, deadline: '', estimate: '' }; open = true; }}><Icon name="plus" />New project</button>{/if}
</PageHeader>
{#if d?.timer}<div class="card mb-4 flex items-center justify-between border-accent/40 p-4"><div><Icon name="clock" class="mr-1 inline text-accent" /> Timer running on <b>{d.timer.project}</b> · {d.timer.task}</div><button class="btn btn-primary" onclick={stop}><Icon name="pause" />Stop timer</button></div>{/if}
<Tabs tabs={[{ key: 'INPROGRESS', label: 'In progress' }, { key: 'CLOSED', label: 'Closed' }, { key: 'ALL', label: 'All' }]} bind:value={tab} />
<div class="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
  {#each d?.projects ?? [] as p}
    <button class="card p-5 text-left hover:border-accent" onclick={() => navigate(`/projects/${p.id}`)}>
      <div class="flex items-start justify-between"><div><div class="text-[16px] font-semibold">{p.name}</div><div class="text-[13px] text-muted">{p.contact_name ?? 'No contact'}{p.deadline ? ` · due ${date(p.deadline)}` : ''}</div></div></div>
      <div class="mt-4 grid grid-cols-3 gap-2 text-[13px]">
        <div><div class="text-muted">Time</div><div class="font-semibold">{hm(p.summary.minutes)}</div></div>
        <div><div class="text-muted">To invoice</div><div class="num font-semibold">{money(p.summary.to_invoice)}</div></div>
        <div><div class="text-muted">Profit</div><div class="num font-semibold {p.summary.profit < 0 ? 'text-bad' : ''}">{money(p.summary.profit)}</div></div>
      </div>
      {#if p.summary.estimate}<div class="mt-3 h-1.5 overflow-hidden rounded-full bg-bg2"><div class="h-full bg-accent" style="width:{Math.min(100, ((p.summary.invoiced + p.summary.to_invoice) / p.summary.estimate) * 100)}%"></div></div><div class="mt-1 text-[11px] text-faint">{money(p.summary.invoiced + p.summary.to_invoice)} of {money(p.summary.estimate)} estimate</div>{/if}
    </button>
  {:else}<div class="card p-10 text-center text-muted md:col-span-3">No projects. Track time and costs against jobs, then invoice them in one click.</div>{/each}
</div>
<Modal bind:open title="New project">
  <div class="space-y-3">
    <div><label class="label" for="pn">Project name</label><input id="pn" class="input" bind:value={f.name} /></div>
    <div><label class="label" for="cb-projects-contact-1">Contact</label><ContactSelect id="cb-projects-contact-1" bind:value={f.contact_id} /></div>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="pd">Deadline</label><input id="pd" type="date" class="input" bind:value={f.deadline} /></div><div><label class="label" for="pe">Estimate</label><input id="pe" class="input num" bind:value={f.estimate} /></div></div>
  </div>
  {#snippet footer()}<button class="btn" onclick={() => (open = false)}>Cancel</button><button class="btn btn-primary" disabled={!f.name} onclick={create}>Create</button>{/snippet}
</Modal>
