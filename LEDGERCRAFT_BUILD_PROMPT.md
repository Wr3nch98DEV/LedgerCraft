# Build Prompt: LedgerCraft, an Open-Source Accounting Suite with Xero's Feature Set

> Paste everything below the line into your AI coding agent (Claude Code, Cursor, etc.). It's written to be run as a long, multi-session build. Have the agent work phase by phase and commit after each phase.

---

## ROLE & MISSION

You are a senior full-stack engineer and a chartered accountant. Build **LedgerCraft**, a lightweight, offline-first, open-source desktop accounting application for small businesses and their accountants. It must match the functional scope of Xero (cloud accounting) feature for feature, as listed below. It is part of the "Craft" suite of open-source alternatives to commercial software.

**Clean-room rule:** Reproduce *functionality*, not *identity*. Do not use Xero's name, logo, colours, icons, copy, screenshots, or proprietary code or assets anywhere in the product. Design an original UI. Mentioning "Xero" is allowed only in the import/migration feature ("Import from Xero CSV export") and in docs.

## NON-NEGOTIABLE PRINCIPLES

1. **Correctness first.** Use strict double-entry bookkeeping. Every posted transaction produces journal lines where debits equal credits, enforced in the database by a constraint or trigger, not only in app code. Store money as **integer minor units** (cents) plus a currency code. Never use floats for money. Use banker's rounding only where a tax authority requires it, and document each case.
2. **Immutable ledger.** Posted journals are never edited in place. Edits produce a reversal plus a new entry, and every change is written to an append-only audit log (who, when, before, after).
3. **Lightweight.** Installer under 25 MB, cold start under 1.5 s, idle RAM under 150 MB, and it must stay responsive with 100k+ transactions per organisation.
4. **Offline-first.** Everything works without internet. Network features (exchange rates, email, online payments, bank feeds, updates) are optional and degrade gracefully.
5. **Your data stays yours.** Each organisation is one portable SQLite file. Offer optional at-rest encryption (SQLCipher), one-click backup and restore, and full export to CSV, JSON and PDF.
6. **Accessible and keyboard-driven.** WCAG 2.2 AA, full keyboard navigation, a command palette (Ctrl/Cmd+K), and light and dark themes.

## TECH STACK (use this unless there's a strong, documented reason not to)

- **Shell:** Tauri 2 (Rust backend, system webview). Do not use Electron.
- **Backend/core:** Rust. Accounting engine as a separate crate (`ledger-core`) with no UI dependencies, so it is fully unit-testable.
- **Database:** SQLite via `sqlx` or `rusqlite`, with versioned migrations, WAL mode and FTS5 for global search.
- **Frontend:** SvelteKit (static adapter) + TypeScript + Tailwind. Use a virtualised data grid for large tables.
- **PDF:** generated in Rust (`typst` or `printpdf`) from user-editable templates.
- **Charts:** a lightweight library (uPlot or ECharts with tree-shaking).
- **OCR (receipts/bills):** Tesseract via a bundled sidecar, optional download so it doesn't bloat the base installer.
- **Local API:** an optional embedded REST and webhook server (off by default, localhost only, token auth) so third parties can integrate.
- **Tests:** `cargo test` + property-based tests (`proptest`) for the ledger, Vitest for the UI, Playwright for end-to-end.

## DATA MODEL (minimum)

Organisation, User, Role, Permission, Account (chart of accounts), TaxRate, TaxComponent, Currency, ExchangeRate, Contact, ContactGroup, ContactPerson, Item, InventoryMovement, TrackingCategory, TrackingOption, Invoice (ACCREC), Bill (ACCPAY), CreditNote, Quote, PurchaseOrder, LineItem, Payment, Overpayment, Prepayment, BatchPayment, BankAccount, BankStatementLine, BankRule, BankTransaction (spend/receive), Transfer, Reconciliation, ManualJournal, Journal (posted), JournalLine, FixedAsset, AssetType, DepreciationRun, Budget, BudgetLine, Project, Task, TimeEntry, ExpenseClaim, Receipt, Employee, PayRun, Payslip, PayItem, LeaveType, LeaveRequest, Timesheet, RepeatingTemplate, Attachment, Note/HistoryEntry, AuditLog, LockDate, ReportLayout, EmailTemplate, BrandingTheme, FileInboxItem.

