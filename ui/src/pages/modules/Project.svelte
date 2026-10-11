<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, date, today, unitPrice } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { can, confirm, toast, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Tabs from '../../components/Tabs.svelte';
  import Modal from '../../components/Modal.svelte';
  import Status from '../../components/Status.svelte';
  import Icon from '../../components/Icon.svelte';
  let { id }: { id: string } = $props();
  let p = $state<any>(null);
  let tab = $state('time');
  const load = () => rpc('projects.get', { id: Number(id) }).then((r) => (p = r)).catch(toastError);
  load();
  const hm = (m: number) => `${Math.floor(m / 60)}:${String(m % 60).padStart(2, '0')}`;
  let te = $state({ task_id: null as number | null, date: today(), hours: '', description: '' });
  async function addTime() {
    const [h, m] = te.hours.includes(':') ? te.hours.split(':').map(Number) : [Number(te.hours), 0];
    const minutes = Math.round((h || 0) * 60 + (m || 0));
    try { await rpc('projects.addTime', { project_id: Number(id), task_id: te.task_id, date: te.date, minutes, description: te.description }); te.hours = ''; te.description = ''; toast('Time added'); load(); } catch (e) { toastError(e); }
  }
  async function startTimer() { try { await rpc('projects.startTimer', { project_id: Number(id), task_id: te.task_id, description: te.description }); toast('Timer started'); load(); } catch (e) { toastError(e); } }
  async function stopTimer() { try { await rpc('projects.stopTimer'); toast('Timer stopped'); load(); } catch (e) { toastError(e); } }
  let taskOpen = $state(false);
  let tf = $state<any>({ name: '', charge_type: 'TIME', rate: '' });
  async function saveTask() { try { await rpc('projects.saveTask', { id: tf.id ?? null, project_id: Number(id), task: tf }); taskOpen = false; load(); } catch (e) { toastError(e); } }
  let costOpen = $state(false);
  let cf = $state<any>({ description: '', amount: '', date: today(), chargeable: true, markup: '' });
  async function saveCost() { try { await rpc('projects.addCost', { project_id: Number(id), cost: cf }); costOpen = false; load(); } catch (e) { toastError(e); } }
  async function invoice() { try { const r = await rpc('projects.invoice', { id: Number(id) }); toast('Draft invoice created'); navigate(`/sales/invoices/${r.id}`); } catch (e) { toastError(e); } }
  async function close() { await rpc('projects.save', { id: Number(id), project: { ...p, status: p.status === 'CLOSED' ? 'INPROGRESS' : 'CLOSED', estimate: String(p.estimate / 100) } }); load(); }
  async function delTime(t: any) { if (await confirm('Delete this time entry?')) { try { await rpc('projects.deleteTime', { id: t.id }); load(); } catch (e) { toastError(e); } } }
</script>

{#if p}
<PageHeader title={p.name} crumbs={[{ label: 'Projects', href: '/projects' }]}>
  {#snippet sub()}{p.contact_name ?? ''}{p.deadline ? ` · deadline ${date(p.deadline)}` : ''} · {p.status === 'CLOSED' ? 'Closed' : 'In progress'}{/snippet}
  {#if can('projects:write')}<button class="btn" onclick={close}>{p.status === 'CLOSED' ? 'Reopen' : 'Close project'}</button>{/if}
  {#if can('sales:write')}<button class="btn btn-primary" disabled={!p.summary.to_invoice} onclick={invoice}><Icon name="doc" />Invoice {money(p.summary.to_invoice)}</button>{/if}
</PageHeader>
<div class="mb-4 grid gap-3 sm:grid-cols-5">
  <div class="card p-4"><div class="text-[12px] text-muted">Estimate</div><div class="num text-lg font-semibold">{money(p.summary.estimate)}</div></div>
  <div class="card p-4"><div class="text-[12px] text-muted">Time logged</div><div class="text-lg font-semibold">{hm(p.summary.minutes)} h</div></div>
  <div class="card p-4"><div class="text-[12px] text-muted">Invoiced</div><div class="num text-lg font-semibold">{money(p.summary.invoiced)}</div></div>
  <div class="card p-4"><div class="text-[12px] text-muted">Costs</div><div class="num text-lg font-semibold">{money(p.summary.costs)}</div></div>
  <div class="card p-4"><div class="text-[12px] text-muted">Profit</div><div class="num text-lg font-semibold {p.summary.profit < 0 ? 'text-bad' : 'text-ok'}">{money(p.summary.profit)}</div></div>
</div>
{#if can('projects:write')}
  <div class="card mb-4 flex flex-wrap items-end gap-3 p-4">
    <div class="w-52"><span class="label">Task</span><select class="select" bind:value={te.task_id}><option value={null}>Choose task</option>{#each p.tasks.filter((t: any) => t.charge_type !== 'FIXED') as t}<option value={t.id}>{t.name}</option>{/each}</select></div>
    <div><label class="label" for="td">Date</label><input id="td" type="date" class="input" bind:value={te.date} /></div>
    <div class="w-28"><label class="label" for="th">Hours (h or h:mm)</label><input id="th" class="input num" bind:value={te.hours} placeholder="1:30" /></div>
    <div class="min-w-[200px] flex-1"><label class="label" for="tds">Description</label><input id="tds" class="input" bind:value={te.description} /></div>
    <button class="btn btn-primary" disabled={!te.task_id || !te.hours} onclick={addTime}>Add time</button>
    {#if p.timer && p.timer.project_id === p.id}<button class="btn" onclick={stopTimer}><Icon name="pause" />Stop timer</button>{:else}<button class="btn" disabled={!te.task_id || !!p.timer} onclick={startTimer}><Icon name="play" />Start timer</button>{/if}
  </div>
{/if}
<Tabs tabs={[{ key: 'time', label: 'Time', count: p.time.length }, { key: 'tasks', label: 'Tasks', count: p.tasks.length }, { key: 'costs', label: 'Expenses', count: p.costs.length }, { key: 'invoices', label: 'Invoices', count: p.invoices.length }]} bind:value={tab} />
<div class="card overflow-x-auto">
  {#if tab === 'time'}
    <table class="tbl"><thead><tr><th>Date</th><th>Task</th><th>Description</th><th>Who</th><th class="n">Hours</th><th>Invoiced</th><th></th></tr></thead>
      <tbody>{#each p.time as t}<tr><td>{date(t.date)}</td><td>{t.task}</td><td class="text-muted">{t.description}</td><td>{t.user ?? ''}</td><td class="n">{t.running_since ? 'running…' : hm(t.minutes)}</td><td>{#if t.invoiced_document_id}<a class="link" href="/doc/{t.invoiced_document_id}">Yes</a>{/if}</td>
        <td class="text-right">{#if !t.invoiced_document_id && can('projects:write')}<button class="text-faint hover:text-bad" onclick={() => delTime(t)} aria-label="Delete"><Icon name="trash" size={14} /></button>{/if}</td></tr>
      {:else}<tr><td colspan="7" class="py-8 text-center text-muted">No time logged yet.</td></tr>{/each}</tbody></table>
  {:else if tab === 'tasks'}
    <div class="flex justify-end p-3">{#if can('projects:write')}<button class="btn btn-sm" onclick={() => { tf = { name: '', charge_type: 'TIME', rate: '' }; taskOpen = true; }}><Icon name="plus" size={14} />Add task</button>{/if}</div>
    <table class="tbl"><thead><tr><th>Task</th><th>Charge</th><th class="n">Rate / price</th><th class="n">Time</th><th>Status</th></tr></thead>
      <tbody>{#each p.tasks as t}<tr class="clickable" onclick={() => { tf = { ...t, rate: unitPrice(t.rate_e2) }; taskOpen = true; }}><td>{t.name}</td><td class="text-muted">{t.charge_type === 'TIME' ? 'Hourly' : t.charge_type === 'FIXED' ? 'Fixed price' : 'Non-chargeable'}</td><td class="n">{unitPrice(t.rate_e2)}</td><td class="n">{hm(t.minutes)}</td><td><Status status={t.status} /></td></tr>{/each}</tbody></table>
  {:else if tab === 'costs'}
    <div class="flex justify-end p-3">{#if can('projects:write')}<button class="btn btn-sm" onclick={() => { cf = { description: '', amount: '', date: today(), chargeable: true, markup: '' }; costOpen = true; }}><Icon name="plus" size={14} />Add expense</button>{/if}</div>
    <table class="tbl"><thead><tr><th>Date</th><th>Description</th><th class="n">Cost</th><th>Chargeable</th><th class="n">Markup</th><th>Invoiced</th></tr></thead>
      <tbody>{#each p.costs as c}<tr><td>{date(c.date)}</td><td>{c.description}</td><td class="n">{money(c.amount)}</td><td>{c.chargeable ? 'Yes' : 'No'}</td><td class="n">{c.markup_bp ? c.markup_bp / 100 + '%' : ''}</td><td>{#if c.invoiced_document_id}<a class="link" href="/doc/{c.invoiced_document_id}">Yes</a>{/if}</td></tr>{/each}</tbody></table>
  {:else}
    <table class="tbl"><thead><tr><th>Number</th><th>Date</th><th class="n">Total</th><th>Status</th></tr></thead>
      <tbody>{#each p.invoices as i}<tr class="clickable" onclick={() => navigate(`/sales/invoices/${i.id}`)}><td>{i.number}</td><td>{date(i.date)}</td><td class="n">{money(i.total)}</td><td><Status status={i.status} /></td></tr>{/each}</tbody></table>
  {/if}
</div>
{/if}
<Modal bind:open={taskOpen} title="Task">
  <div class="space-y-3"><div><label class="label" for="tn">Name</label><input id="tn" class="input" bind:value={tf.name} /></div>
    <div><span class="label">How is it charged?</span><select class="select" bind:value={tf.charge_type}><option value="TIME">Hourly rate</option><option value="FIXED">Fixed price</option><option value="NONCHARGEABLE">Non-chargeable</option></select></div>
    {#if tf.charge_type !== 'NONCHARGEABLE'}<div><label class="label" for="tr">{tf.charge_type === 'TIME' ? 'Hourly rate' : 'Price'}</label><input id="tr" class="input num" bind:value={tf.rate} /></div>{/if}</div>
  {#snippet footer()}<button class="btn" onclick={() => (taskOpen = false)}>Cancel</button><button class="btn btn-primary" onclick={saveTask}>Save</button>{/snippet}
</Modal>
<Modal bind:open={costOpen} title="Project expense">
  <div class="space-y-3"><div><label class="label" for="cd">Description</label><input id="cd" class="input" bind:value={cf.description} /></div>
    <div class="grid grid-cols-3 gap-3"><div><label class="label" for="ca">Cost</label><input id="ca" class="input num" bind:value={cf.amount} /></div><div><label class="label" for="cdt">Date</label><input id="cdt" type="date" class="input" bind:value={cf.date} /></div><div><label class="label" for="cm">Markup %</label><input id="cm" class="input num" bind:value={cf.markup} /></div></div>
    <label class="flex items-center gap-2"><input type="checkbox" bind:checked={cf.chargeable} class="accent-[var(--accent)]" />Charge this to the client</label></div>
  {#snippet footer()}<button class="btn" onclick={() => (costOpen = false)}>Cancel</button><button class="btn btn-primary" onclick={saveCost}>Add</button>{/snippet}
</Modal>
