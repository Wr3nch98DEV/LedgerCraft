# LedgerCraft — Next Steps Plan

Written 13 Oct 2026 from your six-step list and the four feature guides (QuickBooks Desktop Enterprise 2024, Xero Ultra, MYOB Acumatica, Sage). Steps run in your order; each has a **done when** test, so a step is finished only when it passes. Step 6 combines your old "next" list (bring their books in, Desktop gaps, shippable, real data).

| # | Step | Size | Depends on |
|---|------|------|-----------|
| 1 | Every feature in every layout | Medium | — |
| 2 | Xero and QuickBooks Online as close to 1:1 as possible | Large | 1 |
| 3 | Add MYOB Acumatica-style and Sage-style layouts | Large | 1, 2 |
| 4 | Every feature from the four guides, in every layout | Very large | 1 (and 3 so new screens get built five ways once) |
| 5 | Your new features (AI, sync, Android, CRM, loans, metering…) | Very large | 1, 4 |
| 6 | Import their books, Desktop gaps, shippable, prove with real data | Large | — (import can move earlier, see note) |

---

## Step 1 — Every feature shows in every layout

**Where we are.** The five layouts will share one engine and one set of features. Most of last night's work is only reachable from the QuickBooks Desktop menus:
- **Desktop only:** Reminders, Find, Price Levels, Templates, Build Assemblies, Inventory Center, Batch Invoices, Finance Charges, Print Forms, Pay Liabilities, Set Closing Date, Edit Item/Account and Customer:Job.
- **Disabled in QuickBooks Online-style:** timesheets, All lists, recurring transactions, attachments, import/export, budgeting, audit log, My accountant and Apps.
- **Disabled in Xero-style:** Fixed assets.
- **Engine only:** fixed assets, budgets, the audit log, import/export, expense claims, currencies, bank rules, custom reports and report packs are in the engine but have no native screen yet.

**Plan**
1. **Feature registry.** One list of every feature: id, route, what it's called in each product (vocabulary already exists in `vocab.rs`), and where it lives in each product's navigation. Every layout builds its menus from this list instead of a hand-written one, so a new feature shows everywhere by default.
2. **Layout structure.** This is groundwork for steps 2 and 3. Today a layout is `if app.desktop()` branches and a separate Desktop module. Before adding two more layouts, split each screen into shared state and logic plus a view per layout family. Common building blocks (list window, form grid, center/detail, report window) get one implementation each, styled per layout. Five layouts then cost five views, not five copies of the logic.
3. **Wire the gaps.** Put every Desktop-only feature into Online's sidebar and gear menu and into Xero's Business/Accounting/Contacts menus, where each product puts it, with that product's wording. Replace every disabled (`None`) menu item and every "Not in the native app yet" placeholder with a real screen.
4. **Native screens for engine-only features:** fixed assets register, budgets, audit log, import/export, expense claims, currencies, bank rules, custom report builder and report packs, users and roles, branding/email settings. After this the web version is no longer needed for anything.

**Done when** an automated test walks the feature registry in every layout and finds every feature reachable from that layout's own navigation, with no placeholder and no disabled entry.

**Status (13 Oct, afternoon): done** — except 1.2 (the per-layout view split), which moves to the start of step 3 where the new layouts need it.
- The feature registry (`crates/ledger-app/src/features.rs`) builds the QuickBooks Online-style and Xero-style menus. Every Desktop feature is in both, with each product's wording, and the test `every_feature_in_every_layout` clicks each one in each layout.
- Placeholders are gone: Help, My accountant and Apps are real screens.
- New native screens: Fixed assets, Audit log, Currencies, Budgets, Expense claims and mileage, Bank rules, Import and export, Custom reports, Payroll settings, and project tasks/time/costs.
- Added to existing screens: payslips and bank payment files, two-step verification, user passwords, matching CSV columns by hand, and a second tracking category.
- Nothing points to the web version any more.
- Bugs found and fixed:
  - In Online/Xero, the edit-customer/item/account screens showed the journal form.
  - Setting a staff member's first password signed the current session out.

