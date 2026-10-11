# Architecture

```
┌──────────────────────────┐      JSON RPC / REST       ┌──────────────────────────────┐
│ ui/ (Svelte 5 + Tailwind) │ ─────────────────────────▶ │ crates/ledger-server (axum)  │
│  built to ui/build and    │ ◀───────────────────────── │  127.0.0.1 only              │
│  embedded in the binary   │      HTML views / files     │  sessions, permissions       │
└──────────────────────────┘                             └──────────────┬───────────────┘
                                                                         │ &mut Ledger
                                                          ┌──────────────▼───────────────┐
                                                          │ crates/ledger-core            │
                                                          │  accounting engine (no UI)    │
                                                          └──────────────┬───────────────┘
                                                                         │ rusqlite (bundled, FTS5)
                                                          ┌──────────────▼───────────────┐
                                                          │ %APPDATA%/LedgerCraft/orgs/   │
                                                          │   <org>.lcdb (SQLite, WAL)    │
                                                          └──────────────────────────────┘
```

## Decisions and deviations from the original brief

| Brief | What was built | Why |
|---|---|---|
| Tauri 2 desktop shell | Rust server + system browser in **app mode** (`msedge --app=…`) launched by `start.bat` | Same footprint (system WebView), no extra toolchain, every screen testable in a normal browser. A Tauri wrapper can be added later without changing the app. |
| SvelteKit (static adapter) | Svelte 5 + Vite + a 60-line router | SvelteKit 3 moved its config model; plain Svelte keeps the bundle small (~60 KB gzip) and the build simple. |
| PDF via typst/printpdf | Self-contained HTML documents with a print stylesheet; "Print / Save as PDF" | Keeps the binary small and makes templates easy to edit. |
| ECharts / uPlot | Hand-written SVG charts | ~2 KB instead of ~300 KB. |
| SQLCipher at-rest encryption | Not yet | Use BitLocker/FileVault for now (see roadmap). |

## ledger-core modules

| Module | Responsibility |
|---|---|
| `money` | Integer-cents maths, fixed-point parsing, half-away-from-zero and banker's rounding |
| `db`, `schema_v*.sql` | Versioned migrations (append-only) |
| `journal` | **The only way to write the general ledger.** Balanced, lock-date checked, immutable |
| `tax` | Line tax maths (exclusive / inclusive / no tax, compound components) |
| `documents` | Invoices, bills, credit notes, quotes, POs; status state machine; posting |
| `payments` | Payments, refunds, credit allocations, batch payments, realised FX |
| `banking`, `statements` | Spend/receive, transfers, bank rules; statement parsers & reconciliation |
| `reports`, `extra_reports` | All financial reports (accrual and cash basis) |
| `advanced` | Tracking, currencies & revaluation, manual & repeating journals, conversion balances, budgets, find & recode |
| `taxreturns` | `TaxReturnForm` trait + BAS, GST101, VAT100, VAT3, GST/HST, US sales tax, generic |
| `assets` | Fixed asset register, depreciation runs, disposals |
| `payroll` | `PayrollJurisdiction` trait + AU/UK/NZ/US/generic; pay runs, leave, timesheets |
| `expenses`, `projects` | Expense claims, projects & time |
| `importexport`, `adviser` | CSV import/export, batch bank files, attachments, files inbox, workpapers, assurance |
| `ext`, `modules` | RPC dispatch for feature modules, API tokens, webhooks, OpenAPI |

## Ledger invariants (enforced in SQLite, not just in Rust)

* `journal_post_balanced` trigger: a journal can only be marked posted if its lines sum to zero.
* `jl_no_insert_posted`, `jl_no_update`, `jl_no_delete`, `journal_immutable`, `journal_no_delete`: posted journals and their lines can never change. Corrections are reversals.
* `journal_post_lockdate`: nothing can be posted on or before the all-users lock date, regardless of the path (UI, API, import).
* `audit_log_no_update` / `audit_log_no_delete`: the audit log is append-only.

## Money

Amounts are `i64` minor units. Quantities are ×10⁴, unit prices ×10² of the minor unit (4 dp), rates are basis points, FX rates ×10⁸ (1 base = rate foreign). Tax is rounded per line, half away from zero. Base-currency conversions are rounded per line and the control account (AR/AP/bank) takes the balancing amount so every journal balances exactly.

## Performance

SQLite in WAL mode with indexes on journal dates, accounts, sources and documents. Reports aggregate in SQL. The UI lists render up to 500 rows with server-side search; the reconcile screen pages 15 lines at a time.

## Native app (`crates/ledger-app`)

A native window built with iced 0.14 (Elm architecture). `backend.rs` calls the engine in-process through `ledger_server::rpc::handle` on Tokio's blocking pool; `app.rs` holds routes, state and messages; `shell.rs` draws the QuickBooks-style (left sidebar) or Xero-style (top bar) frame; screens (`dashboard`, `lists`, `doc`, `payments`, `banking`, `reports`, `forms`, `settings`, `setup`) keep one screen model and switch only their view and wording (`vocab.rs`) by layout. `theme.rs` holds dark/light design tokens per layout. `ledger-server` is now a library (dispatcher, jobs, rendering) plus the web server binary.
