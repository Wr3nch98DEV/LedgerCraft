# LedgerCraft

**Open-source, offline-first accounting for small businesses and their advisers** — part of the *Craft* suite of open alternatives to commercial software.

LedgerCraft covers the same ground as cloud accounting suites like Xero: invoicing, bills, bank reconciliation, reports, GST/VAT returns, multi-currency, fixed assets, projects, expense claims and payroll. It runs on your own computer as a single small program, and each business is stored in one portable file.

> LedgerCraft is not affiliated with or endorsed by Xero Limited. Payroll and tax calculations must be verified by a qualified professional. Statutory e-filing (BAS lodgement, STP, MTD, RTI…) requires separate government registration; LedgerCraft prepares the figures and export files.

---

## Quick start (Windows)

1. Install **Rust** (<https://rustup.rs>) and **Node.js LTS** (<https://nodejs.org>) — one time only.
2. Double-click **`start.bat`**.

The first build takes a few minutes; after that it starts in seconds. LedgerCraft opens in its own window (Edge/Chrome app mode) at `http://127.0.0.1:4180`.

| Command | What it does |
|---|---|
| `start.bat` | Build if needed and open LedgerCraft in its own window |
| `start.bat browser` | Open in your normal browser instead |
| `start.bat test` | Run the accounting test-suite |
| `start.bat clean` | Wipe build output and rebuild |

On first launch choose **Just show me the demo company** to explore twelve months of realistic data, or create your own organisation (AU, NZ, UK, IE, CA, US or generic VAT/GST).

Your data lives in `%APPDATA%\LedgerCraft\orgs\*.lcdb` (one SQLite file per organisation). **Settings → Backup** downloads a copy at any time.

**Dark mode is the default.** Toggle light/dark from the sun/moon button in the top bar or press **Alt+T**.

macOS / Linux: `cd ui && npm install && npm run build && cd .. && cargo run --release -p ledger-server -- --open`

---

## Feature matrix

✅ implemented · 🟡 implemented with a documented limitation · ⬜ not yet

| Area | Xero feature | LedgerCraft |
|---|---|---|
| **Setup** | Multiple organisations & switcher | ✅ |
| | Country chart of accounts & tax rates (AU, NZ, UK, IE, CA, US, generic) | ✅ |
| | Financial year, tax basis, tax period, default terms | ✅ |
| | Conversion (opening) balances | ✅ |
| | Lock dates (all users / non-advisers) — enforced by the database | ✅ |
| | Users & roles (Adviser, Standard, Invoice only, Read only, Cashbook, Payroll admin, custom) | ✅ |
| | Passwords (Argon2), TOTP two-step authentication | ✅ |
| **Dashboard** | Bank account widgets, invoices owed, bills to pay, cash in/out, watchlist, P&L | ✅ |
| | Short-term cash flow with what-if | ✅ |
| | Business snapshot | ✅ |
| **Sales** | Invoices (items, qty, price, discount, account, tax, 2 tracking categories, inclusive/exclusive/no tax) | ✅ |
| | Approvals workflow (draft → awaiting approval → approved) | ✅ |
| | Quotes (sent / accepted / declined → invoice) | ✅ |
| | Credit notes, allocation, refunds | ✅ |
| | Repeating invoices with [Month]/[Year] placeholders, auto-approve, auto-email | ✅ |
| | Online invoice link, online quote accept/decline | ✅ · card "Pay now" via plugin 🟡 |
| | Email invoices, statements, templates, reminders | ✅ SMTP sending (your mail server) · Outbox · .eml download |
| | Customer statements (outstanding & activity) | ✅ |
| | Branding themes with logo | ✅ |
| | Print / PDF | ✅ (print-to-PDF) |
| **Purchases** | Bills, approvals, supplier credits, repeating bills | ✅ |
| | Purchase orders → bill | ✅ |
| | Batch payments + ABA / BACS / NACHA / SEPA files | ✅ |
| | Files inbox / bill capture | 🟡 PDF text extraction built in; photos need an OCR plugin (Tesseract) |
| | Expense claims (receipts & mileage) | ✅ |
| **Banking** | Bank, credit card, payment-service, cash accounts | ✅ |
| | Statement import: CSV (mapping wizard), OFX, QFX, QIF, CAMT.053, MT940 + duplicate detection | ✅ |
| | Live bank feeds | 🟡 plugin interface (no aggregator bundled) |
| | Reconcile: suggested matches, find & match (many-to-one, partial payments), create, transfer, discuss, OK | ✅ |
| | Bank rules (any/all conditions, fixed & % allocations, transfer rules) | ✅ |
| | Spend / receive money, overpayments, prepayments, transfers | ✅ |
| | Cash coding (bulk) | ✅ |
| | Bank reconciliation report | ✅ |
| **Contacts** | Customers/suppliers, people, groups, defaults, timeline, merge, archive, CSV import/export | ✅ |
| **Products** | Untracked & tracked inventory (average cost, COGS, adjustments, low stock) | ✅ |
| **Accounting** | Chart of accounts (CRUD, archive, bulk tax, import/export) | ✅ |
| | Manual journals (auto-reversing), repeating journals | ✅ |
| | Tracking categories (2 active, unlimited options) | ✅ |
| | Budgets (overall / per tracking option) + budget variance | ✅ |
| | Find & recode | ✅ |
| | Multi-currency (160+ currencies), realised & unrealised FX, revaluation | ✅ (manual rates or one-click ECB reference rates) |
| | Fixed assets (SL, DV 100/150/200%, full, none; book & tax; runs, rollback, disposal) | ✅ |
| | GST/VAT returns: AU BAS, NZ GST101, UK VAT100 (MTD JSON), IE VAT3, CA GST/HST, US sales tax, generic | ✅ (lodgement by you) |
| | Assurance dashboard, audit log, history & notes everywhere | ✅ |
| | Workpapers | ✅ |
| **Reports** | P&L (comparisons, monthly, budget, % of income, tracking, cash/accrual), Balance Sheet (cash/accrual), Statement of Cash Flows, Trial Balance, GL summary & detail, Account Transactions, Journal Report, Aged AR/AP, Cash Summary, Bank Summary, Executive Summary, Sales/Purchases by Item & Contact, Tax Reconciliation, Invoice detail, Contact transactions, Inventory, Fixed asset register/reconciliation/schedule, Project profitability, Expense claims, Payroll activity, Leave balances, FX gains | ✅ |
| | Drill-down, CSV / Excel / print-to-PDF export | ✅ |
| | Custom report builder (layouts, formulas) & report packs | ✅ |
| **Projects** | Projects, tasks (hourly/fixed/non-chargeable), timer, costs with markup, invoice from project, profitability | ✅ |
| **Payroll** | Employees, calendars, pay items, timesheets, leave, pay runs, payslips, journals, net-pay bank transaction | ✅ |
| | Tax tables AU / UK / NZ / US / generic | 🟡 approximations — verify |
| | STP / RTI / IR filing | 🟡 export file only |
| **Platform** | Global search, command palette (Ctrl+K), keyboard shortcuts, notifications | ✅ |
| | Import from Xero / QBO / MYOB / Wave CSV (contacts, accounts, items, invoices, bills, journals, trial balance) with dry run | ✅ |
| | Full export (CSV + JSON archive), backup | ✅ |
| | Local REST API (token auth, OpenAPI), signed webhooks with background delivery | ✅ |
| | Dark & light themes | ✅ |

---

## How it's built

* **ledger-core** (Rust): the accounting engine — double-entry journals with database-enforced invariants, documents, payments, banking, reports, tax returns, payroll, assets, projects. No UI code; ~40 tests including a property test that random sequences of operations always leave the ledger balanced.
* **ledger-server** (Rust, axum): serves the UI and a JSON API on `127.0.0.1` only.
* **ui** (Svelte 5 + Tailwind 4 + Vite): the interface, embedded into the server binary at build time.

See [ARCHITECTURE.md](ARCHITECTURE.md), [ACCOUNTING.md](ACCOUNTING.md) (posting rules), [PLUGINS.md](PLUGINS.md), [API.md](API.md) and [CONTRIBUTING.md](CONTRIBUTING.md).

## Licence

AGPL-3.0-or-later. See [LICENSE](LICENSE).
