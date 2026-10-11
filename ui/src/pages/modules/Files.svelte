<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, dateTime } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { can, toast, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Icon from '../../components/Icon.svelte';
  let rows = $state<any[]>([]);
  let drag = $state(false);
  let busy = $state(false);
  const load = () => rpc('inbox.list').then((r) => (rows = r)).catch(toastError);
  load();
  async function upload(files: FileList | File[]) {
    busy = true;
    for (const f of Array.from(files)) {
      const data = await new Promise<string>((ok) => { const fr = new FileReader(); fr.onload = () => ok(String(fr.result)); fr.readAsDataURL(f); });
      try { await rpc('inbox.upload', { filename: f.name, mime: f.type || 'application/octet-stream', data }); } catch (e) { toastError(e); }
    }
    busy = false; toast('Uploaded'); load();
  }
  let edits = $state<Record<number, any>>({});
  function ov(r: any) { return (edits[r.id] ??= { supplier: r.extracted?.fields?.supplier ?? '', date: r.extracted?.fields?.date ?? '', number: r.extracted?.fields?.number ?? '', total: r.extracted?.fields?.total != null ? (r.extracted.fields.total / 100).toFixed(2) : '' }); }
  async function toBill(r: any) { try { const x = await rpc('inbox.toBill', { id: r.id, overrides: ov(r) }); toast('Draft bill created — review and approve it'); navigate(`/purchases/bills/${x.id}`); } catch (e) { toastError(e); } }
  async function archive(r: any) { await rpc('inbox.archive', { id: r.id }); load(); }
</script>

<PageHeader title="Files inbox" crumbs={[{ label: 'Business', href: '/purchases' }]} />
{#if can('purchases:write')}
  <label class="mb-4 flex cursor-pointer flex-col items-center justify-center rounded-xl border-2 border-dashed px-6 py-10 text-center transition {drag ? 'border-accent bg-accent-soft' : 'border-line2 hover:border-accent'}"
    ondragover={(e) => { e.preventDefault(); drag = true; }} ondragleave={() => (drag = false)} ondrop={(e) => { e.preventDefault(); drag = false; if (e.dataTransfer?.files) upload(e.dataTransfer.files); }}>
    <Icon name="upload" size={26} class="text-accent" />
    <div class="mt-2 font-semibold">{busy ? 'Reading files…' : 'Drop bills and receipts here, or click to browse'}</div>
    <div class="max-w-xl text-[13px] text-muted">PDF invoices are read automatically (supplier, date, number, total). Photos need an OCR engine: set <code>ocr_command</code> to a Tesseract executable under Settings → Connected apps (optional plugin).</div>
    <input type="file" multiple class="hidden" accept="application/pdf,image/*,text/plain" onchange={(e) => { const f = (e.target as HTMLInputElement).files; if (f) upload(f); }} />
  </label>
{/if}
<div class="space-y-3">
  {#each rows as r}
    {@const o = ov(r)}
    <div class="card grid gap-4 p-4 lg:grid-cols-[1fr_2fr_auto]">
      <div><a class="link font-medium" href="/download/attachment?id={r.attachment_id}" target="_blank">{r.filename}</a><div class="text-[12px] text-muted">{dateTime(r.created_at)} · {Math.round(r.size / 1024)} KB · {r.extracted?.method === 'none' ? 'no text found' : 'read by ' + r.extracted?.method}</div>
        {#if r.status === 'PROCESSED'}<a class="chip mt-1 bg-ok-soft text-ok" href="/doc/{r.document_id}">Bill created</a>{/if}</div>
      <div class="grid grid-cols-2 gap-2 md:grid-cols-4">
        <input class="input" placeholder="Supplier" bind:value={o.supplier} /><input class="input" type="date" bind:value={o.date} /><input class="input" placeholder="Bill number" bind:value={o.number} /><input class="input num" placeholder="Total" bind:value={o.total} />
      </div>
      <div class="flex items-start gap-2">{#if r.status !== 'PROCESSED'}<button class="btn btn-primary" onclick={() => toBill(r)}>Create bill</button>{/if}<button class="btn" onclick={() => archive(r)}>Archive</button></div>
    </div>
  {:else}<div class="card p-10 text-center text-muted">Your inbox is empty.</div>{/each}
</div>
