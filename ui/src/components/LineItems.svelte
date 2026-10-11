<script lang="ts">
  // Editable line-item grid used by invoices, bills, credit notes, quotes, POs, spend/receive money.
  import Combo from './Combo.svelte';
  import Icon from './Icon.svelte';
  import { lookups } from '../lib/stores.svelte';
  import { accountOptions, taxOptions, trackingCategories, trackingOptions, itemOptions } from './Pickers';
  import { money } from '../lib/format';
  import { calcLine } from '../lib/calc';

  let { lines = $bindable([]), lat = 'EXCLUSIVE', sales = true, showItems = true, showQty = true, readonly = false, accountFilter = undefined }:
    { lines: any[]; lat?: string; sales?: boolean; showItems?: boolean; showQty?: boolean; readonly?: boolean; accountFilter?: (a: any) => boolean } = $props();

  const cats = $derived(trackingCategories());
  const accts = $derived(accountOptions(accountFilter ?? ((a) => a.type !== 'BANK' || !sales)));
  const taxes = $derived(taxOptions());
  const items = $derived(itemOptions(sales));
  const hasDiscount = $derived(lines.some((l) => l.discount && l.discount !== '0'));
  let showDisc = $state(false);

  export function blank() { return { item_id: null, description: '', quantity: '1', unit_price: '', discount: '', account_id: null, tax_rate_id: null, tracking1: null, tracking2: null }; }
  $effect(() => { if (!readonly && (lines.length === 0 || !isBlank(lines[lines.length - 1]))) lines.push(blank()); });
  function isBlank(l: any) { return !l.item_id && !l.description && !l.unit_price && !l.account_id && !l.amount; }

  const calc = (l: any) => calcLine(l, lat);

  function pickItem(i: number, id: number | null) {
    const it = lookups.items.find((x) => x.id === id);
    if (!it) return;
    const l = lines[i];
    l.item_id = id;
    l.description = (sales ? it.sales_description : it.purchase_description) || it.name;
    const unit = sales ? it.sale_unit_e2 : it.purchase_unit_e2;
    if (unit !== null && unit !== undefined) l.unit_price = (unit / 10000).toFixed(unit % 100 === 0 ? 2 : 4);
    l.account_id = sales ? it.sale_account_id ?? l.account_id : (it.is_tracked ? it.inventory_account_id : it.purchase_account_id) ?? l.account_id;
    l.tax_rate_id = (sales ? it.sale_tax_rate_id : it.purchase_tax_rate_id) ?? l.tax_rate_id;
  }
  function pickAccount(i: number, id: number | null) {
    const a = lookups.accounts.find((x) => x.id === id);
    if (a && !lines[i].tax_rate_id) lines[i].tax_rate_id = a.tax_rate_id;
  }
  function remove(i: number) { lines.splice(i, 1); }
  function dup(i: number) { lines.splice(i + 1, 0, { ...lines[i] }); }
  function move(i: number, d: number) { const j = i + d; if (j < 0 || j >= lines.length) return; const t = lines[i]; lines[i] = lines[j]; lines[j] = t; }
</script>

