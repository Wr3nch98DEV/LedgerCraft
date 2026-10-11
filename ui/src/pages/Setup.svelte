<script lang="ts">
  import { rpc } from '../lib/api';
  import { session, toastError } from '../lib/stores.svelte';
  import { route } from '../lib/router.svelte';
  import Icon from '../components/Icon.svelte';

  const countries = [
    { code: 'AU', name: 'Australia', cur: 'AUD', fy: 6 }, { code: 'NZ', name: 'New Zealand', cur: 'NZD', fy: 3 },
    { code: 'GB', name: 'United Kingdom', cur: 'GBP', fy: 3 }, { code: 'IE', name: 'Ireland', cur: 'EUR', fy: 12 },
    { code: 'CA', name: 'Canada', cur: 'CAD', fy: 12 }, { code: 'US', name: 'United States', cur: 'USD', fy: 12 },
    { code: 'OTHER', name: 'Other (generic VAT/GST)', cur: 'USD', fy: 12 },
  ];
  const months = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
  const orgs = $derived(session.data?.orgs ?? []);
  let creating = $state(route.query.get('new') === '1' || !(session.data?.orgs?.length));
  let busy = $state(false);
  let f = $state({ org_name: '', country: 'AU', base_currency: 'AUD', fy_end_month: 6, tax_basis: 'ACCRUAL', tax_registered: true, tax_number: '', tax_period: '', owner_name: '', owner_email: '', demo: false });

  function setCountry(c: string) {
    const x = countries.find((k) => k.code === c)!;
    f.country = c; f.base_currency = x.cur; f.fy_end_month = x.fy;
  }
  async function create() {
    if (!f.org_name.trim()) return;
    busy = true;
    try {
      await rpc('orgs.create', { ...f, country: f.country === 'OTHER' ? 'XX' : f.country });
      location.href = '/';
    } catch (e) { toastError(e); busy = false; }
  }
  async function demo() {
    busy = true;
    try { await rpc('orgs.create', { org_name: 'Harbourside Design Co (demo)', country: 'AU', demo: true, owner_name: 'Demo User' }); location.href = '/'; }
    catch (e) { toastError(e); busy = false; }
  }
  async function open(file: string) { await rpc('orgs.open', { file }); location.href = '/'; }
  const taxWord = $derived(f.country === 'US' ? 'sales tax' : ['GB', 'IE', 'OTHER'].includes(f.country) ? 'VAT' : 'GST');
</script>

<div class="min-h-screen bg-bg px-4 py-12">
  <div class="mx-auto max-w-2xl">
    <div class="mb-8 flex items-center gap-3">
      <span class="grid h-10 w-10 place-items-center rounded-xl bg-accent text-lg font-bold text-accent-ink">L</span>
      <div><div class="text-xl font-semibold">LedgerCraft</div><div class="text-muted">Open-source accounting for small business</div></div>
    </div>

    {#if !creating && orgs.length}
      <div class="card p-6">
        <h1 class="mb-4 text-lg font-semibold">Choose an organisation</h1>
        <div class="space-y-2">
          {#each orgs as o}
            <button class="flex w-full items-center justify-between rounded-lg border border-line px-4 py-3 text-left hover:border-accent hover:bg-hover" onclick={() => open(o.file)}>
              <span><span class="font-medium">{o.name}</span><span class="ml-2 text-[12px] text-faint">{o.file}</span></span><Icon name="arrowRight" class="text-muted" />
            </button>
          {/each}
        </div>
        <div class="mt-5 flex gap-2"><button class="btn" onclick={() => (creating = true)}><Icon name="plus" />New organisation</button><button class="btn" disabled={busy} onclick={demo}>Try the demo company</button></div>
      </div>
    {:else}
      <div class="card p-6">
        <h1 class="text-lg font-semibold">Set up your organisation</h1>
        <p class="mb-5 mt-1 text-muted">You can change any of this later in Settings. Everything is stored in one file on this computer.</p>
        <form class="grid grid-cols-1 gap-4 sm:grid-cols-2" onsubmit={(e) => { e.preventDefault(); create(); }}>
          <div class="sm:col-span-2"><label class="label" for="on">Organisation name</label><input id="on" class="input" bind:value={f.org_name} placeholder="e.g. Harbourside Design Co" required /></div>
          <div><label class="label" for="ct">Country</label>
            <select id="ct" class="select" value={f.country} onchange={(e) => setCountry((e.target as HTMLSelectElement).value)}>{#each countries as c}<option value={c.code}>{c.name}</option>{/each}</select></div>
          <div><label class="label" for="cur">Base currency</label><input id="cur" class="input uppercase" maxlength="3" bind:value={f.base_currency} /></div>
          <div><label class="label" for="fy">Financial year ends</label>
            <select id="fy" class="select" bind:value={f.fy_end_month}>{#each months as m, i}<option value={i + 1}>Last day of {m}</option>{/each}</select></div>
          <div><label class="label" for="tb">{taxWord.toUpperCase()} accounting basis</label>
            <select id="tb" class="select" bind:value={f.tax_basis}><option value="ACCRUAL">Accrual (invoice) basis</option><option value="CASH">Cash (payments) basis</option></select></div>
          <div class="flex items-center gap-2 sm:col-span-2"><input id="reg" type="checkbox" bind:checked={f.tax_registered} class="h-4 w-4 accent-[var(--accent)]" /><label for="reg">Registered for {taxWord}</label></div>
          {#if f.tax_registered}<div class="sm:col-span-2"><label class="label" for="tn">{f.country === 'AU' ? 'ABN' : f.country === 'NZ' ? 'GST number' : f.country === 'US' ? 'EIN / sales tax ID' : 'VAT / tax number'}</label><input id="tn" class="input" bind:value={f.tax_number} /></div>{/if}
          <div><label class="label" for="yn">Your name</label><input id="yn" class="input" bind:value={f.owner_name} /></div>
          <div><label class="label" for="ye">Your email</label><input id="ye" type="email" class="input" bind:value={f.owner_email} /></div>
          <div class="flex items-center gap-2 sm:col-span-2"><input id="dm" type="checkbox" bind:checked={f.demo} class="h-4 w-4 accent-[var(--accent)]" /><label for="dm">Fill with 12 months of sample data (for exploring)</label></div>
          <div class="flex justify-between gap-2 pt-2 sm:col-span-2">
            {#if orgs.length}<button type="button" class="btn" onclick={() => (creating = false)}>Back</button>{:else}<button type="button" class="btn" disabled={busy} onclick={demo}>Just show me the demo company</button>{/if}
            <button class="btn btn-primary" disabled={busy || !f.org_name.trim()}>{busy ? 'Setting up…' : 'Create organisation'}</button>
          </div>
        </form>
      </div>
    {/if}
    <p class="mt-6 text-center text-[12px] text-faint">LedgerCraft is not affiliated with or endorsed by Xero Limited.</p>
  </div>
</div>
