<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, monthEnd, addMonths, today, dateTime } from '../../lib/format';
  import { can, toast, toastError } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Icon from '../../components/Icon.svelte';
  let period = $state(monthEnd(addMonths(today(), -1)));
  let rows = $state<any[]>([]);
  $effect(() => { rpc('workpapers.list', { period_end: period }).then((r) => (rows = r)).catch(toastError); });
  async function save(r: any, reviewed = !!r.reviewed) { try { await rpc('workpapers.save', { account_id: r.account_id, period_end: period, note: r.note ?? '', reviewed }); r.reviewed = reviewed ? 1 : 0; toast('Saved'); } catch (e) { toastError(e); } }
  async function attach(r: any, f: File) {
    const data = await new Promise<string>((ok) => { const fr = new FileReader(); fr.onload = () => ok(String(fr.result)); fr.readAsDataURL(f); });
    await rpc('attachments.add', { entity: 'workpaper', entity_id: r.account_id, filename: `${period} ${f.name}`, mime: f.type, data }); r.files += 1; toast('File attached');
  }
  const done = $derived(rows.filter((r) => r.reviewed).length);
</script>

<PageHeader title="Workpapers" crumbs={[{ label: 'Accounting', href: '/reports' }]}>
  <div><label class="label" for="pe">Period ending</label><input id="pe" type="date" class="input" bind:value={period} /></div>
</PageHeader>
<p class="mb-4 text-muted">Review each balance sheet account for the period: add notes and supporting files (bank statements, loan statements, schedules) and mark them reviewed. <b class="text-text">{done} of {rows.length}</b> reviewed.</p>
<div class="card overflow-x-auto">
  <table class="tbl"><thead><tr><th>Account</th><th class="n">Balance</th><th>Notes</th><th>Files</th><th>Reviewed</th></tr></thead>
    <tbody>{#each rows as r}<tr>
      <td class="whitespace-nowrap"><a class="link" href="/reports/account-transactions?account_id={r.account_id}&to={period}">{r.code} {r.name}</a></td>
      <td class="n">{money(r.balance)}</td>
      <td class="w-[40%] !p-1"><input class="grid-in" bind:value={r.note} onblur={() => save(r)} placeholder="Add a note" disabled={!can('accounting:write')} /></td>
      <td><label class="btn btn-ghost btn-sm cursor-pointer"><Icon name="paperclip" size={14} />{r.files || ''}<input type="file" class="hidden" onchange={(e) => { const f = (e.target as HTMLInputElement).files?.[0]; if (f) attach(r, f); }} /></label></td>
      <td><label class="flex items-center gap-2 text-[13px]"><input type="checkbox" checked={!!r.reviewed} onchange={(e) => save(r, (e.target as HTMLInputElement).checked)} disabled={!can('accounting:write')} class="accent-[var(--accent)]" />{#if r.reviewed}<span class="text-muted">{r.reviewed_by ?? ''}</span>{/if}</label></td>
    </tr>{/each}</tbody></table>
</div>
