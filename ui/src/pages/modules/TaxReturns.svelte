<script lang="ts">
  import { rpc } from '../../lib/api';
  import { money, date } from '../../lib/format';
  import { navigate } from '../../lib/router.svelte';
  import { can, toastError, setting } from '../../lib/stores.svelte';
  import PageHeader from '../../components/PageHeader.svelte';
  import Status from '../../components/Status.svelte';
  import Icon from '../../components/Icon.svelte';
  let d = $state<any>(null);
  rpc('taxReturns.list').then((r) => { d = r; f.from = r.next[0]; f.to = r.next[1]; f.form = r.form; }).catch(toastError);
  let f = $state({ from: '', to: '', form: '', basis: setting('tax_basis', 'ACCRUAL') });
  const FORMS: Record<string, string> = { 'AU-BAS': 'Australia — Business Activity Statement', 'NZ-GST101': 'New Zealand — GST101A', 'UK-VAT100': 'United Kingdom — VAT100 (MTD)', 'IE-VAT3': 'Ireland — VAT3', 'CA-GST34': 'Canada — GST/HST return', 'US-SALESTAX': 'United States — sales tax liability', GENERIC: 'Generic VAT/GST return' };
  async function create() { try { const r = await rpc('taxReturns.create', f); navigate(`/accounting/tax-returns/${r.id}`); } catch (e) { toastError(e); } }
  const net = (r: any) => r.data?.boxes?.find((b: any) => b.total)?.amount ?? 0;
</script>

<PageHeader title="{setting('tax_label', 'Tax')} returns" crumbs={[{ label: 'Accounting', href: '/reports' }]}>
  <a class="btn" href="/reports/tax-reconciliation"><Icon name="chart" />Tax reconciliation report</a>
</PageHeader>
{#if d}
  {#if can('accounting:write')}
    <div class="card mb-4 flex flex-wrap items-end gap-3 p-4">
      <div><span class="label">Return</span><select class="select w-80" bind:value={f.form}>{#each Object.entries(FORMS) as [k, v]}<option value={k}>{v}</option>{/each}</select></div>
      <div><label class="label" for="tf">From</label><input id="tf" type="date" class="input" bind:value={f.from} /></div>
      <div><label class="label" for="tt">To</label><input id="tt" type="date" class="input" bind:value={f.to} /></div>
      <div><span class="label">Basis</span><select class="select w-32" bind:value={f.basis}><option value="ACCRUAL">Accrual</option><option value="CASH">Cash</option></select></div>
      <button class="btn btn-primary" onclick={create}>Prepare return</button>
    </div>
  {/if}
  <div class="card overflow-x-auto">
    <table class="tbl"><thead><tr><th>Period</th><th>Return</th><th>Basis</th><th class="n">Net amount</th><th>Status</th></tr></thead>
      <tbody>{#each d.returns as r}<tr class="clickable" onclick={() => navigate(`/accounting/tax-returns/${r.id}`)}><td>{date(r.period_start)} – {date(r.period_end)}</td><td>{FORMS[r.jurisdiction] ?? r.jurisdiction}</td><td class="text-muted">{r.basis.toLowerCase()}</td><td class="n">{money(net(r))}</td><td><Status status={r.status} /></td></tr>
      {:else}<tr><td colspan="5" class="py-10 text-center text-muted">No returns yet. Choose a period above and prepare your first return.</td></tr>{/each}</tbody></table>
  </div>
  <p class="mt-3 text-[13px] text-muted">Returns are calculated from the ledger and always reconcile to the Tax Reconciliation report. Filing a return locks the period for non-adviser users. Lodging with the tax authority (ATO, IRD, HMRC MTD, Revenue, CRA) requires registered software credentials — LedgerCraft prepares the figures and export files for you to lodge.</p>
{/if}