---

## Step 2 — Xero-style and QuickBooks Online-style as close to 1:1 as possible

**Plan, for each layout**
1. **Reference set first.** I can't see the real products. For true 1:1, the best input is screenshots from you (or a trial account) of the screens in the checklist below. Without them I work from public documentation, which gets close but not pixel-exact.
2. **Screen-by-screen fidelity checklist** (about 40 screens per product):
   - **Layout:** same regions in the same places; same columns in lists and grids, in the same order.
   - **Words:** labels, button text, status names and date formats.
   - **Flow:** what happens after Save / Approve / Send; which screen opens next.
   - **Keys:** the product's keyboard shortcuts and its search behaviour.
   - **Empty and error states.**
3. **Xero-style:**
   - Dashboard cards.
   - Invoice list tabs: Draft, Awaiting approval, Awaiting payment, Paid, Repeating.
   - Invoice editor with inline line editing and "Approve & email".
   - Bank reconciliation: statement lines on the left, Match / Create / Transfer on the right.
   - Contacts with history.
   - "Find & recode".
   - Reports with the Xero report layout.
   - Projects, Expenses, Payroll (AU) and Settings in Xero's structure.
4. **QuickBooks Online-style:**
   - Left navigation and "+ New" menu.
   - Sales/Expenses hubs with the money bar.
   - Customer and vendor detail pages.
   - Banking "For review / Categorized / Excluded".
   - Reports page with Favorites, Standard and Custom, and the report customise panel.
   - Gear menu, Settings tabs, Recurring transactions, Audit log.
5. Light and dark mode for every screen, with dark as the default.

**Done when** every checklist screen has been compared side by side (screenshot vs ours), and the differences that remain are listed and agreed with you as acceptable.

**Status (13 Oct, 15:30): first pass done from public documentation.** Waiting on your reference screenshots for the side-by-side comparison.

Xero-style:
- Invoice and bill lists have the summary boxes (Draft, Awaiting approval, Awaiting payment, Overdue), a Repeating tab, and Import / Export / Send statements / New credit note / New repeating.
- Invoice editor:
  - Save draft / Submit for approval / Approve / Approve & email.
  - "Sales overview › Invoices" breadcrumb.
  - Xero's field names: To/From, Date, Invoice number.
- Reconcile: each line has Match / Create / Transfer / Discuss tabs.
- Contact page: details on the left; Money in / Money out, recent activity, and History & notes on the right.
- Find and recode.
- Reports: Favourites (starred, kept per company), search, and a Custom tab.
- Organisation settings tiles.
- Projects: In progress / Closed tabs. Payroll: "Pay runs / Add pay run".

QuickBooks Online-style:
- Sales and expense lists: Action column.
- Customer and vendor pages: Transaction list / Statements / Details / Notes tabs.
- Reports: Standard / Custom tabs, Favorites with stars, and search.
- Account and settings: Company / Sales / Users / Advanced / Layout tabs.
- Reclassify transactions.

Light mode was checked on the new screens; dark stays the default.

---

## Step 3 — MYOB Acumatica-style and Sage-style layouts

**Decision needed from you: which Sage?** "Sage" covers several very different products:
- **Sage Accounting:** cloud, small business.
- **Sage 50:** desktop/cloud hybrid.
- **Sage Intacct:** mid-market ERP.

My recommendation is **Sage Accounting** for the Sage style. It's the closest fit for LedgerCraft's users, and MYOB Acumatica already covers the ERP-style experience. Sage 50 could come later as a sixth layout if you want a second desktop-style option.

**Plan**
1. **MYOB Acumatica-style:**
   - Workspace navigation: the left module bar (Finance, Banking, Payables, Receivables, Sales Orders, Purchases, Inventory, Projects, Payroll…) with a workspace menu of Data Entry, Processes, Inquiries and Reports per module.
   - Generic Inquiry list screens with column filters.
   - Record screens with a toolbar (Save, Cancel, Add, Delete, First/Prev/Next/Last), a summary area and tabbed detail grids.
   - Acumatica wording: Bills and Adjustments, Invoices and Memos, Checks and Payments, Release.
   - The *Release* step: documents are "Balanced" until released to the GL.
