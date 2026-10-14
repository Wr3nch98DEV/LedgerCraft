# LedgerCraft

**Local desktop accounting software with a choice of familiar layouts.**

LedgerCraft is an open-source alternative to commercial accounting software for small businesses and their advisers. It runs on your own computer and stores your books locally, with one company file per business. It is part of the *Craft* suite of open alternatives to commercial software.

The idea is simple: use the accounting layout you are comfortable with, without having to relearn everything to manage your books in a different program. Several interface styles share the same accounting engine and company data.

> **Work in progress:** LedgerCraft is under active development. Features, screens and workflows are still being refined, and bugs or incomplete behaviour may remain. Start with the demo company, keep backups, and verify results before relying on it for live business accounts. Payroll tax calculations include approximations; statutory filing requires separate arrangements.

## Choose your layout

| Layout | What it feels like |
|---|---|
| **QuickBooks Desktop-style** | Classic menu bar, icon bar, Home workflow, customer/vendor centres and multiple open windows. The default on first launch. |
| **QuickBooks Online-style** | Sidebar navigation, a **+ New** menu and familiar online-accounting terminology. |
| **Xero-style** | Top navigation, bank account dashboard cards, reconciliation and Xero-style terminology. |
| **MYOB Acumatica-style** | Workspaces, record toolbars and On Hold / Release workflows. |
| **Sage Accounting-style** | Top menus for sales, purchases, banking, reporting and accounting. |

Choose a layout during setup or change it in **Settings / Preferences**. The Desktop layout also provides a **View** menu. Switching layouts keeps the same books; it changes the presentation and terminology. Light and dark themes are available (**Alt+T** toggles them).

These are independently developed interface styles inspired by familiar products. LedgerCraft is not affiliated with or endorsed by Intuit, Xero Limited, MYOB or The Sage Group.

## What you can do

The current code includes:

- **Sales and purchases:** customers and suppliers, invoices, quotes/estimates, bills, purchase orders, credit notes, payments and repeating transactions.
- **Banking:** statement imports, reconciliation, bank rules, transfers and batch payment files.
- **Accounting and reports:** double-entry bookkeeping, chart of accounts, journals, opening balances, budgets, profit and loss, balance sheet, trial balance and aged receivables/payables.
- **Business tools:** inventory, fixed assets, multi-currency, projects, time tracking and expense claims.
- **Payroll and tax:** pay runs, payslips, leave and tax return preparation, with jurisdiction-specific limitations.
- **Data and administration:** multiple companies, users and roles, audit history, CSV imports/exports, backups and a local API.

Some integrations need configuration or plugins. Email uses your SMTP server, live bank feeds need a provider/plugin, and government tax or payroll submissions are not automatically lodged. Core bookkeeping is local; optional online services require an internet connection.

## Get started on Windows

The repository currently uses a **build-from-source** launcher.

1. Install [Rust](https://rustup.rs). On Windows, the Rust setup may also require Microsoft C++ build tools and the Windows SDK; follow the toolchain setup prompts.
2. Download this repository using GitHub's **Code → Download ZIP** and extract it, or clone it:

   ```powershell
   git clone https://github.com/Wr3nch98DEV/LedgerCraft.git
   cd LedgerCraft
   ```

3. Double-click **start.bat** in the extracted/cloned folder. From PowerShell, use:

   ```powershell
   .\start.bat
   ```

The launcher builds the native Rust desktop app and opens its own window. The first build can take several minutes and needs internet access to download dependencies. Later builds reuse the compiler cache. Node.js is only needed for the optional web interface.

### Your first company

1. Choose **Just show me the demo company** to explore sample transactions, or create your own company.
2. For a new company, choose your country and preferred layout, then review the company settings, chart of accounts and tax rates.
3. Add customers/suppliers and bank accounts. Enter opening balances if you are bringing existing books across.
4. Create invoices and bills, record payments, and import bank statements to reconcile transactions.
5. Review your reports and make a backup regularly.

Menu names differ between layouts: quotes may be called estimates, and suppliers may be called vendors.

### Launcher commands

Run these from the project folder; in PowerShell, prefix `start.bat` with `.\`.

| Command | Action |
|---|---|
| `start.bat` | Build and open the native app in your saved layout. |
| `start.bat desktop` | Open in the QuickBooks Desktop-style layout. |
| `start.bat online` | Open in the QuickBooks Online-style layout. |
| `start.bat xero` | Open in the Xero-style layout. |
| `start.bat web` | Build and open the optional web interface in a browser app window. |
| `start.bat browser` | Open the web interface in your default browser. |
| `start.bat server` | Run the local web server without opening a window. |
| `start.bat test` | Run the accounting engine and native app tests. |
| `start.bat clean` | Remove generated build output and rebuild. |

Select the MYOB Acumatica-style and Sage Accounting-style layouts inside the app.

## Your data and backups

On Windows, company files are stored in:

```text
%APPDATA%\LedgerCraft\orgs\*.lcdb
```

Each `.lcdb` file is a SQLite company database. The native app and optional web interface use the same data directory. Company data is separate from this source repository.

Use the app's backup controls in **Settings / Preferences → Data and backups**, or the Desktop layout's file menu. Restore a company using **Restore a backup…** on the company selection screen. Keep backup copies in a separate location, especially before updating the program.

## Optional web interface

Install [Node.js LTS](https://nodejs.org) as well as Rust, then run `start.bat web` or `start.bat browser`. The interface is served locally at `http://127.0.0.1:4180`; the server runs on your computer.

For a manual build (including macOS/Linux, with the appropriate platform build dependencies installed):

```sh
cd ui
npm install
npm run build
cd ..
cargo run --release -p ledger-server -- --open
```

To build and launch the native app manually:

```sh
cargo run --release -p ledger-app
```

## Development and project status

LedgerCraft uses **Rust + iced** for the native desktop app, a shared **Rust accounting engine**, **SQLite** company files, and an optional **Svelte + Tailwind** web interface served by **axum**.

The project is still evolving. Progress notes describe development milestones rather than a guarantee that every feature is ready for production use.

- [Progress notes](PROGRESS.md)
- [Next steps and roadmap](NEXT_STEPS_PLAN.md)
- [Native interface development](UI_REWRITE_PLAN.md)
- [Architecture](ARCHITECTURE.md) and [accounting rules](ACCOUNTING.md)
- [Plugins](PLUGINS.md) and [API documentation](API.md)
- [Contributing](CONTRIBUTING.md)

## Licence

AGPL-3.0-or-later. See [LICENSE](LICENSE).
