<script lang="ts">
  import { rpc } from '../lib/api';
  import { navigate } from '../lib/router.svelte';
  import { toast, toastError } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Combo from '../components/Combo.svelte';
  import Icon from '../components/Icon.svelte';
  import { accountOptions, taxOptions, currencyOptions } from '../components/Pickers';

  let { id = undefined }: { id?: string } = $props();
  let groups = $state<any[]>([]);
  let f = $state<any>({ name: '', first_name: '', last_name: '', email: '', phone: '', mobile: '', website: '', tax_number: '', account_number: '', bank_account: '',
    postal_address: '', street_address: '', is_customer: null, is_supplier: null, default_currency: null, sales_account_id: null, purchase_account_id: null,
    sales_tax_rate_id: null, purchase_tax_rate_id: null, sales_terms_type: '', sales_terms_days: null, bill_terms_type: '', bill_terms_days: null, discount_bp: 0,
    credit_limit: null, notes: '', people: [], groups: [] });
  let busy = $state(false);
  rpc('contactGroups.list').then((g) => (groups = g));
  if (id) rpc('contacts.get', { id: Number(id) }).then((c) => {
    f = { ...c, is_customer: !!c.is_customer, is_supplier: !!c.is_supplier, sales_terms_type: c.sales_terms_type ?? '', bill_terms_type: c.bill_terms_type ?? '', groups: c.groups.map((g: any) => g.id),
      people: c.people.map((p: any) => ({ ...p, include_in_emails: !!p.include_in_emails })) };
  }).catch(toastError);
  const termTypes = [['', 'Organisation default'], ['DAYSAFTERBILLDATE', 'day(s) after the invoice date'], ['DAYSAFTERBILLMONTH', 'day(s) after the end of the invoice month'], ['OFCURRENTMONTH', 'of the current month'], ['OFFOLLOWINGMONTH', 'of the following month']];
  async function save(e: Event) {
    e.preventDefault(); busy = true;
    const body = { ...f, sales_terms_days: f.sales_terms_days === '' || f.sales_terms_days === null ? null : Number(f.sales_terms_days), bill_terms_days: f.bill_terms_days === '' || f.bill_terms_days === null ? null : Number(f.bill_terms_days) };
    try {
      if (id) { await rpc('contacts.update', { id: Number(id), contact: body }); toast('Contact saved'); navigate(`/contacts/${id}`); }
      else { const r = await rpc('contacts.create', body); toast('Contact created'); navigate(`/contacts/${r.id}`); }
    } catch (x) { toastError(x); } finally { busy = false; }
  }
</script>

