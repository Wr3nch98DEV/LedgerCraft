<script lang="ts">
  import { rpc } from '../lib/api';
  import { dateTime, date } from '../lib/format';
  import { navigate } from '../lib/router.svelte';
  import { session, can, confirm, toast, toastError, loadSession } from '../lib/stores.svelte';
  import PageHeader from '../components/PageHeader.svelte';
  import Modal from '../components/Modal.svelte';
  import Icon from '../components/Icon.svelte';
  import ImportExport from './ImportExport.svelte';

  let { section = '' }: { section?: string } = $props();
  const sections = [
    { key: 'organisation', label: 'Organisation details', icon: 'briefcase', perm: 'settings:read' },
    { key: 'financial', label: 'Financial settings & lock dates', icon: 'lock', perm: 'settings:read' },
    { key: 'users', label: 'Users & roles', icon: 'users', perm: 'users:read' },
    { key: 'branding', label: 'Invoice settings & branding', icon: 'doc', perm: 'settings:read' },
    { key: 'emails', label: 'Email templates & reminders', icon: 'mail', perm: 'settings:read' },
    { key: 'outbox', label: 'Outbox', icon: 'mail', perm: 'settings:read' },
    { key: 'payments', label: 'Online payments', icon: 'dollar', perm: 'settings:read' },
    { key: 'connected', label: 'Connected apps & API', icon: 'link', perm: 'settings:read' },
    { key: 'import', label: 'Import & export', icon: 'upload', perm: 'settings:read' },
    { key: 'backup', label: 'Backup & data', icon: 'download', perm: 'settings:read' },
    { key: 'profile', label: 'Your profile & password', icon: 'users' },
  ];
  const s = $derived(section || 'organisation');
  let st = $state<any>({});
  async function loadSettings() { st = { ...(await rpc('settings.get')) }; }
  loadSettings().catch(toastError);
  async function saveSettings(keys: string[]) {
    try { const body: any = {}; for (const k of keys) body[k] = st[k] ?? ''; await rpc('settings.update', body); toast('Settings saved'); await loadSession(); await loadSettings(); } catch (e) { toastError(e); }
  }
  const months = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];

  // users
  let users = $state<any[]>([]);
  let userOpen = $state(false);
  let uf = $state<any>({});
  let editUser = $state<number | null>(null);
  const ROLES = [['ADVISER', 'Adviser — full access incl. lock dates & users'], ['STANDARD', 'Standard — everything except users & payroll'], ['INVOICE_ONLY', 'Invoice only — sales & contacts'],
    ['READONLY', 'Read only — view everything except payroll'], ['CASHBOOK', 'Cashbook — banking, contacts & reports'], ['PAYROLL_ADMIN', 'Payroll admin — payroll only']];
  $effect(() => { if (s === 'users' && can('users:read')) rpc('users.list').then((u) => (users = u)).catch(toastError); });
  async function saveUser() {
    try {
      if (editUser) await rpc('users.update', { id: editUser, user: uf }); else await rpc('users.create', uf);
      toast('User saved'); userOpen = false; users = await rpc('users.list');
    } catch (e) { toastError(e); }
  }
  async function toggleUser(u: any) { await rpc('users.setActive', { id: u.id, active: !u.active }).catch(toastError); users = await rpc('users.list'); }

  // branding
  let themes = $state<any[]>([]);
  let th = $state<any>(null);
  let seqs = $state<any[]>([]);
  const seqLabels: Record<string, string> = { INV: 'Invoices', CN: 'Credit notes', QU: 'Quotes', PO: 'Purchase orders', BILL: 'Bills (optional)', MJ: 'Manual journals', ASSET: 'Fixed assets', PAYRUN: 'Pay runs', EXP: 'Expense claims' };
  $effect(() => { if (s === 'branding') rpc('sequences.list').then((x) => (seqs = x)).catch(toastError); });
  async function saveSeq(q: any) {
    try { await rpc('sequences.save', { name: q.name, prefix: q.prefix, next: Number(q.next) }); toast(`${seqLabels[q.name] ?? q.name}: next number is ${q.prefix}${String(q.next).padStart(q.width, '0')}`); } catch (e) { toastError(e); }
  }
  $effect(() => { if (s === 'branding') rpc('branding.list').then((t) => { themes = t; th = th ?? { ...t[0], payment_advice: !!t[0]?.payment_advice, show_tax_number: !!t[0]?.show_tax_number }; }); });
  async function uploadLogo(f: File) {
    if (f.size > 2 * 1024 * 1024) { toastError('Logos must be 2 MB or smaller'); return; }
    const data = await new Promise<string>((ok) => { const fr = new FileReader(); fr.onload = () => ok(String(fr.result)); fr.readAsDataURL(f); });
    try { const r = await rpc('attachments.add', { entity: 'branding', entity_id: th.id ?? null, filename: f.name, mime: f.type, data }); th.logo_attachment_id = r.id; toast('Logo uploaded — save the theme to keep it'); } catch (e) { toastError(e); }
  }
  async function saveTheme() { try { const r = await rpc('branding.save', { id: th.id ?? null, theme: th, default: !!th.is_default }); toast('Theme saved'); themes = await rpc('branding.list'); th = { ...themes.find((t) => t.id === r.id) }; } catch (e) { toastError(e); } }

  // emails
  let templates = $state<any[]>([]);
  let reminders = $state<any[]>([]);
  $effect(() => { if (s === 'emails') { rpc('emailTemplates.list').then((t) => (templates = t)); rpc('reminders.list').then((r) => (reminders = r)); } });
  async function saveTemplate(t: any) { await rpc('emailTemplates.save', t).catch(toastError); toast('Template saved'); }
  async function addReminder() { await rpc('reminders.save', { days: 7 }); reminders = await rpc('reminders.list'); }
  async function saveReminder(r: any) { await rpc('reminders.save', { id: r.id, days: Number(r.days), active: !!r.active }); toast('Reminder saved'); }
  async function delReminder(r: any) { await rpc('reminders.delete', { id: r.id }); reminders = await rpc('reminders.list'); }

  let testTo = $state('');
  async function testEmail() { try { await rpc('email.test', { to: testTo }); toast('Test email sent'); } catch (e) { toastError(e); } }
  async function runJobs() { try { const r = await rpc('jobs.run'); toast(`Sent ${r.emails} email(s)`); outbox = await rpc('outbox.list'); } catch (e) { toastError(e); } }
  // outbox
  let outbox = $state<any[]>([]);
  $effect(() => { if (s === 'outbox') rpc('outbox.list').then((o) => (outbox = o)); });

  // API
  let tokens = $state<any[]>([]);
  let hooks = $state<any[]>([]);
  let newToken = $state('');
  let tokenName = $state('');
  let hookUrl = $state('');
  $effect(() => { if (s === 'connected') { rpc('api.tokens').then((t) => (tokens = t)).catch(() => {}); rpc('webhooks.list').then((h) => (hooks = h)).catch(() => {}); } });
  async function createToken() { try { const r = await rpc('api.createToken', { name: tokenName, scopes: ['*'] }); newToken = r.token; tokenName = ''; tokens = await rpc('api.tokens'); } catch (e) { toastError(e); } }
  async function revoke(t: any) { if (await confirm('Revoke this token?', 'Apps using it will stop working.', 'Revoke', true)) { await rpc('api.revokeToken', { id: t.id }); tokens = await rpc('api.tokens'); } }
  async function addHook() { try { await rpc('webhooks.save', { url: hookUrl, events: ['*'] }); hookUrl = ''; hooks = await rpc('webhooks.list'); } catch (e) { toastError(e); } }

  let tfa = $state<any>(null);
  let tfaCode = $state('');
  async function begin2fa() { try { tfa = await rpc('users.begin2fa'); } catch (e) { toastError(e); } }
  async function confirm2fa() { try { await rpc('users.confirm2fa', { code: tfaCode }); tfa = null; tfaCode = ''; toast('Two-step verification is on'); } catch (e) { toastError(e); } }
  async function disable2fa() { if (await confirm('Turn off two-step verification?')) { await rpc('users.disable2fa'); toast('Two-step verification turned off'); } }
  // profile
  let pw = $state({ a: '', b: '' });
  async function setPassword() {
    if (pw.a !== pw.b) { toastError('Passwords do not match'); return; }
    try { await rpc('users.setPassword', { id: session.data.user.id, password: pw.a || null }); toast(pw.a ? 'Password set — you will need it to sign in' : 'Password removed'); pw = { a: '', b: '' }; } catch (e) { toastError(e); }
  }