Every document has a status lifecycle: Draft → Submitted for approval → Approved/Awaiting payment → Paid / Voided / Deleted (drafts only). Model this as an explicit state machine with permitted transitions.

## FEATURE SCOPE (build all of this)

### 1. Organisation & setup
- Multiple organisations in one install, with a switcher.
- Setup wizard: business name, country, base currency, financial year end, tax basis (cash/accrual), tax registration, and a country-specific chart-of-accounts template (ship AU, NZ, UK, US, CA, IE and a generic template).
- Conversion balances (opening balances) with comparative periods, plus a conversion date.
- Lock dates: one for all users and a separate one for advisers.
- Financial settings, invoice/bill numbering sequences, and default due-date terms.
- Users and roles: Adviser, Standard, Invoice-only, Read-only, Cashbook, Payroll admin, plus custom permission sets. Local login with Argon2-hashed passwords and optional TOTP 2FA.

### 2. Dashboard
- Configurable widgets: bank account summaries (statement vs. ledger balance, items to reconcile), invoices owed to you, bills to pay, cash in/out chart, account watchlist, total cash, P&L snapshot, expense claims and tasks.
- Short-term cash flow projection (30/60/90 days from due invoices, bills and repeating templates) with what-if adjustments.
- Business snapshot: profitability, efficiency (debtor/creditor days), financial position and an expense breakdown.

### 3. Banking
- Multiple bank, credit card, PayPal-style and cash accounts.
- Statement import: **CSV (with a saved column-mapping wizard), OFX, QFX, QIF, CAMT.053, MT940**. Duplicate detection.
- Optional bank feeds through a plugin interface (an Open Banking/aggregator adapter trait). Ship a mock provider and document how to add GoCardless/Plaid/TrueLayer adapters. Don't hard-code any one provider.
- **Reconciliation screen**: statement line on the left, match or create on the right. Suggested matches (amount, date window, reference, contact fuzzy match), match to multiple transactions, split, partial payments, bank-rule auto-coding, "OK" in bulk, discuss/notes per line, and a reconciliation report with unreconciled-items warnings.
- Bank rules for spend, receive and transfer, with conditions (any/all; contains/equals/starts with; amount ranges) and allocations by percentage or fixed amount across accounts, tax rates and tracking.
- Spend money, receive money, transfers between accounts, overpayments, prepayments, and cash coding (bulk grid coding).
- Find & recode: bulk-edit the account, tax or tracking of historical transactions, respecting lock dates and logged as reversals.
- Batch payments and deposits. Export a batch payment file (ABA for AU, BACS for UK, NACHA for US, SEPA XML for EU).
- Bank reconciliation report and bank statement report.

### 4. Sales (Accounts Receivable)
- Invoices: line items (item, description, qty, unit price, discount %, account, tax rate, tracking ×2), tax-inclusive, exclusive or no tax, multi-currency, attachments, notes and history.
- Quotes: draft → sent → accepted/declined → convert to invoice, with expiry dates.
- Credit notes with allocation to invoices, or refunded.
- Repeating invoices: schedules, auto-approve, auto-email, and placeholders like `[Month]` / `[Year]`.
- Online invoice view: a self-contained HTML/PDF the customer receives. Optional "Pay now" through a payment-gateway plugin interface (ship Stripe as a reference adapter, test mode by default).
- Invoice reminders: scheduled emails at configurable offsets before/after the due date (SMTP settings, editable templates).
- Customer statements (activity and outstanding), sent in bulk.
- Branding themes: logo, fonts, layout, terms, payment advice slip, with a live preview. Multiple themes.
- Sales overview: draft, awaiting approval, awaiting payment and overdue, with aged receivables.
- Apply payments, partial payments, and write-offs.

### 5. Purchases (Accounts Payable)
- Bills with the same line-item model as invoices. Approvals workflow.
- Purchase orders: draft → approved → billed (convert to bill), with delivery address and "copy to bill".
- Supplier credit notes, repeating bills, and batch payments to suppliers.
- **Files inbox / bill capture:** drag-drop or email-forward-to-folder. OCR extracts supplier, date, total, tax and line items into a pre-filled draft bill or receipt for review.
- Purchases overview with aged payables and a "pay by" planner.

### 6. Expense claims
- Employees submit receipts with photo/PDF, mileage claims (rate per km/mile) and categories. Approval → payment (reimbursement) → posting.