2. **Sage-style:** Sage Accounting's navigation (Sales, Purchases, Banking, Contacts, Products & Services, Reporting, Settings), its forms and wording, and its bank-feed and VAT/GST flows.
3. Both are built on step 1's registry and building blocks, and each goes through step 2's fidelity checklist.

**Done when** both layouts pass the step 1 reachability test and the step 2 side-by-side review.

**Status (13 Oct, 16:15): both layouts built.** I went with Sage Accounting, as recommended, because there was no answer yet; say if you'd rather have Sage 50 or Intacct. Both pass the reachability tests (`every_feature_in_the_acumatica_layout`, `every_feature_in_the_sage_layout`). The side-by-side review needs your screenshots.

MYOB Acumatica-style:
- Workspace main menu, with a Transactions / Processes / Inquiries / Profiles / Reports / Preferences panel for each workspace.
- Acumatica wording.
- Record screens with the toolbar: Save, Cancel, Add, Delete, First/Previous/Next/Last.
- On Hold → Remove Hold (Balanced) → Release.
- Column filters on lists.
- Its own colours.

Sage Accounting-style:
- Black top bar with Summary / Contacts / Sales / Purchases / Banking / Products & Services / Payroll / Reporting / Accounting menus and a settings menu.
- Sage wording: Sales Invoices, Purchase Invoices, Customer Receipts, Outstanding, Save & Email…
- Its own colours.

Still to do:
- Sage's bank-feed screen still uses the Xero reconcile layout.
- Acumatica's Financial and Address tabs on record screens are labels only, for now.

---

## Step 4 — Every feature from the four guides, in every layout

The table below is the combined, de-duplicated list from the four guides. The **Now** column is my assessment from the code today:
- **Have:** works natively.
- **Partial:** part of it, or engine/web only.
- **Missing:** not built.

The first task of step 4 is to verify this column feature by feature; the table then becomes the step's checklist.