<PageHeader title={id ? `Edit ${f.name}` : 'New contact'} crumbs={[{ label: 'Contacts', href: '/contacts' }]} />
<form class="grid gap-4 lg:grid-cols-2" onsubmit={save}>
  <section class="card space-y-3 p-5">
    <h2 class="font-semibold">Details</h2>
    <div><label class="label" for="cn">Contact name *</label><input id="cn" class="input" bind:value={f.name} required /></div>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="fn">First name</label><input id="fn" class="input" bind:value={f.first_name} /></div><div><label class="label" for="ln">Last name</label><input id="ln" class="input" bind:value={f.last_name} /></div></div>
    <div><label class="label" for="em">Email</label><input id="em" type="email" class="input" bind:value={f.email} /></div>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="ph">Phone</label><input id="ph" class="input" bind:value={f.phone} /></div><div><label class="label" for="mo">Mobile</label><input id="mo" class="input" bind:value={f.mobile} /></div></div>
    <div><label class="label" for="ws">Website</label><input id="ws" class="input" bind:value={f.website} /></div>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="pa">Postal address</label><textarea id="pa" rows="3" class="textarea" bind:value={f.postal_address}></textarea></div><div><label class="label" for="sa">Street address</label><textarea id="sa" rows="3" class="textarea" bind:value={f.street_address}></textarea></div></div>
    <div class="flex gap-5"><label class="flex items-center gap-2"><input type="checkbox" bind:checked={f.is_customer} class="accent-[var(--accent)]" />Customer</label><label class="flex items-center gap-2"><input type="checkbox" bind:checked={f.is_supplier} class="accent-[var(--accent)]" />Supplier</label></div>
  </section>
  <section class="card space-y-3 p-5">
    <h2 class="font-semibold">Financial details</h2>
    <div class="grid grid-cols-2 gap-3"><div><label class="label" for="tn">Tax / business number</label><input id="tn" class="input" bind:value={f.tax_number} /></div><div><label class="label" for="an">Account number</label><input id="an" class="input" bind:value={f.account_number} /></div></div>
    <div><label class="label" for="ba">Bank account (for batch payments)</label><input id="ba" class="input" bind:value={f.bank_account} placeholder="BSB / sort code + account" /></div>
    <div class="grid grid-cols-2 gap-3">
      <div><span class="label">Default sales account</span><Combo bind:value={f.sales_account_id} options={accountOptions((a) => a.type !== 'BANK')} /></div>
      <div><span class="label">Default sales tax</span><Combo bind:value={f.sales_tax_rate_id} options={taxOptions()} /></div>
      <div><span class="label">Default purchases account</span><Combo bind:value={f.purchase_account_id} options={accountOptions((a) => a.type !== 'BANK')} /></div>
      <div><span class="label">Default purchases tax</span><Combo bind:value={f.purchase_tax_rate_id} options={taxOptions()} /></div>
      <div><span class="label">Currency</span><Combo bind:value={f.default_currency} options={currencyOptions()} /></div>
    </div>
    <div><span class="label">Invoices due</span><div class="flex gap-2"><input class="input w-20" type="number" min="0" bind:value={f.sales_terms_days} /><select class="select" bind:value={f.sales_terms_type}>{#each termTypes as [k, l]}<option value={k}>{l}</option>{/each}</select></div></div>
    <div><span class="label">Bills due</span><div class="flex gap-2"><input class="input w-20" type="number" min="0" bind:value={f.bill_terms_days} /><select class="select" bind:value={f.bill_terms_type}>{#each termTypes as [k, l]}<option value={k}>{l}</option>{/each}</select></div></div>
    {#if groups.length}<div><span class="label">Groups</span><div class="flex flex-wrap gap-3">{#each groups as g}<label class="flex items-center gap-1.5"><input type="checkbox" checked={f.groups.includes(g.id)} onchange={(e) => { const c = (e.target as HTMLInputElement).checked; f.groups = c ? [...f.groups, g.id] : f.groups.filter((x: number) => x !== g.id); }} class="accent-[var(--accent)]" />{g.name}</label>{/each}</div></div>{/if}
    <div><label class="label" for="nt">Notes</label><textarea id="nt" rows="3" class="textarea" bind:value={f.notes}></textarea></div>
  </section>
  <section class="card space-y-3 p-5 lg:col-span-2">
    <div class="flex items-center justify-between"><h2 class="font-semibold">Additional people</h2><button type="button" class="btn btn-sm" onclick={() => f.people.push({ first_name: '', last_name: '', email: '', include_in_emails: true })}><Icon name="plus" size={14} />Add person</button></div>
    {#each f.people as p, i}
      <div class="grid grid-cols-[1fr_1fr_2fr_auto_auto] items-center gap-2">
        <input class="input" placeholder="First name" bind:value={p.first_name} /><input class="input" placeholder="Last name" bind:value={p.last_name} /><input class="input" placeholder="Email" bind:value={p.email} />
        <label class="flex items-center gap-1.5 whitespace-nowrap text-[13px]"><input type="checkbox" bind:checked={p.include_in_emails} class="accent-[var(--accent)]" />Include in emails</label>
        <button type="button" class="btn btn-ghost btn-sm" aria-label="Remove" onclick={() => f.people.splice(i, 1)}><Icon name="trash" size={14} /></button>
      </div>
    {:else}<div class="text-muted">No additional people.</div>{/each}
  </section>
  <div class="flex justify-end gap-2 lg:col-span-2"><a class="btn" href={id ? `/contacts/${id}` : '/contacts'}>Cancel</a><button class="btn btn-primary" disabled={busy}>Save</button></div>
</form>
