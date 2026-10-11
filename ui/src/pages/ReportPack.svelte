<script lang="ts">
  import { rpc } from '../lib/api';
  import { today, date } from '../lib/format';
  import { navigate } from '../lib/router.svelte';
  import { confirm, toast, toastError, setting } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import ReportTable from '../components/ReportTable.svelte';
  import Icon from '../components/Icon.svelte';
  let { id = 'new' }: { id?: string } = $props();
  const isNew = id === 'new';
  let cat = $state<any[]>([]);
  let layouts = $state<any[]>([]);
  let name = $state('Monthly management pack');
  const fyEnd = Number(setting('fy_end_month', '6'));
  const fyStart = (() => { const d = new Date(); const sm = (fyEnd % 12) + 1; const y = d.getMonth() + 1 >= sm ? d.getFullYear() : d.getFullYear() - 1; return `${y}-${String(sm).padStart(2, '0')}-01`; })();
  let def = $state<any>({ cover_title: 'Management Report', cover_subtitle: '', commentary: '', from: fyStart, to: today(),
    items: [{ report: 'profit-and-loss', title: 'Profit and Loss' }, { report: 'balance-sheet', title: 'Balance Sheet' }, { report: 'aged-receivables', title: 'Aged Receivables' }] });
  let pack = $state<any>(null);
  let editing = $state(true);
  rpc('reports.catalogue').then((c) => (cat = c));
  rpc('layouts.list').then((ls) => { layouts = ls.filter((l: any) => l.kind === 'CUSTOM'); if (!isNew) { const l = ls.find((x: any) => x.id === Number(id)); if (l) { name = l.name; def = { ...def, ...l.definition }; editing = false; build(); } } });
  function withParams() {
    return { ...def, items: def.items.map((it: any) => it.layout_id ? it : { ...it, params: { from: def.from, to: def.to, date: def.to } }) };
  }
  async function build() { try { pack = await rpc('layouts.preview', { kind: 'PACK', name, definition: withParams() }); } catch (e) { toastError(e); } }
  async function save() { try { const r = await rpc('layouts.save', { id: isNew ? null : Number(id), name, kind: 'PACK', definition: def }); toast('Pack saved'); if (isNew) navigate(`/reports/pack/${r.id}`, { replace: true }); } catch (e) { toastError(e); } }
  async function del() { if (await confirm('Delete this report pack?', '', 'Delete', true)) { await rpc('layouts.delete', { id: Number(id) }); navigate('/reports'); } }
  function move(i: number, d: number) { const j = i + d; if (j < 0 || j >= def.items.length) return; const t = def.items[i]; def.items[i] = def.items[j]; def.items[j] = t; }
</script>

<PageHeader title={name} crumbs={[{ label: 'Reports', href: '/reports' }]}>
  <button class="btn" onclick={() => (editing = !editing)}>{editing ? 'Hide editor' : 'Edit pack'}</button>
  {#if pack}<button class="btn" onclick={() => window.print()}><Icon name="print" />Print / PDF</button>{/if}
  {#if !isNew}<button class="btn btn-danger" onclick={del}>Delete</button>{/if}
  <button class="btn btn-primary" onclick={save}>Save</button>
</PageHeader>
{#if editing}
  <section class="no-print card mb-4 grid gap-4 p-4 lg:grid-cols-2">
    <div class="space-y-2">
      <div><label class="label" for="pn">Pack name</label><input id="pn" class="input" bind:value={name} /></div>
      <div><label class="label" for="ct">Cover title</label><input id="ct" class="input" bind:value={def.cover_title} /></div>
      <div><label class="label" for="cs">Cover subtitle</label><input id="cs" class="input" bind:value={def.cover_subtitle} placeholder="e.g. Board meeting — October" /></div>
      <div class="grid grid-cols-2 gap-2"><div><label class="label" for="pf">Period from</label><input id="pf" type="date" class="input" bind:value={def.from} /></div><div><label class="label" for="pt">to / as at</label><input id="pt" type="date" class="input" bind:value={def.to} /></div></div>
      <div><label class="label" for="cm">Commentary</label><textarea id="cm" rows="5" class="textarea" bind:value={def.commentary} placeholder="Highlights, risks and actions for the period…"></textarea></div>
    </div>
    <div>
      <div class="label">Reports in this pack</div>
      {#each def.items as it, i}
        <div class="mb-1.5 flex items-center gap-2 rounded-lg border border-line px-3 py-2 text-[13px]"><span class="flex-1">{i + 1}. {it.title ?? cat.find((c) => c.key === it.report)?.title ?? layouts.find((l) => l.id === it.layout_id)?.name}</span>
          <button class="btn btn-ghost btn-sm" onclick={() => move(i, -1)} aria-label="Up">↑</button><button class="btn btn-ghost btn-sm" onclick={() => move(i, 1)} aria-label="Down">↓</button><button class="btn btn-ghost btn-sm" onclick={() => def.items.splice(i, 1)} aria-label="Remove"><Icon name="x" size={13} /></button></div>
      {/each}
      <select class="select mt-2" onchange={(e) => { const v = (e.target as HTMLSelectElement).value; if (v.startsWith('L')) { const l = layouts.find((x) => x.id === Number(v.slice(1))); def.items.push({ layout_id: l.id, title: l.name }); } else if (v) { def.items.push({ report: v, title: cat.find((c) => c.key === v)?.title }); } (e.target as HTMLSelectElement).value = ''; }}>
        <option value="">+ Add a report…</option>
        {#each cat.filter((c) => !['account-transactions', 'bank-reconciliation'].includes(c.key)) as c}<option value={c.key}>{c.title}</option>{/each}
        {#each layouts as l}<option value={'L' + l.id}>Custom: {l.name}</option>{/each}
      </select>
      <button class="btn btn-primary mt-3 w-full justify-center" onclick={build}>Build pack</button>
    </div>
  </section>
{/if}
{#if pack}
  <article class="card mb-6 flex min-h-[60vh] flex-col items-center justify-center p-10 text-center print:break-after-page">
    <div class="text-[13px] uppercase tracking-[0.2em] text-muted">{pack.org}</div>
    <h1 class="mt-3 text-4xl font-semibold">{pack.title}</h1>
    {#if pack.subtitle}<div class="mt-2 text-lg text-muted">{pack.subtitle}</div>{/if}
    <div class="mt-6 text-[13px] text-muted">Prepared {date(pack.prepared)}</div>
    {#if pack.commentary}<div class="mt-10 max-w-2xl whitespace-pre-wrap text-left text-[15px] leading-relaxed">{pack.commentary}</div>{/if}
  </article>
  {#each pack.reports as r}
    <article class="card mb-6 p-6 print:break-after-page">
      <div class="mb-4 text-center"><div class="text-xl font-semibold">{r.title}</div><div class="text-[13px] text-muted">{r.org} · {r.subtitle}</div></div>
      <ReportTable report={r} drill={false} />
    </article>
  {/each}
{/if}