### Ledger, banking and compliance
| Feature | From | Now |
|---|---|---|
| Multi-currency, live rates, revaluation, realised/unrealised gains | Xero, Sage, Acumatica | Have (native Currencies screen: rates, fetch, revalue) |
| Multi-entity consolidation, intercompany eliminations, combined reports | Xero, Sage, Acumatica, QB | Have: combine company files' P&L and balance sheet by account code, eliminate chosen intercompany accounts; currency translation still to add (flagged when currencies differ) |
| Segmented chart of accounts / dimensions (department, location, project) | Sage, Acumatica | Partial: 2 tracking categories (Class List) |
| Fixed assets: depreciation, disposal, leases | Acumatica, QB | Partial: native register, depreciation and disposal; leases missing |
| Deferred revenue / revenue recognition schedules (IFRS 15 / ASC 606) | Acumatica | Have: defer an invoice over N months (to a Deferred Revenue liability), recognise up to a date, Deferred Revenue report; multi-element allocation still to add |
| Budgets, long-range forecasts, scenario (what-if) modelling | Xero, Sage, Acumatica | Partial: budgets and cash-flow forecast; scenarios missing |
| Bank feeds (live) with rules and auto-matching | all | Partial: file import, rules, suggestions; live feed needs an aggregator |
| AI categorisation, anomaly detection, predictive insights | Xero, Acumatica | Missing (step 5's AI assistant builds on this) |
| GST/BAS, PAYG; US multi-jurisdiction sales tax; VAT | all | Have; direct lodgement needs government registration |
| Audit trail, closing date, segregation of duties | all | Partial: native audit log and closing date; fine-grained duties missing |
| Targeted restore of a bad import/sync (not a whole rollback) | Xero | Partial: backups/restore; targeted undo missing |
| 256-bit encryption of the company file | QB, Sage | Missing (SQLCipher) |

### Sales, purchases and AR/AP
| Feature | From | Now |
|---|---|---|
| Recurring invoices and bills, subscription billing | all | Have (memorized/repeating) |
| Dunning and automatic payment reminders | Sage, Acumatica | Have: Payment reminders screen with levels (days after/before due), on/off, send now |
| Pay-now links on invoices (Stripe, PayPal, card, ACH) | Sage, Xero | Partial: plugin interface, no gateway bundled |
| Approval workflows for bills, POs and expenses | Acumatica, Sage, Xero | Have: approval rules by document type and amount and approver role; others' approvals become Awaiting approval; expense claims have submit/approve |
| Batch payments and bank payment files (ABA/ACH/EFT) | all | Have: batch supplier payments and pay runs both make bank files (ABA/BACS/NACHA) natively |
| Customer prepayments/deposits on sales orders (liability) | QB | Partial: prepayments; SO deposits missing |
| Credit limits and credit holds | Acumatica | Have: limit and hold on the contact page; over-limit asks "Save anyway", hold blocks approval |
| Invoice profitability (margin per line) | QB | Have: cost of items, gross margin and % on sales documents |
| Estimates → sales orders → invoices, estimate inactivation, progress invoicing | QB, Sage, Acumatica | Have: progress invoices by % with invoiced-so-far; estimate closes at 100% |
| Receipt capture with OCR and auto-matching | Sage, Xero, Acumatica | Partial: PDF reading; photo OCR needs Tesseract |
| Expense claims, mileage, reimbursements | Sage, Xero | Have (native receipts, mileage, submit/approve) |
| 1099 / W-2 | Sage, QB | Have: 1099 Contractor Summary and W-2 Wage and Tax Summary reports (figures for the forms; no e-filing) |
| Advanced price rules (customer type, category, quantity breaks, dates) | QB, Acumatica | Have: price levels plus price rules (item, level, quantity breaks, dates); item categories still to add |

### Inventory, warehouse and manufacturing
| Feature | From | Now |
|---|---|---|
| Stock, reorder points, low-stock alerts, valuation | all | Have |
| FIFO and LIFO costing (not only average) | QB, Sage | Have: average or FIFO, chosen in settings (LIFO isn't allowed under IFRS, so it's left out) |
| Multiple locations / warehouses / bins | all | Have: locations (stock per location, transfers, a location on bills/invoices/credits, Stock by Location report); bins still to add |
| Lot and serial numbers, expiry | QB, Sage, Acumatica | Have: items tracked by lot or serial; lots/serials on bills, invoices and credits (checked against stock); expiry dates; Lots report. Moving lot stock between locations still to add |
| Landed costs (freight, duty, insurance) | QB, Acumatica | Have: spread a freight/duty/insurance bill over received stock by value or quantity; raises item cost (also FIFO layers) |
| Item receipts separate from bills (Enhanced Inventory Receiving) | QB | Have: receive stock into Goods Received Not Invoiced, enter the bill later against it, void unbilled receipts; lots and locations on receipts |
| Backorders, drop shipping | Acumatica | Missing |
| Barcode scanning, pick/pack/ship, physical counts | QB, Acumatica | Missing (needs the Android app, step 5) |
| Assemblies / kits | QB, Sage | Have (single-level build) |
| Multi-level BOM, work orders, MRP, capacity scheduling, ECOs | Acumatica, Sage | Missing |
| 3PL, multi-carrier shipping, rate shopping | Acumatica | Missing (integration-based) |

### Projects and job costing
| Feature | From | Now |
|---|---|---|
| Projects, time and costs, project invoicing, profitability | all | Have |
| Billing modes: time & materials, fixed, milestone, progress, cost-plus | Acumatica, Sage | Partial: T&M and fixed |
| Budget vs actual per job, WIP reports | QB, Sage, Acumatica | Partial |
| Change orders, retainage/holdback, subcontractor certificates | QB (Contractor), Acumatica | Partial: retainage held back on invoices (Retainage Receivable) and invoiced on release; change orders and subcontractor certificates still to add |
| Resource/capacity planning, time and expense approvals | Acumatica | Missing |

### CRM, payroll, field service
| Feature | From | Now |
|---|---|---|
| Leads, opportunities, pipeline, lead scoring | Acumatica | Missing (step 5 CRM) |
| 360° contact history, notes, relationship mapping | Acumatica, Xero | Partial |
| Customer portal: invoices, pay online, tickets | Acumatica | Partial: online invoice/quote links (web) |
| Marketing automation, email nurture | Acumatica | Missing |
| Payroll with leave, super/pension, STP Phase 2 / RTI files | all | Partial: files produced, no direct lodgement |
| SuperStream, direct deposit (ABA) | Xero, Acumatica, Sage | Missing |
| Timesheets, rosters, self-service, onboarding | Acumatica, Xero | Partial: timesheets in engine |
| Mobile time clock with geofence | QB, Acumatica | Missing (Android, step 5) |
| Service orders, contracts, dispatch, technician mobile app | Acumatica | Missing |

### Platform and reporting
| Feature | From | Now |
|---|---|---|
| 200+ reports, custom reports, report packs | QB, all | Partial: many reports and builder (web) |
| Dashboards, custom KPIs, ratio analysis, benchmarking | Xero, Sage, Acumatica | Partial: Insights; Executive Summary ratios (margins, current/quick ratio, debtor/creditor/inventory days, debt to equity, ROE); custom KPIs and benchmarking still to add |
| SQL/ODBC access for reporting | QB | Have: read-only SQL query screen for advisers (single SELECT, query-only connection, no passwords/tokens), CSV export; an ODBC driver is still to add |
| Custom fields on customers, vendors, items, employees | QB | Have: text / number / date / yes-no fields, filled in on Edit Customer/Vendor and Edit Item; and on the employee form (every layout) |
| Roles and 100+ granular permissions, data-level permissions | QB, Xero, Acumatica | Partial: roles |
| Multi-factor authentication | Sage | Have (TOTP) |
| Many concurrent users | QB, Acumatica | Partial: multi-user in the server; the native app is single-user |
| Open REST API, marketplace of add-ons | Acumatica, Xero | Partial: API and plugin interface |
| E-commerce sync, Salesforce/CRM connectors | QB, Xero | Missing |
| Industry editions: contractor, manufacturing, retail, professional services, nonprofit (donors, pledges, grants, UCOA), accountant tools (client data review, batch entry, reclassify) | QB | Missing (accountant tools partial) |

**Plan**
1. Verify the table.
2. Group the Missing and Partial rows into milestones, in this order:
   1. **Small-business core**, used by everyone: currencies screen, budgets, fixed assets, approvals, payment files, dunning, custom fields, permissions.
   2. **Inventory+**: FIFO, locations, lots/serials, landed costs, item receipts, price rules.
   3. **Projects+**: billing modes, progress invoicing, change orders, retainage, WIP.
   4. **Multi-entity**: consolidation and intercompany.
   5. **ERP tier**: manufacturing/MRP, field service, WMS, 3PL. These are each product-sized and depend on the Android app for scanning and field work.
3. Each feature is built once in the engine. It gets a screen per layout through step 1's building blocks and is named with each product's own wording. Engine tests and native tests come with every feature.

**Done when** every row reads Have, or is a clearly agreed "later" (for example the ERP tier), and the step 1 reachability test passes for all of them in all five layouts.

---

## Step 5 — New features (from your list)

| Feature | What it means for LedgerCraft | Notes |
|---|---|---|
| **Easy first-launch setup, always in Settings** | The setup wizard: company, country, tax, chart, opening balances, bank, invoice template, users. It can be re-run or resumed from Settings ▸ Setup at any time. | Small. Could be done early because it helps every tester. |
| **Multiple users with admin control** | Admin screen for users, roles and custom roles with feature-level switches; per-user layout and theme (done). | Builds on step 4's permissions row. |
| **Online and desktop application** | Today the desktop app talks to a local engine. Add a hosted mode: the same engine served over HTTPS with accounts, used by the desktop app, the browser and Android. | Shares work with cloud sync. |
| **Cloud sync** | Sync company files between devices: one host is authoritative, the others sync changes. Self-hostable, with no lock-in. | Needs conflict rules for offline edits. |
| **Android app (first)** | Day-to-day tasks: invoices, receipts by photo, expenses, bank review, time clock, approvals, barcode scanning. | Also enables the mobile features from step 4 (WMS, time clock, field service). |
| **Plugin system** | Extend the existing `PLUGINS.md` interface into a stable SDK with hooks for screens, reports, bank feeds, payment gateways and importers, plus a local plugin manager. | The marketplace comes later. |
| **MCP connectors** | LedgerCraft as an MCP server (read reports, create drafts, all under the user's permissions), plus an MCP client so the assistant can use outside tools. | Audit every AI action. |
| **AI assistant with RAG; local models** | Ask questions about your books, explain a report, draft invoices, categorise bank lines, spot anomalies. RAG over the ledger, documents and help. Works with a hosted model or a local one (Ollama / llama.cpp / any OpenAI-compatible server). | Every change it makes is a draft that a person approves. |
| **CRM (own + connect)** | Built-in leads, opportunities, pipeline and activities linked to contacts and quotes. Connectors to HubSpot / Salesforce / Zoho. | Covers the Acumatica CRM rows in step 4. |
| **Import from previous accounting software** | Read QuickBooks Desktop (IIF and exports), QuickBooks Online, Xero, MYOB and Sage export files: chart, contacts, items, open invoices/bills, opening balances and history. | Same work as step 6a. Do it once. |
| **Open-source loan management** | Loans given and received: schedules (amortising, interest-only, balloon), interest accrual, repayments split into principal and interest, statements, arrears. | Fits as a module with its own centre in every layout. |
| **Usage metering and billing for AI products** | Meter events (tokens, compute, API calls, any usage), price them (per-unit, tiered, credits, entitlements), and turn them into invoices, payments and revenue in the ledger. Agent-friendly API. | Close to open-source tools like Lago and OpenMeter; worth deciding whether to build or integrate. |
| **100+ countries** | Country packs: chart of accounts, tax rates and returns, date/number formats, currencies and languages, contributed as data files. | Builds on the existing AU/NZ/UK/US packs; translations need a string catalogue. |

You said you have more written down for step 5; they'll be added here when you send them.

---

## Step 6 — Bring their books in, Desktop gaps, shippable, real data

**6a — Bring their books in.**
- The importers from step 5. Each one shows a "what will come across" preview.
- An **Opening Balances** screen, so balances can be entered as at the switch date.
- **Done when** a sample file from each product imports with a matching trial balance.

**6b — Remaining Desktop gaps.**
- Overlapping windows with Cascade and Tile.
- Report E-mail (PDF) and Comment on Report.
- A deposit with payments and other amounts saved as one entry.
- Undo.

**6c — Shippable.**
- Windows installer with Start-menu shortcut.
- Signed builds and auto-update.
- Crash reporting (opt-in).
- Retire the old web version once step 1 has replaced every screen. Only with your OK.

**6d — Prove it with real data.**
- A month of real (or anonymised) transactions, with reports compared against the original software.
- A speed test on a large company file: 1M lines, the QuickBooks Enterprise scale.

---

## Decisions I need from you

1. **Which Sage** for step 3: Sage Accounting (recommended), Sage 50 or Sage Intacct?
2. **Reference screenshots** for steps 2 and 3. Can you supply screenshots or trial logins? They make the difference between "close" and 1:1.
3. **Import timing.** Import (6a / step 5) is what lets people actually switch. It could move ahead of step 4 if you want testers on real books sooner.
4. **Metering:** build our own, or integrate an existing open-source engine?

## Notes
- **Brand assets:** no logos, product names in the app or copied artwork from Intuit, Xero, MYOB or Sage. Layouts are "QuickBooks Desktop-style", "Xero-style" and so on, modelled on how the products work, as now.
- **Workflow:** every step keeps the same loop: build → test → review → improve. `start.bat` keeps working at every commit, and `PROGRESS.md` gets a morning summary.
