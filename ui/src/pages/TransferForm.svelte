<script lang="ts">
  import { rpc } from '../lib/api';
  import { today } from '../lib/format';
  import { navigate, route } from '../lib/router.svelte';
  import { lookups, toast, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Combo from '../components/Combo.svelte';
  import { bankAccounts } from '../components/Pickers';
  let f = $state({ from_account_id: route.query.get('from') ? Number(route.query.get('from')) : null as number | null, to_account_id: null as number | null, date: today(), amount: '', to_amount: '', reference: '' });
  const cur = (id: number | null) => lookups.accounts.find((a) => a.id === id)?.currency;
  const fx = $derived(f.from_account_id && f.to_account_id && cur(f.from_account_id) !== cur(f.to_account_id));
  async function save(e: Event) {
    e.preventDefault();
    try { await rpc('transfers.save', f); toast('Transfer recorded'); navigate(`/banking/${f.from_account_id}?tab=transactions`); } catch (x) { toastError(x); }
  }
</script>

<PageHeader title="Transfer money" crumbs={[{ label: 'Bank accounts', href: '/banking' }]} />
<form class="card max-w-2xl space-y-4 p-5" onsubmit={save}>
  <div class="grid grid-cols-2 gap-4">
    <div><label class="label" for="cb-transfer-from-1">From</label><Combo id="cb-transfer-from-1" bind:value={f.from_account_id} options={bankAccounts()} /></div>
    <div><label class="label" for="cb-transfer-to-2">To</label><Combo id="cb-transfer-to-2" bind:value={f.to_account_id} options={bankAccounts().filter((o) => o.value !== f.from_account_id)} /></div>
    <div><label class="label" for="d">Date</label><input id="d" type="date" class="input" bind:value={f.date} /></div>
    <div><label class="label" for="a">Amount {cur(f.from_account_id) ?? ''}</label><input id="a" class="input num" bind:value={f.amount} required /></div>
    {#if fx}<div><label class="label" for="ta">Amount received {cur(f.to_account_id)}</label><input id="ta" class="input num" bind:value={f.to_amount} required /></div>{/if}
    <div class="col-span-2"><label class="label" for="r">Reference</label><input id="r" class="input" bind:value={f.reference} /></div>
  </div>
  <div class="flex justify-end gap-2"><a class="btn" href="/banking">Cancel</a><button class="btn btn-primary" disabled={!f.from_account_id || !f.to_account_id}>Save transfer</button></div>
</form>