### 7. Contacts
- Customers/suppliers (a contact can be both), multiple contact people, addresses, tax numbers, default accounts/tax/currency/payment terms, groups, notes, history, and a full transaction timeline per contact.
- Merge duplicates, archive, and import/export CSV.

### 8. Products & services / inventory
- Untracked items (sell/buy price, accounts, tax) and **tracked inventory** (quantity on hand, average cost, COGS posted automatically on sale, inventory adjustments, stock valuation report, low-stock alerts).

### 9. Fixed assets
- Asset register (draft → registered → disposed), asset types with defaults, and depreciation methods: straight line, diminishing value (100%/150%/200%), full depreciation at purchase, none.
- Tax vs. book depreciation, monthly depreciation runs (rollback possible), disposal with gain/loss calculation, and a fixed asset reconciliation report.

### 10. Projects & time
- Projects with estimates, tasks (hourly/fixed/non-chargeable), time entries (timer + manual), costs/expenses, invoicing from time and costs, and project profitability reports.

### 11. Payroll (pluggable by jurisdiction)
- Employees, pay templates, pay calendars (weekly/fortnightly/monthly), earnings/deductions/reimbursements/superannuation-pension/leave pay items, leave balances and requests, timesheets, pay runs → payslips (PDF/email), and payroll journals posted to the ledger.
- Implement a `PayrollJurisdiction` trait. Ship a **generic configurable** implementation (flat or bracket-based tax tables loaded from a versioned data file) and stubs for AU (STP/PAYG/super), UK (PAYE/RTI/NI/auto-enrolment), NZ (PAYE/KiwiSaver) and US (federal + state withholding).
- Clearly label statutory filing (STP, RTI, etc.) as **"requires government developer registration — export file only"**. Generate the filing payloads/files but never claim certified submission.

### 12. Tax
- Tax rates with compound components. Cash vs. accrual tax basis. Reverse charge, zero-rated, exempt, out-of-scope and import tax.
- Returns per jurisdiction behind a `TaxReturn` trait: **AU BAS/IAS, NZ GST101, UK VAT100 (MTD-format export), IE VAT3, CA GST/HST, US sales-tax liability report**, and a generic VAT/GST return.
- Return workflow: draft → filed (locks the period) with an audit trail. Include a tax transaction drill-down.

### 13. Multi-currency
- 160+ ISO currencies, with daily exchange rates from an optional free source (e.g. ECB) or manual entry.
- Foreign-currency invoices, bills and bank accounts. Realised FX gains/losses on payment, unrealised revaluation at period end, and FX reports.

### 14. Accounting & adviser tools
- Chart of accounts: CRUD, archive, account types (asset/liability/equity/revenue/expense subtypes), system accounts, import/export, bulk tax change.
- Manual journals (with auto-reversing date), repeating journals, journal report.
- Tracking categories (2 active, unlimited options) on all transactions and reports.
- Budgets: overall and per tracking option, monthly figures, import from CSV, budget manager grid, budget variance report.
- Year-end: close period, retained earnings roll, lock dates.
- Assurance dashboard (audit log viewer, unusual-activity flags, user activity), **history & notes** on every object.
- **Accountant workpapers lite:** attach notes and files to balance-sheet accounts per period and mark them reviewed.

### 15. Reports (all with date ranges, comparative periods, tracking filters, cash/accrual toggle, drill-down to source transaction, export to PDF/XLSX/CSV, and saved/custom layouts)
- Profit & Loss (with % of income, budget comparison, monthly columns)
- Balance Sheet
- Cash Summary / Statement of Cash Flows (direct and indirect)
- Trial Balance
- General Ledger (summary + detail)
- Account Transactions
- Journal Report
- Aged Receivables (summary/detail), Aged Payables (summary/detail)
- Receivable/Payable invoice detail, Customer/Supplier statements
- Sales by Item, Sales by Contact, Purchases by Item, Inventory Item Summary/Details
- Bank Reconciliation, Bank Summary
- Budget Summary and Budget Variance
- Executive Summary, Business Performance (KPIs)
- Tax/GST/VAT Reconciliation, Tax Audit Report
- Fixed Asset Register / Reconciliation / Depreciation Schedule
- Project profitability, Expense claims detail, Payroll activity, Leave balances
- Foreign currency gains & losses
- Contact transactions, Income/Expense by contact
- **Custom report builder:** drag rows/columns, formulas, sections, text blocks, and saved templates with "publish" to lock a version.
- **Management report pack:** combine several reports, a cover page and commentary into one PDF.