</script>

<PageHeader title="Settings" />
<div class="grid gap-5 lg:grid-cols-[250px_1fr]">
  <nav class="space-y-0.5">
    {#each sections as x}{#if !x.perm || can(x.perm)}
      <a href="/settings/{x.key}" class="flex items-center gap-2.5 rounded-lg px-3 py-2 {s === x.key ? 'bg-accent-soft text-accent' : 'text-muted hover:bg-hover hover:text-text'}"><Icon name={x.icon} size={15} />{x.label}</a>
    {/if}{/each}
    <div class="mt-3 border-t border-line pt-3 text-[13px]">
      <a class="block rounded-lg px-3 py-1.5 text-muted hover:bg-hover" href="/accounting/chart">Chart of accounts</a>
      <a class="block rounded-lg px-3 py-1.5 text-muted hover:bg-hover" href="/accounting/tax-rates">Tax rates</a>
      <a class="block rounded-lg px-3 py-1.5 text-muted hover:bg-hover" href="/accounting/currencies">Currencies</a>
      <a class="block rounded-lg px-3 py-1.5 text-muted hover:bg-hover" href="/accounting/tracking">Tracking categories</a>
      <a class="block rounded-lg px-3 py-1.5 text-muted hover:bg-hover" href="/accounting/conversion">Conversion balances</a>
      <a class="block rounded-lg px-3 py-1.5 text-muted hover:bg-hover" href="/payroll/settings">Payroll settings</a>
    </div>
  </nav>

  <div>
    {#if s === 'organisation'}
      <section class="card space-y-3 p-5">
        <h2 class="text-lg font-semibold">Organisation details</h2>
        <div class="grid gap-3 sm:grid-cols-2">
          <div><label class="label" for="on">Display name</label><input id="on" class="input" bind:value={st.org_name} /></div>
          <div><label class="label" for="ln">Legal / trading name</label><input id="ln" class="input" bind:value={st.legal_name} /></div>
          <div><label class="label" for="tn">Tax / business number</label><input id="tn" class="input" bind:value={st.tax_number} /></div>
          <div><label class="label" for="cn">Company number</label><input id="cn" class="input" bind:value={st.company_number} /></div>
          <div class="sm:col-span-2"><label class="label" for="ad">Address</label><textarea id="ad" rows="3" class="textarea" bind:value={st.address}></textarea></div>
          <div><label class="label" for="ph">Phone</label><input id="ph" class="input" bind:value={st.phone} /></div>
          <div><label class="label" for="em">Email</label><input id="em" class="input" bind:value={st.email} /></div>
          <div><label class="label" for="ws">Website</label><input id="ws" class="input" bind:value={st.website} /></div>
          <div><label class="label" for="in">Industry</label><input id="in" class="input" bind:value={st.industry} /></div>
        </div>
        <div class="text-[13px] text-muted">Country: {st.country} · Base currency: {st.base_currency} (set when the organisation was created)</div>
        {#if can('settings:write')}<div class="flex justify-end"><button class="btn btn-primary" onclick={() => saveSettings(['org_name', 'legal_name', 'tax_number', 'company_number', 'address', 'phone', 'email', 'website', 'industry'])}>Save</button></div>{/if}
      </section>
    {:else if s === 'financial'}
      <section class="card space-y-4 p-5">
        <h2 class="text-lg font-semibold">Financial settings</h2>
        <div class="grid gap-3 sm:grid-cols-2">
          <div><label class="label" for="fy">Financial year end</label><select id="fy" class="select" bind:value={st.fy_end_month}>{#each months as m, i}<option value={String(i + 1)}>Last day of {m}</option>{/each}</select></div>
          <div><label class="label" for="tb">{st.tax_label} basis</label><select id="tb" class="select" bind:value={st.tax_basis}><option value="ACCRUAL">Accrual / invoice basis</option><option value="CASH">Cash / payments basis</option></select></div>
          <div><label class="label" for="tp">{st.tax_label} period</label><select id="tp" class="select" bind:value={st.tax_period}><option value="MONTHLY">Monthly</option><option value="TWOMONTHLY">Two-monthly</option><option value="QUARTERLY">Quarterly</option><option value="SIXMONTHLY">Six-monthly</option><option value="ANNUALLY">Annually</option></select></div>
          <div><label class="label" for="tr">Registered for {st.tax_label}</label><select id="tr" class="select" bind:value={st.tax_registered}><option value="1">Yes</option><option value="0">No</option></select></div>
          <div><label class="label" for="st">Default invoice due (days after invoice date)</label><input id="st" type="number" class="input" bind:value={st.sales_terms_days} /></div>
          <div><label class="label" for="bt">Default bill due (days after bill date)</label><input id="bt" type="number" class="input" bind:value={st.bill_terms_days} /></div>
          <div><label class="label" for="sl">Default sales amounts are</label><select id="sl" class="select" bind:value={st.default_sales_lat}><option value="EXCLUSIVE">Tax exclusive</option><option value="INCLUSIVE">Tax inclusive</option><option value="NOTAX">No tax</option></select></div>
          <div><label class="label" for="pl">Default purchase amounts are</label><select id="pl" class="select" bind:value={st.default_purchases_lat}><option value="INCLUSIVE">Tax inclusive</option><option value="EXCLUSIVE">Tax exclusive</option><option value="NOTAX">No tax</option></select></div>
        </div>
        {#if can('settings:write')}<div class="flex justify-end"><button class="btn btn-primary" onclick={() => saveSettings(['fy_end_month', 'tax_basis', 'tax_period', 'tax_registered', 'sales_terms_days', 'bill_terms_days', 'default_sales_lat', 'default_purchases_lat'])}>Save</button></div>{/if}
      </section>
      <section class="card mt-4 space-y-3 p-5">
        <h2 class="flex items-center gap-2 text-lg font-semibold"><Icon name="lock" />Lock dates</h2>
        <p class="text-[13px] text-muted">Lock dates stop transactions being entered or changed on or before a date. Enforced by the database for every change, including imports and the API.</p>
        <div class="grid gap-3 sm:grid-cols-2">
          <div><label class="label" for="lo">Stop all users except advisers on or before</label><input id="lo" type="date" class="input" bind:value={st.lock_date_others} disabled={!can('lockdates:write')} /></div>
          <div><label class="label" for="la">Stop all users (including advisers) on or before</label><input id="la" type="date" class="input" bind:value={st.lock_date_all} disabled={!can('lockdates:write')} /></div>
        </div>
        {#if can('lockdates:write')}<div class="flex justify-end"><button class="btn btn-primary" onclick={() => saveSettings(['lock_date_others', 'lock_date_all'])}>Save lock dates</button></div>{:else}<div class="text-[13px] text-muted">Only advisers can change lock dates.</div>{/if}
      </section>
    {:else if s === 'users'}
      <section class="card p-5">
        <div class="mb-3 flex items-center justify-between"><h2 class="text-lg font-semibold">Users</h2>{#if can('users:write')}<button class="btn btn-primary" onclick={() => { editUser = null; uf = { name: '', email: '', role: 'STANDARD', password: '' }; userOpen = true; }}><Icon name="plus" />Invite a user</button>{/if}</div>
        <table class="tbl"><thead><tr><th>Name</th><th>Email</th><th>Role</th><th>Sign-in</th><th>Status</th><th></th></tr></thead>
          <tbody>{#each users as u}<tr><td class="font-medium">{u.name}</td><td class="text-muted">{u.email}</td><td>{ROLES.find((r) => r[0] === u.role)?.[1].split(' — ')[0] ?? u.role}</td>
            <td class="text-muted">{u.has_password ? 'Password' : 'No password'}</td><td>{u.active ? 'Active' : 'Inactive'}</td>
            <td class="text-right">{#if can('users:write')}<button class="btn btn-ghost btn-sm" onclick={() => { editUser = u.id; uf = { name: u.name, email: u.email, role: u.role, password: '' }; userOpen = true; }}>Edit</button>{#if u.id !== 1}<button class="btn btn-ghost btn-sm" onclick={() => toggleUser(u)}>{u.active ? 'Deactivate' : 'Activate'}</button>{/if}{/if}</td></tr>{/each}</tbody></table>
        <p class="mt-3 text-[13px] text-muted">Once any user has a password, LedgerCraft asks everyone to sign in. Passwords are stored as Argon2 hashes.</p>
      </section>
    {:else if s === 'branding' && th}
      <section class="card mb-4 p-5">
        <h2 class="mb-1 text-lg font-semibold">Invoice settings</h2>
        <p class="mb-3 text-[13px] text-muted">Prefix and next number used for new documents. Changing the next number doesn't renumber existing documents.</p>
        <div class="overflow-x-auto"><table class="tbl text-[13px]"><thead><tr><th>Document</th><th>Prefix</th><th>Next number</th><th>Next will be</th><th></th></tr></thead>
          <tbody>{#each seqs as q (q.name)}<tr>
            <td>{seqLabels[q.name] ?? q.name}</td>
            <td><input class="input w-28" aria-label="{seqLabels[q.name] ?? q.name} prefix" bind:value={q.prefix} disabled={!can('settings:write')} /></td>
            <td><input class="input num w-28" type="number" min="1" aria-label="{seqLabels[q.name] ?? q.name} next number" bind:value={q.next} disabled={!can('settings:write')} /></td>
            <td class="font-mono">{q.prefix}{String(q.next).padStart(q.width, '0')}</td>
            <td class="text-right">{#if can('settings:write')}<button class="btn btn-sm" onclick={() => saveSeq(q)}>Save</button>{/if}</td>
          </tr>{/each}</tbody></table></div>
      </section>
      <section class="card p-5">
        <div class="mb-3 flex items-center justify-between"><h2 class="text-lg font-semibold">Branding themes</h2>
          <div class="flex gap-2">{#each themes as t}<button class="btn btn-sm {th.id === t.id ? 'btn-primary' : ''}" onclick={() => (th = { ...t, payment_advice: !!t.payment_advice, show_tax_number: !!t.show_tax_number })}>{t.name}{t.is_default ? ' ★' : ''}</button>{/each}
            <button class="btn btn-sm" onclick={() => (th = { name: 'New theme', accent: '#0f766e', invoice_title: 'Tax Invoice', credit_title: 'Credit Note', quote_title: 'Quote', po_title: 'Purchase Order', terms: '', payment_instructions: '', payment_advice: true, show_tax_number: true })}><Icon name="plus" size={14} />New</button></div></div>
        <div class="grid gap-3 sm:grid-cols-2">
          <div><label class="label" for="tn2">Theme name</label><input id="tn2" class="input" bind:value={th.name} /></div>
          <div><label class="label" for="ac">Accent colour</label><div class="flex gap-2"><input type="color" class="h-9 w-12 rounded border border-line bg-transparent" bind:value={th.accent} /><input id="ac" class="input" bind:value={th.accent} /></div></div>
          <div><label class="label" for="it">Invoice title</label><input id="it" class="input" bind:value={th.invoice_title} /></div>
          <div><label class="label" for="ct">Credit note title</label><input id="ct" class="input" bind:value={th.credit_title} /></div>
          <div><label class="label" for="qt">Quote title</label><input id="qt" class="input" bind:value={th.quote_title} /></div>
          <div><label class="label" for="pt">Purchase order title</label><input id="pt" class="input" bind:value={th.po_title} /></div>
          <div class="sm:col-span-2"><label class="label" for="pi">Payment instructions</label><textarea id="pi" rows="3" class="textarea" bind:value={th.payment_instructions} placeholder="e.g. BSB 062-000 Account 1234 5678"></textarea></div>
          <div class="sm:col-span-2"><label class="label" for="tt">Terms & conditions</label><textarea id="tt" rows="3" class="textarea" bind:value={th.terms}></textarea></div>
          <div class="sm:col-span-2"><span class="label">Logo</span><div class="flex items-center gap-3">
            {#if th.logo_attachment_id}<img src="/download/attachment?id={th.logo_attachment_id}" alt="Logo" class="h-12 max-w-[180px] rounded bg-white object-contain p-1" />{/if}
            <label class="btn btn-sm cursor-pointer">Upload logo<input type="file" accept="image/png,image/jpeg,image/svg+xml" class="hidden" onchange={(e) => { const f = (e.target as HTMLInputElement).files?.[0]; if (f) uploadLogo(f); }} /></label>
            {#if th.logo_attachment_id}<button class="btn btn-sm btn-ghost" onclick={() => (th.logo_attachment_id = null)}>Remove</button>{/if}</div></div>
          <label class="flex items-center gap-2"><input type="checkbox" bind:checked={th.payment_advice} class="accent-[var(--accent)]" />Show payment advice slip</label>
          <label class="flex items-center gap-2"><input type="checkbox" bind:checked={th.is_default} class="accent-[var(--accent)]" />Default theme</label>
        </div>
        <div class="mt-4 flex justify-end gap-2">{#if th.id}<a class="btn" href="/sales/invoices">Preview on an invoice</a>{/if}{#if can('settings:write')}<button class="btn btn-primary" onclick={saveTheme}>Save theme</button>{/if}</div>
      </section>
    {:else if s === 'emails'}
      <section class="card mb-4 p-5">
        <div class="mb-2 flex items-center justify-between"><h2 class="text-lg font-semibold">Invoice reminders</h2>
          <label class="flex items-center gap-2 text-[13px]"><input type="checkbox" checked={st.invoice_reminders_enabled === '1'} onchange={(e) => { st.invoice_reminders_enabled = (e.target as HTMLInputElement).checked ? '1' : '0'; saveSettings(['invoice_reminders_enabled']); }} class="accent-[var(--accent)]" />Send reminders automatically</label></div>
        <p class="mb-3 text-[13px] text-muted">Reminders are queued for unpaid invoices when they are the given number of days overdue (negative = before due date).</p>
        {#each reminders as r}
          <div class="mb-2 flex items-center gap-2"><input type="number" class="input w-24" bind:value={r.days} /><span class="text-muted">days after due date</span>
            <label class="ml-3 flex items-center gap-1 text-[13px]"><input type="checkbox" bind:checked={r.active} class="accent-[var(--accent)]" />On</label>
            <button class="btn btn-sm" onclick={() => saveReminder(r)}>Save</button><button class="btn btn-ghost btn-sm" onclick={() => delReminder(r)} aria-label="Delete"><Icon name="trash" size={14} /></button></div>
        {/each}
        <div class="flex gap-2"><button class="btn btn-sm" onclick={addReminder}><Icon name="plus" size={14} />Add reminder</button><button class="btn btn-sm" onclick={async () => { const r = await rpc('reminders.run'); toast(`${r.queued} reminder(s) queued`); }}>Run reminders now</button></div>
      </section>
      <section class="card mb-4 space-y-3 p-5">
        <h2 class="text-lg font-semibold">Email sending (SMTP)</h2>
        <p class="text-[13px] text-muted">Optional. With SMTP configured, queued emails (invoices, reminders, payslips) are sent automatically every few minutes. Without it, emails are kept in the Outbox where you can download them as .eml files.</p>
        <div class="grid gap-3 sm:grid-cols-3">
          <div><label class="label" for="sh">SMTP host</label><input id="sh" class="input" bind:value={st.smtp_host} placeholder="smtp.example.com" /></div>
          <div><label class="label" for="sp">Port</label><input id="sp" class="input" bind:value={st.smtp_port} placeholder="587" /></div>
          <div><label class="label" for="sf">From address</label><input id="sf" class="input" bind:value={st.smtp_from} /></div>
          <div><label class="label" for="su">Username</label><input id="su" class="input" bind:value={st.smtp_user} /></div>
          <div><label class="label" for="spw">Password</label><input id="spw" type="password" class="input" bind:value={st.smtp_pass} placeholder="(unchanged)" /></div>
          <div><label class="label" for="stl">Security</label><select id="stl" class="select" bind:value={st.smtp_tls}><option value="STARTTLS">STARTTLS (port 587)</option><option value="TLS">TLS (port 465)</option><option value="NONE">None (local relay)</option></select></div>
        </div>
        {#if can('settings:write')}<div class="flex justify-end gap-2">
          <input class="input w-60" placeholder="Send a test to…" bind:value={testTo} /><button class="btn" onclick={testEmail}>Send test</button>
          <button class="btn btn-primary" onclick={() => saveSettings(['smtp_host', 'smtp_port', 'smtp_from', 'smtp_user', 'smtp_tls', ...(st.smtp_pass ? ['smtp_pass'] : [])])}>Save</button></div>{/if}
      </section>
      <section class="card space-y-4 p-5">
        <h2 class="text-lg font-semibold">Email templates</h2>
        <p class="text-[13px] text-muted">Placeholders: [Contact Name] [Contact First Name] [Invoice Number] [Invoice Total] [Amount Due] [Due Date] [Online Invoice Link] [Trading Name] [Currency Code] [Reference]</p>
        {#each templates as t}
          <details class="rounded-lg border border-line p-3"><summary class="cursor-pointer font-medium">{t.name} <span class="text-[12px] text-muted">({t.kind.toLowerCase()})</span></summary>
            <div class="mt-3 space-y-2"><input class="input" bind:value={t.subject} /><textarea rows="8" class="textarea" bind:value={t.body}></textarea>
              <div class="flex justify-end"><button class="btn btn-sm btn-primary" onclick={() => saveTemplate(t)}>Save template</button></div></div></details>
        {/each}
      </section>
    {:else if s === 'outbox'}
      <section class="card overflow-x-auto p-5">
        <div class="mb-3 flex items-center justify-between"><h2 class="text-lg font-semibold">Outbox</h2><button class="btn btn-sm" onclick={runJobs}>Send queued now</button></div>
        <table class="tbl"><thead><tr><th>Created</th><th>To</th><th>Subject</th><th>Status</th><th></th></tr></thead>
          <tbody>{#each outbox as o}<tr><td class="whitespace-nowrap text-muted">{dateTime(o.created_at)}</td><td>{o.to_addr}</td><td>{o.subject}</td><td>{o.status === 'SAVED' ? 'Saved (not sent)' : o.status === 'FAILED' ? 'Failed: ' + (o.error ?? '') : o.status === 'QUEUED' ? 'Queued' : 'Sent ' + dateTime(o.sent_at)}</td>
            <td class="text-right"><a class="btn btn-sm" href="/download/eml?id={o.id}" download><Icon name="download" size={14} />.eml</a></td></tr>
          {:else}<tr><td colspan="5" class="py-8 text-center text-muted">No emails yet.</td></tr>{/each}</tbody></table>
      </section>
    {:else if s === 'payments'}
      <section class="card space-y-3 p-5">
        <h2 class="text-lg font-semibold">Online payments</h2>
        <p class="text-muted">Customers can view invoices online (Invoice → Online invoice). A "Pay now" button is added when a payment gateway plugin is configured. LedgerCraft ships a Stripe reference adapter in test mode; see PLUGINS.md for setup. Card details never pass through LedgerCraft.</p>
        <div class="rounded-lg bg-bg2 p-3 text-[13px] text-muted">No payment gateway configured.</div>
      </section>
    {:else if s === 'connected'}
      <section class="card mb-4 p-5">
        <h2 class="mb-1 text-lg font-semibold">API tokens</h2>
        <p class="mb-3 text-[13px] text-muted">Local REST API at <code class="rounded bg-bg2 px-1">http://127.0.0.1:4180/api/v1/</code> (localhost only). OpenAPI spec: <a class="link" href="/api/v1/openapi.json" target="_blank">/api/v1/openapi.json</a> (needs a token).</p>
        {#if newToken}<div class="mb-3 rounded-lg border border-warn/40 bg-warn-soft p-3 text-[13px]">Copy this token now — it won't be shown again:<br /><code class="select-all font-mono text-text">{newToken}</code></div>{/if}
        {#if can('settings:write')}<div class="mb-3 flex gap-2"><input class="input" placeholder="Token name, e.g. Zapier" bind:value={tokenName} /><button class="btn btn-primary" disabled={!tokenName} onclick={createToken}>Create token</button></div>{/if}
        <table class="tbl text-[13px]"><thead><tr><th>Name</th><th>Prefix</th><th>Created</th><th>Last used</th><th></th></tr></thead>
          <tbody>{#each tokens as t}<tr class={t.revoked ? 'opacity-50' : ''}><td>{t.name}</td><td class="font-mono">{t.prefix}…</td><td>{dateTime(t.created_at)}</td><td>{t.last_used ? dateTime(t.last_used) : 'Never'}</td>
            <td class="text-right">{#if !t.revoked}<button class="btn btn-ghost btn-sm" onclick={() => revoke(t)}>Revoke</button>{:else}Revoked{/if}</td></tr>{/each}</tbody></table>
      </section>
      <section class="card p-5">
        <h2 class="mb-1 text-lg font-semibold">Webhooks</h2>
        <p class="mb-3 text-[13px] text-muted">LedgerCraft POSTs a signed JSON payload when contacts, documents, payments or bank transactions are created via the API.</p>
        {#if can('settings:write')}<div class="mb-3 flex gap-2"><input class="input" placeholder="https://example.com/hook" bind:value={hookUrl} /><button class="btn btn-primary" disabled={!hookUrl} onclick={addHook}>Add webhook</button></div>{/if}
        {#each hooks as h}<div class="flex items-center justify-between border-t border-line py-2 text-[13px]"><span>{h.url}</span><button class="btn btn-ghost btn-sm" onclick={async () => { await rpc('webhooks.delete', { id: h.id }); hooks = await rpc('webhooks.list'); }}>Remove</button></div>{/each}
      </section>
    {:else if s === 'import'}
      <ImportExport />
    {:else if s === 'backup'}
      <section class="card space-y-3 p-5">
        <h2 class="text-lg font-semibold">Backup & data</h2>
        <p class="text-muted">Your organisation is stored in a single file: <code class="rounded bg-bg2 px-1">{session.data?.org?.file}</code> in your LedgerCraft data folder (%APPDATA%\LedgerCraft\orgs).</p>
        <div class="flex flex-wrap gap-2"><a class="btn btn-primary" href="/download/backup" download><Icon name="download" />Download backup (.lcdb)</a><a class="btn" href="/download/archive" download><Icon name="download" />Full export (JSON)</a></div>
        <p class="text-[13px] text-muted">To restore, copy a backup file into the orgs folder and choose it from the organisation menu.</p>
      </section>
    {:else if s === 'profile'}
      <section class="card space-y-3 p-5">
        <h2 class="text-lg font-semibold">Your profile</h2>
        <div>{session.data?.user?.name} · {session.data?.user?.email} · {session.data?.role}</div>
        <div class="grid max-w-md gap-3"><div><label class="label" for="p1">New password</label><input id="p1" type="password" class="input" bind:value={pw.a} /></div><div><label class="label" for="p2">Confirm password</label><input id="p2" type="password" class="input" bind:value={pw.b} /></div></div>
        <div class="flex gap-2"><button class="btn btn-primary" onclick={setPassword}>Set password</button></div>
        <p class="text-[13px] text-muted">Leave blank and save to remove your password (only possible if no one else has a password). Minimum 8 characters.</p>
      </section>
      <section class="card mt-4 space-y-3 p-5">
        <h2 class="text-lg font-semibold">Two-step verification</h2>
        <p class="text-[13px] text-muted">Protect sign-in with a 6-digit code from an authenticator app (Microsoft Authenticator, Google Authenticator, 1Password, Bitwarden…). Requires a password.</p>
        {#if tfa}
          <div class="rounded-lg bg-bg2 p-3 text-[13px]">Add this key to your authenticator app (choose "enter a setup key"): <code class="select-all font-mono text-text">{tfa.secret}</code><div class="mt-1 break-all text-faint">{tfa.uri}</div></div>
          <div class="flex gap-2"><input class="input w-40 num" placeholder="123456" bind:value={tfaCode} maxlength="6" /><button class="btn btn-primary" onclick={confirm2fa}>Verify & turn on</button></div>
        {:else}
          <div class="flex gap-2"><button class="btn" onclick={begin2fa}>Set up two-step verification</button><button class="btn btn-ghost" onclick={disable2fa}>Turn off</button></div>
        {/if}
      </section>
    {/if}
  </div>
</div>

<Modal bind:open={userOpen} title={editUser ? 'Edit user' : 'Invite a user'}>
  <div class="space-y-3">
    <div><label class="label" for="un">Name</label><input id="un" class="input" bind:value={uf.name} /></div>
    <div><label class="label" for="ue">Email</label><input id="ue" class="input" bind:value={uf.email} /></div>
    <div><label class="label" for="ur">Role</label><select id="ur" class="select" bind:value={uf.role}>{#each ROLES as [k, l]}<option value={k}>{l}</option>{/each}</select></div>
    <div><label class="label" for="up">Password {editUser ? '(leave blank to keep)' : '(optional)'}</label><input id="up" type="password" class="input" bind:value={uf.password} /></div>
  </div>
  {#snippet footer()}<button class="btn" onclick={() => (userOpen = false)}>Cancel</button><button class="btn btn-primary" onclick={saveUser}>Save</button>{/snippet}
</Modal>
