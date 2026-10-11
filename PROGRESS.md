# LedgerCraft — overnight build report

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
* **Release binary: 4.7 MB** including the whole UI (target was < 25 MB). No installer needed — it's one `.exe`.
* **Tests:** 60+ automated tests including a property test that random sequences of invoices/bills/payments/voids always leave the ledger balanced and AR equal to outstanding invoices. Run with `start.bat test`.
* **Performance:** with 10,000 transactions every report runs in < 50 ms; the 100k figures are in `crates/ledger-core/tests/perf.rs` output.

## Things to try in the demo company

1. **Dashboard** → "Reconcile 7 items" on Business Everyday — the reconcile screen suggests matching invoices; press **OK**. Bank rules auto-fill the coffee and Uber lines.
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
* **Not built:** SQLCipher file encryption, a mobile app, Hubdoc-style email-in address, live webhook retry UI.

## Where things are

* Your data: `%APPDATA%\LedgerCraft\orgs\` (one `.lcdb` file per business) — back up from **Settings → Backup & data**.
* Build output: `%LOCALAPPDATA%\LedgerCraft\build\` (kept out of OneDrive on purpose).
* Docs: `README.md` (feature matrix vs Xero), `ARCHITECTURE.md`, `ACCOUNTING.md` (debit/credit rules), `PLUGINS.md`, `API.md`.
