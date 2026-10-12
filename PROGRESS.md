# LedgerCraft — overnight build report

> **Update — native app prototype (12 Oct 2026).** `start.bat` now opens the new **native LedgerCraft app** (100% Rust, iced): a **QuickBooks-style** layout by default and a **Xero-style** layout you can switch to from the gear menu (or `start.bat xero`). Both use the same company files. Working natively: dashboard, invoices/estimates/credit memos/sales receipts/bills/POs (with PDF printing and save-and-send), receive payment, pay bills, expenses and checks, bank deposits from Undeposited Funds, transfers, bank feed review + statement upload, tick-off Reconcile, chart of accounts, journal entries, all reports (export to PDF/CSV), payroll runs, tax returns, projects, mileage, company settings and sign-in. Still in the previous version: users, branding/email settings and a few advanced screens — **`start.bat web`**. Status and what's next: see `UI_REWRITE_PLAN.md`.

**Start here:** double-click **`start.bat`**. First run builds everything (a few minutes). Then click **"Just show me the demo company"** to explore 12 months of sample data, or set up your own business.

LedgerCraft opens in its own window in **dark mode**. Toggle light/dark with the sun/moon icon (top right) or **Alt+T**.

---

## What was built

All ten phases of `LEDGERCRAFT_BUILD_PROMPT.md` were worked through, each one built → tested → reviewed → fixed → committed (see `git log`).

| Phase | Status |
|---|---|
| 1. Foundation — Rust accounting engine, SQLite schema & migrations, journal engine with database-enforced invariants, audit log, setup wizard, chart of accounts, users & roles | ✅ |
| 2. Sales & purchases — contacts, items, invoices, bills, credit notes, payments, quotes, POs, printable documents & branding, email templates | ✅ |
| 3. Banking — statement import (CSV/OFX/QFX/QIF/CAMT.053/MT940), reconciliation, bank rules, transfers, spend/receive, cash coding, batch payments | ✅ |
| 4. Core reports — P&L, Balance Sheet, Trial Balance, GL, aged AR/AP, account transactions, drill-down, CSV/Excel/print export | ✅ |
| 5. Tax & multi-currency — tax rates (compound), BAS / GST101 / VAT100 / VAT3 / GST-HST / US sales tax, FX gains, revaluation, ECB rates | ✅ |
| 6. Advanced accounting — manual & repeating journals, tracking, budgets, fixed assets, lock dates, find & recode, inventory | ✅ |
| 7. Workflow — repeating invoices/bills, reminders, statements, approvals, expense claims, files inbox, dashboard, cash flow forecast, business snapshot | ✅ |
| 8. Projects & payroll — projects/time/timer/costs/invoicing; payroll engine (AU/UK/NZ/US/generic), pay runs, payslips, leave, timesheets | ✅ |
| 9. Extensibility & migration — REST API with tokens + OpenAPI, webhooks, plugin traits, Xero/QBO/MYOB/Wave CSV import, custom report builder, report packs | ✅ |
| 10. Polish — 100k-transaction performance test, docs, `start.bat` | ✅ (see limitations) |

### Numbers
* **Release binary: ~6.5 MB** including the whole UI (target was < 25 MB). No installer needed — it's one `.exe`.
* **Tests:** 46 automated tests (Rust engine + UI) including a property test that random sequences of invoices/bills/payments/voids always leave the ledger balanced and AR equal to outstanding invoices. Run with `start.bat test`.
* **Performance:** with 100,000 transactions every main report (P&L, Balance Sheet, Trial Balance, aged AR) runs in under 0.6 s; with 10,000 they're under 50 ms (`crates/ledger-core/tests/perf.rs`).

## Added after the main build (second half of the night)

* **Two-step sign-in (TOTP)** — Settings → Security; works with any authenticator app.
* **Real email sending over SMTP** — Settings → Email (your own mail server or Gmail app password); invoices, statements, reminders and repeating invoices send from the background worker. "Send test email" button included.
* **One-click exchange rates** from the European Central Bank (Accounting → Currencies).
* **Custom report builder & report packs**, **Statement of Cash Flows** (indirect method), **cash-basis Balance Sheet**.
* **Online quotes** — customers can accept or decline a quote from its online link.
* **Logo upload** for branding themes, **cash coding** screen, **undo** toasts after deleting/archiving, budget CSV import/export, contact discounts, expected/planned payment dates feeding the cash flow forecast, reconciliation adjustments (bank fees/rounding).
* **Security hardening:** the server only answers on 127.0.0.1 and rejects other hostnames (blocks DNS-rebinding); session cookie is HttpOnly; downloads and printable views require sign-in once passwords are set.

### Bugs found and fixed while testing
* **Hourly payroll was 100× too high** (weekly-hours conversion) — fixed, with a test.
* Lines marked "No tax" kept the account's default tax rate and would have polluted the BAS — fixed.
* Cash-basis reports dropped opening (conversion) balances, and reversals of journals hidden from cash basis still showed — both fixed; a property test now checks the cash-basis balance sheet balances after thousands of random operations.
* Balance Sheet printed "Total Equity" twice; dates used UTC instead of local time near midnight; straight-line depreciation drifted by cents; default asset lives were wrong; a lock-up when switching organisations; slow imports at 100k rows — all fixed.
* Wide tables pushed pages sideways on narrow windows — fixed.

## Things to try in the demo company

1. **Dashboard** → "Reconcile N items" on Business Everyday — the reconcile screen suggests matching invoices; press **OK**. Bank rules auto-fill the coffee and Uber lines.
2. **Business → Invoices → New invoice** — pick a contact and the item `DES-HR`, then **Approve & email** / **Print**.
3. **Accounting → Reports** — Profit & Loss with monthly columns and budget comparison, Balance Sheet, Aged Receivables (expand a customer), drill into any number.
4. **Accounting → Tax returns** → Prepare return → BAS boxes calculated from the ledger.
5. **Payroll → Pay runs → Add pay run** → Post → payslips.
6. **Projects** → Pinecrest brand refresh → Invoice the unbilled time.
7. **Reports → Custom report** — build your own report with formulas; **New report pack** for a board pack with cover and commentary.
8. **Ctrl+K** anywhere to search or jump to any page.

## Honest limitations (things that are not like Xero yet)

* **Desktop shell:** the app runs as a local server shown in an Edge/Chrome "app" window instead of a Tauri window (same footprint; see ARCHITECTURE.md).
* **Bank feeds:** no live bank connection is bundled (needs a paid aggregator such as Basiq/Plaid/GoCardless). Import statement files instead; a plugin interface exists.
* **Lodging returns / STP / MTD:** figures and export files are produced, but direct submission to the ATO/HMRC/IRD needs government developer registration.
* **Payroll tax tables** are simplified approximations — verify before paying anyone.
* **Online card payments** on invoices need a payment-gateway plugin (not bundled).
* **OCR for photos** needs Tesseract installed (PDF invoices are read without it).
* **Not built:** SQLCipher file encryption, a mobile app, Hubdoc-style email-in address, a webhook retry screen (failed deliveries are retried automatically).
* **Exchange rates:** the free ECB feed covers ~30 major currencies; others are entered by hand.

## Where things are

* Your data: `%APPDATA%\LedgerCraft\orgs\` (one `.lcdb` file per business) — back up from **Settings → Backup & data**.
* Build output: `%LOCALAPPDATA%\LedgerCraft\build\` (kept out of OneDrive on purpose).
* Docs: `README.md` (feature matrix vs Xero), `ARCHITECTURE.md`, `ACCOUNTING.md` (debit/credit rules), `PLUGINS.md`, `API.md`.
