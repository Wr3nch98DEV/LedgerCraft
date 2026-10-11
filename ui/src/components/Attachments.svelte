<script lang="ts">
  import { rpc } from '../lib/api';
  import { confirm, toast, toastError } from '../lib/stores.svelte';
  import Icon from './Icon.svelte';
  let { entity, id, readonly = false }: { entity: string; id: number; readonly?: boolean } = $props();
  let files = $state<any[]>([]);
  let drag = $state(false);
  const load = () => rpc('attachments.list', { entity, id }).then((f) => (files = f)).catch(() => {});
  load();
  async function upload(list: FileList | File[]) {
    for (const f of Array.from(list)) {
      if (f.size > 15 * 1024 * 1024) { toastError(`${f.name} is larger than 15 MB`); continue; }
      const data = await new Promise<string>((ok) => { const fr = new FileReader(); fr.onload = () => ok(String(fr.result)); fr.readAsDataURL(f); });
      try { await rpc('attachments.add', { entity, entity_id: id, filename: f.name, mime: f.type || 'application/octet-stream', data }); } catch (e) { toastError(e); }
    }
    toast('File attached'); load();
  }
  async function del(f: any) { if (await confirm(`Remove ${f.filename}?`, '', 'Remove', true)) { await rpc('attachments.delete', { id: f.id }); load(); } }
  const kb = (n: number) => (n > 1048576 ? (n / 1048576).toFixed(1) + ' MB' : Math.max(1, Math.round(n / 1024)) + ' KB');
</script>

<section class="card mt-4 p-4" ondragover={(e) => { if (!readonly) { e.preventDefault(); drag = true; } }} ondragleave={() => (drag = false)}
  ondrop={(e) => { e.preventDefault(); drag = false; if (!readonly && e.dataTransfer?.files) upload(e.dataTransfer.files); }} role="region" aria-label="Files">
  <div class="mb-2 flex items-center justify-between"><span class="font-semibold"><Icon name="paperclip" size={15} class="mr-1 inline" />Files <span class="text-muted">({files.length})</span></span>
    {#if !readonly}<label class="btn btn-sm cursor-pointer"><Icon name="upload" size={14} />Attach<input type="file" multiple class="hidden" onchange={(e) => { const f = (e.target as HTMLInputElement).files; if (f) upload(f); }} /></label>{/if}</div>
  {#if files.length}
    <div class="flex flex-wrap gap-2">{#each files as f}
      <span class="flex items-center gap-1.5 rounded-lg border border-line px-2.5 py-1 text-[13px]"><a class="link" href="/download/attachment?id={f.id}" target="_blank">{f.filename}</a><span class="text-faint">{kb(f.size)}</span>
        {#if !readonly}<button class="text-faint hover:text-bad" onclick={() => del(f)} aria-label="Remove file"><Icon name="x" size={12} /></button>{/if}</span>{/each}</div>
  {:else}<div class="rounded-lg border border-dashed px-3 py-3 text-center text-[13px] text-muted {drag ? 'border-accent bg-accent-soft' : 'border-line2'}">Drop receipts, contracts or supporting documents here</div>{/if}
</section>
