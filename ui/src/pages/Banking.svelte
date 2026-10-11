<script lang="ts">
  import { rpc } from '../lib/api';
  import { money, date } from '../lib/format';
  import { can, toast, toastError, loadLookups, setting } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Modal from '../components/Modal.svelte';
  import Combo from '../components/Combo.svelte';
  import Icon from '../components/Icon.svelte';
  import { currencyOptions } from '../components/Pickers';

  let banks = $state<any[]>([]);
  const load = () => rpc('accounts.bank').then((b) => (banks = b)).catch(toastError);
  load();
  let add = $state({ open: false, name: '', code: '', bank_type: 'BANK', bank_account_number: '', currency: setting('base_currency', 'AUD') });
  async function create() {
    try {
      await rpc('accounts.create', { code: add.code, name: add.name, type: 'BANK', bank_type: add.bank_type, bank_account_number: add.bank_account_number, currency: add.currency });
      add.open = false; toast('Bank account added'); await loadLookups(); load();
    } catch (e) { toastError(e); }
  }
  const TYPE: Record<string, string> = { BANK: 'Bank account', CREDITCARD: 'Credit card', PAYPAL: 'Payment service', CASH: 'Cash' };
</script>

<PageHeader title="Bank accounts">
  <a class="btn" href="/banking/rules">Bank rules</a>
  <a class="btn" href="/banking/transfer"><Icon name="swap" />Transfer money</a>
  {#if can('accounting:write')}<button class="btn btn-primary" onclick={() => (add.open = true)}><Icon name="plus" />Add bank account</button>{/if}
</PageHeader>

<div class="space-y-3">
  {#each banks as b}
    <div class="card flex flex-wrap items-center gap-4 p-5 {b.status === 'ARCHIVED' ? 'opacity-60' : ''}">
      <div class="grid h-10 w-10 place-items-center rounded-lg bg-accent-soft text-accent"><Icon name={b.bank_type === 'CREDITCARD' ? 'dollar' : 'bank'} size={20} /></div>
      <div class="min-w-[200px] flex-1">
        <a href="/banking/{b.id}" class="text-[16px] font-semibold hover:text-accent">{b.name}</a>
        <div class="text-[13px] text-muted">{TYPE[b.bank_type] ?? 'Bank'} · {b.bank_account_number ?? 'No number'} · {b.currency}{b.last_statement_date ? ` · statement to ${date(b.last_statement_date)}` : ''}</div>
      </div>
      <div class="text-right"><div class="text-[12px] text-muted">Statement balance</div><div class="num font-semibold">{money(b.statement_balance)}</div></div>
      <div class="text-right"><div class="text-[12px] text-muted">Balance in LedgerCraft</div><div class="num font-semibold">{money(b.ledger_balance_fx)}</div></div>
      <div class="flex gap-2">
        {#if b.unreconciled_count}<a class="btn btn-primary" href="/banking/{b.id}">Reconcile {b.unreconciled_count}</a>{:else}<a class="btn" href="/banking/{b.id}">Open</a>{/if}
        <a class="btn" href="/banking/{b.id}/import" title="Import statement"><Icon name="upload" /></a>
      </div>
    </div>
  {/each}
</div>

<Modal bind:open={add.open} title="Add bank account">
  <div class="space-y-3">
    <div><label class="label" for="bt">Account type</label><select id="bt" class="select" bind:value={add.bank_type}><option value="BANK">Everyday / savings account</option><option value="CREDITCARD">Credit card</option><option value="PAYPAL">PayPal / Stripe / payment service</option><option value="CASH">Cash / petty cash</option></select></div>
    <div><label class="label" for="bn">Account name</label><input id="bn" class="input" bind:value={add.name} placeholder="e.g. Business Everyday" /></div>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="bc">Code</label><input id="bc" class="input" bind:value={add.code} placeholder="e.g. 092" /></div>
      <div><span class="label">Currency</span><Combo bind:value={add.currency} options={currencyOptions()} clearable={false} /></div></div>
    <div><label class="label" for="bnum">Account number</label><input id="bnum" class="input" bind:value={add.bank_account_number} /></div>
    <p class="text-[12px] text-muted">Import statements as CSV, OFX/QFX, QIF, CAMT.053 or MT940. Live bank feeds can be added through the bank-feed plugin interface (see PLUGINS.md).</p>
  </div>
  {#snippet footer()}<button class="btn" onclick={() => (add.open = false)}>Cancel</button><button class="btn btn-primary" disabled={!add.name || !add.code} onclick={create}>Add account</button>{/snippet}
</Modal>
