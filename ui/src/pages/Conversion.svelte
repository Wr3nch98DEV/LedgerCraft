<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, parseMoney, moneyPlain } from '../lib/format';
  import { lookups, toast, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Combo from '../components/Combo.svelte';
  import Icon from '../components/Icon.svelte';
  import { accountOptions } from '../components/Pickers';
  let date = $state('');
  let lines = $state<any[]>([]);
  rpc('conversion.get').then((c) => {
    date = c.date ?? '';
    lines = c.lines.filter((l: any) => lookups.accounts.find((a) => a.id === l.account_id)?.system_account !== 'HISTORICAL').map((l: any) => ({ account_id: l.account_id, debit: l.amount > 0 ? moneyPlain(l.amount) : '', credit: l.amount < 0 ? moneyPlain(-l.amount) : '' }));
    if (!lines.length) lines = [{ account_id: null, debit: '', credit: '' }];
  }).catch(toastError);
  const tot = $derived(lines.reduce((s, l) => ({ d: s.d + (parseMoney(l.debit) ?? 0), c: s.c + (parseMoney(l.credit) ?? 0) }), { d: 0, c: 0 }));
  async function save() { try { await rpc('conversion.save', { date, lines }); toast('Conversion balances saved'); } catch (e) { toastError(e); } }
</script>

<PageHeader title="Conversion balances" crumbs={[{ label: 'Settings', href: '/settings' }]} />
<p class="mb-4 max-w-3xl text-muted">Enter the balances of your accounts as at the day before you started using LedgerCraft (from your previous system's trial balance). Enter outstanding customer invoices and supplier bills as individual invoices/bills dated before the conversion date so they can be paid normally. Any difference is posted to Historical Adjustment.</p>
<div class="card p-5">
  <div class="mb-4 w-56"><label class="label" for="cd">Balances as at</label><input id="cd" type="date" class="input" bind:value={date} /></div>
  <table class="tbl"><thead><tr><th>Account</th><th class="!text-right w-40">Debit</th><th class="!text-right w-40">Credit</th><th class="w-10"></th></tr></thead>
    <tbody>{#each lines as l, i}<tr><td class="!p-1"><Combo compact bind:value={l.account_id} options={accountOptions((a) => a.class !== 'REVENUE' && a.class !== 'EXPENSE')} /></td>
      <td class="!p-1"><input class="grid-in num text-right" bind:value={l.debit} /></td><td class="!p-1"><input class="grid-in num text-right" bind:value={l.credit} /></td>
      <td><button class="text-faint hover:text-bad" onclick={() => lines.splice(i, 1)} aria-label="Remove"><Icon name="trash" size={14} /></button></td></tr>{/each}
      <tr class="font-semibold"><td>Total</td><td class="n">{money(tot.d)}</td><td class="n">{money(tot.c)}</td><td></td></tr>
      {#if tot.d !== tot.c}<tr><td colspan="4" class="text-right text-muted">Historical adjustment of {money(Math.abs(tot.d - tot.c))} will balance the entry</td></tr>{/if}</tbody></table>
  <div class="mt-3 flex justify-between"><button class="btn btn-sm" onclick={() => lines.push({ account_id: null, debit: '', credit: '' })}><Icon name="plus" size={14} />Add line</button><button class="btn btn-primary" disabled={!date} onclick={save}>Save</button></div>
</div>