### 16. Search, productivity & UX
- Global full-text search across contacts, invoices, bills, transactions, amounts and references.
- Command palette, keyboard shortcuts, recent items, an undo toast for safe actions, inline validation, and a "+ New" quick-create menu.
- Notifications centre (overdue invoices, items to reconcile, approvals waiting, asset depreciation due).

### 17. Import, export & migration
- **Import from Xero CSV exports** (contacts, chart of accounts, items, invoices, bills, manual journals, trial balance) and from QuickBooks/MYOB/Wave CSVs where feasible, all with mapping preview and dry-run validation.
- Export everything (CSV/JSON), plus a full-org archive (`.lcraft` = zipped SQLite + attachments).

### 18. Extensibility
- Local REST API mirroring the domain (contacts, invoices, bills, payments, bank transactions, journals, reports), OpenAPI spec auto-generated, webhooks on create/update events, scoped API tokens.
- Plugin interfaces (Rust traits + config) for: bank feeds, payment gateways, payroll jurisdictions, tax returns, exchange-rate sources and OCR.

## ACCOUNTING CORRECTNESS TESTS (must pass in CI)

- Property test: for any random sequence of valid operations, the trial balance always nets to zero, and Assets = Liabilities + Equity (including current-year earnings).
- Invoice posted → AR ↑, Revenue ↑, Tax payable ↑. Payment → Bank ↑, AR ↓. Voids fully reverse.
- Inventory sale posts COGS at average cost. Adjustments post correctly.
- FX: realised gain/loss computed correctly on partial payments, and revaluation reverses on day 1 of the next period.
- Depreciation schedules match hand-calculated fixtures for every method.
- Tax return totals reconcile to the tax report and to the GL tax accounts.
- Lock dates block every write path (UI, API, import, find & recode).
- Golden-file tests: a seeded demo company with a year of activity; P&L, BS, TB and aged reports must match committed expected outputs.
- Round-trip: export to `.lcraft` → import → identical reports.

## DELIVERY PLAN (work in this order; each phase ends runnable, tested and committed)

1. **Foundation:** repo, Tauri shell, `ledger-core` crate, SQLite schema + migrations, money type, journal engine with invariants, audit log, org setup wizard, chart of accounts, users and roles.
2. **Sales & purchases:** contacts, items, invoices, bills, credit notes, payments, quotes, POs, PDF templates and branding, email (SMTP).
3. **Banking:** statement import, reconciliation, bank rules, transfers, spend/receive, cash coding, batch payments.
4. **Core reports:** P&L, BS, TB, GL, aged AR/AP, account transactions, drill-down, export.
5. **Tax & multi-currency:** tax rates and returns, FX rates, realised/unrealised gains.
6. **Advanced accounting:** manual/repeating journals, tracking, budgets, fixed assets, lock dates, year-end, find & recode, inventory.
7. **Workflow:** repeating invoices/bills, reminders, statements, approvals, expense claims, files inbox + OCR, dashboard + cash flow projection.
8. **Projects & payroll:** projects/time, payroll engine + generic jurisdiction + stubs.
9. **Extensibility & migration:** REST API + webhooks, plugin traits + reference adapters, Xero/QBO/MYOB CSV import, custom report builder, report packs.
10. **Polish:** performance profiling with a 100k-transaction fixture, accessibility audit, installer builds (Windows MSI, macOS DMG, Linux AppImage), auto-update (optional), user docs.

At the end of every phase output: what was built, test results, known gaps, and the next phase's plan. Never mark a feature "done" without a passing test exercising it.

## DOCUMENTATION TO PRODUCE

`README.md` (install, screenshots, feature matrix vs. Xero), `ARCHITECTURE.md`, `ACCOUNTING.md` (posting rules for every transaction type, as debit/credit tables), `PLUGINS.md`, `API.md` (generated OpenAPI), `CONTRIBUTING.md`, and an in-app help panel. License: **AGPL-3.0** (or MIT if wider adoption matters more; state the choice).

## DISCLAIMERS TO SHIP IN THE APP

- LedgerCraft is not affiliated with or endorsed by Xero Limited.
- Payroll and tax calculations must be verified by a qualified professional. Statutory e-filing requires separate government registration.

## START NOW

Begin with Phase 1. First, print a short architecture plan and the full SQLite schema for review, then implement.