<div class="overflow-x-auto rounded-xl border border-line">
  <table class="tbl min-w-[900px] text-[14px]">
    <thead>
      <tr>
        {#if showItems}<th class="w-[130px]">Item</th>{/if}
        <th class="min-w-[200px]">Description</th>
        {#if showQty}<th class="w-[80px] !text-right">Qty</th><th class="w-[110px] !text-right">Price</th>{/if}
        {#if hasDiscount || showDisc}<th class="w-[70px] !text-right">Disc %</th>{/if}
        <th class="w-[200px]">Account</th>
        {#if lat !== 'NOTAX'}<th class="w-[170px]">Tax rate</th>{/if}
        {#each cats as c, k}<th class="w-[130px]">{c.name}</th>{/each}
        <th class="w-[110px] !text-right">Amount</th>
        {#if !readonly}<th class="w-[64px]"></th>{/if}
      </tr>
    </thead>
    <tbody>
      {#each lines as l, i}
        {@const r = calc(l)}
        <tr class="group">
          {#if showItems}<td class="!p-1">{#if readonly}<span class="px-2">{lookups.items.find((x) => x.id === l.item_id)?.code ?? ''}</span>{:else}<Combo compact value={l.item_id} options={items} onchange={(v) => pickItem(i, v)} placeholder="" />{/if}</td>{/if}
          <td class="!p-1">{#if readonly}<div class="whitespace-pre-wrap px-2">{l.description}</div>{:else}<textarea rows="1" class="grid-in resize-none" bind:value={l.description} oninput={(e) => { const t = e.target as HTMLTextAreaElement; t.style.height = 'auto'; t.style.height = t.scrollHeight + 'px'; }}></textarea>{/if}</td>
          {#if showQty}
            <td class="!p-1">{#if readonly}<div class="px-2 text-right num">{l.quantity}</div>{:else}<input class="grid-in num text-right" bind:value={l.quantity} />{/if}</td>
            <td class="!p-1">{#if readonly}<div class="px-2 text-right num">{l.unit_price}</div>{:else}<input class="grid-in num text-right" bind:value={l.unit_price} placeholder={l.amount ? '' : '0.00'} />{/if}</td>
          {/if}
          {#if hasDiscount || showDisc}<td class="!p-1">{#if readonly}<div class="px-2 text-right num">{l.discount}</div>{:else}<input class="grid-in num text-right" bind:value={l.discount} />{/if}</td>{/if}
          <td class="!p-1">{#if readonly}<span class="px-2">{lookups.accounts.find((a) => a.id === l.account_id)?.code ?? ''} {lookups.accounts.find((a) => a.id === l.account_id)?.name ?? ''}</span>{:else}<Combo compact bind:value={l.account_id} options={accts} onchange={(v) => pickAccount(i, v)} />{/if}</td>
          {#if lat !== 'NOTAX'}<td class="!p-1">{#if readonly}<span class="px-2">{lookups.taxRates.find((t) => t.id === l.tax_rate_id)?.name ?? ''}</span>{:else}<Combo compact bind:value={l.tax_rate_id} options={taxes} />{/if}</td>{/if}
          {#each cats as c, k}
            <td class="!p-1">{#if readonly}<span class="px-2">{c.options.find((o: any) => o.id === (k === 0 ? l.tracking1 : l.tracking2))?.name ?? ''}</span>{:else if k === 0}<Combo compact bind:value={l.tracking1} options={trackingOptions(c)} />{:else}<Combo compact bind:value={l.tracking2} options={trackingOptions(c)} />{/if}</td>
          {/each}
          <td class="!p-1 text-right">
            {#if !showQty && !readonly}<input class="grid-in num text-right" bind:value={l.amount} placeholder="0.00" />
            {:else}<span class="num px-2">{money(r.la, { blankZero: isBlank(l) })}</span>{/if}
          </td>
          {#if !readonly}
            <td class="!p-1 text-right whitespace-nowrap">
              {#if !isBlank(l)}
                <span class="invisible group-hover:visible">
                  <button class="text-faint hover:text-text" title="Move up" onclick={() => move(i, -1)}>↑</button>
                  <button class="text-faint hover:text-text" title="Duplicate" onclick={() => dup(i)}>⧉</button>
                  <button class="text-faint hover:text-bad" title="Remove line" aria-label="Remove line" onclick={() => remove(i)}><Icon name="trash" size={14} /></button>
                </span>
              {/if}
            </td>
          {/if}
        </tr>
      {/each}
    </tbody>
  </table>
</div>
{#if !readonly}
  <div class="mt-2 flex gap-2">
    <button class="btn btn-sm" onclick={() => lines.push(blank())}><Icon name="plus" size={14} />Add a new line</button>
    {#if showQty && !hasDiscount}<button class="btn btn-sm btn-ghost" onclick={() => (showDisc = !showDisc)}>{showDisc ? 'Hide' : 'Show'} discount</button>{/if}
  </div>
{/if}
