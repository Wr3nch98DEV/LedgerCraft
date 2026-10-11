-- LedgerCraft schema v2: advanced accounting, workflow, projects, payroll, integrations.

CREATE TABLE budgets (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  type TEXT NOT NULL DEFAULT 'OVERALL',     -- OVERALL TRACKING
  tracking_option_id INTEGER REFERENCES tracking_options(id),
  status TEXT NOT NULL DEFAULT 'ACTIVE',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE budget_lines (
  budget_id INTEGER NOT NULL REFERENCES budgets(id) ON DELETE CASCADE,
  account_id INTEGER NOT NULL REFERENCES accounts(id),
  period TEXT NOT NULL,       -- YYYY-MM
  amount INTEGER NOT NULL,    -- debit-positive like the ledger (income budgets are negative)
  PRIMARY KEY (budget_id, account_id, period)
);

CREATE TABLE asset_types (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  asset_account_id INTEGER NOT NULL REFERENCES accounts(id),
  accum_dep_account_id INTEGER NOT NULL REFERENCES accounts(id),
  dep_expense_account_id INTEGER NOT NULL REFERENCES accounts(id),
  method TEXT NOT NULL DEFAULT 'STRAIGHTLINE', -- STRAIGHTLINE DIMINISHING100 DIMINISHING150 DIMINISHING200 FULL NONE
  rate_bp INTEGER,           -- annual rate
  life_months INTEGER,
  averaging TEXT NOT NULL DEFAULT 'DAILY'  -- DAILY FULLMONTH
);

CREATE TABLE fixed_assets (
  id INTEGER PRIMARY KEY,
  number TEXT NOT NULL,
  name TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  asset_type_id INTEGER REFERENCES asset_types(id),
  status TEXT NOT NULL DEFAULT 'DRAFT',   -- DRAFT REGISTERED DISPOSED
  purchase_date TEXT NOT NULL,
  purchase_price INTEGER NOT NULL,
  serial_number TEXT NOT NULL DEFAULT '',
  warranty_expiry TEXT,
  method TEXT NOT NULL DEFAULT 'STRAIGHTLINE',
  rate_bp INTEGER,
  life_months INTEGER,
  residual_value INTEGER NOT NULL DEFAULT 0,
  dep_start_date TEXT,
  opening_accum_dep INTEGER NOT NULL DEFAULT 0,
  accum_dep INTEGER NOT NULL DEFAULT 0,        -- book depreciation to date
  tax_method TEXT,
  tax_rate_bp INTEGER,
  tax_life_months INTEGER,
  tax_accum_dep INTEGER NOT NULL DEFAULT 0,
  last_dep_date TEXT,
  disposal_date TEXT,
  disposal_price INTEGER,
  disposal_journal_id INTEGER REFERENCES journals(id),
  source_document_id INTEGER,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE depreciation_runs (
  id INTEGER PRIMARY KEY,
  period_end TEXT NOT NULL,
  journal_id INTEGER REFERENCES journals(id),
  total INTEGER NOT NULL DEFAULT 0,
  detail TEXT NOT NULL DEFAULT '[]',     -- JSON [{asset_id, amount, tax_amount}]
  status TEXT NOT NULL DEFAULT 'POSTED', -- POSTED ROLLEDBACK
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE fx_revaluations (
  id INTEGER PRIMARY KEY,
  date TEXT NOT NULL,
  journal_id INTEGER REFERENCES journals(id),
  reversal_journal_id INTEGER REFERENCES journals(id),
  detail TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Expense claims (receipts). Approved claims post as a bill to the claimant.
CREATE TABLE receipts (
  id INTEGER PRIMARY KEY,
  user_id INTEGER REFERENCES users(id),
  claimant_contact_id INTEGER NOT NULL REFERENCES contacts(id),
  supplier TEXT NOT NULL DEFAULT '',
  date TEXT NOT NULL,
  reference TEXT NOT NULL DEFAULT '',
  kind TEXT NOT NULL DEFAULT 'RECEIPT',  -- RECEIPT MILEAGE
  distance_e4 INTEGER,
  rate_e2 INTEGER,
  line_amount_type TEXT NOT NULL DEFAULT 'INCLUSIVE',
  subtotal INTEGER NOT NULL DEFAULT 0,
  total_tax INTEGER NOT NULL DEFAULT 0,
  total INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'DRAFT',  -- DRAFT SUBMITTED AUTHORISED PAID DECLINED VOIDED
  document_id INTEGER REFERENCES documents(id),
  project_id INTEGER,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE projects (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  contact_id INTEGER REFERENCES contacts(id),
  deadline TEXT,
  estimate INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'INPROGRESS',  -- INPROGRESS CLOSED
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE project_tasks (
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  charge_type TEXT NOT NULL DEFAULT 'TIME',  -- TIME FIXED NONCHARGEABLE
  rate_e2 INTEGER NOT NULL DEFAULT 0,         -- hourly rate or fixed price
  estimate_minutes INTEGER,
  status TEXT NOT NULL DEFAULT 'ACTIVE'
);
CREATE TABLE time_entries (
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  task_id INTEGER NOT NULL REFERENCES project_tasks(id),
  user_id INTEGER REFERENCES users(id),
  date TEXT NOT NULL,
  minutes INTEGER NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  invoiced_document_id INTEGER REFERENCES documents(id),
  running_since TEXT
);
CREATE TABLE project_costs (
  id INTEGER PRIMARY KEY,
  project_id INTEGER NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  date TEXT NOT NULL,
  description TEXT NOT NULL,
  amount INTEGER NOT NULL,
  chargeable INTEGER NOT NULL DEFAULT 1,
  markup_bp INTEGER NOT NULL DEFAULT 0,
  source_kind TEXT,  -- BILL RECEIPT MANUAL
  source_id INTEGER,
  invoiced_document_id INTEGER REFERENCES documents(id)
);

-- Payroll
CREATE TABLE employees (
  id INTEGER PRIMARY KEY,
  contact_id INTEGER REFERENCES contacts(id),
  first_name TEXT NOT NULL,
  last_name TEXT NOT NULL,
  email TEXT NOT NULL DEFAULT '',
  address TEXT NOT NULL DEFAULT '',
  start_date TEXT NOT NULL,
  termination_date TEXT,
  tax_id TEXT NOT NULL DEFAULT '',
  tax_code TEXT NOT NULL DEFAULT '',           -- jurisdiction specific (e.g. tax-free threshold Y/N, UK tax code)
  employment_type TEXT NOT NULL DEFAULT 'FULLTIME', -- FULLTIME PARTTIME CASUAL CONTRACTOR
  pay_calendar_id INTEGER,
  pay_basis TEXT NOT NULL DEFAULT 'SALARY',    -- SALARY HOURLY
  annual_salary INTEGER NOT NULL DEFAULT 0,
  hourly_rate_e2 INTEGER NOT NULL DEFAULT 0,
  hours_per_week_e2 INTEGER NOT NULL DEFAULT 3800,
  super_rate_bp INTEGER,
  bank_account TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'ACTIVE',
  template TEXT NOT NULL DEFAULT '[]'          -- JSON recurring pay items [{pay_item_id, amount|rate|units}]
);
CREATE TABLE pay_calendars (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  frequency TEXT NOT NULL DEFAULT 'FORTNIGHTLY',  -- WEEKLY FORTNIGHTLY FOURWEEKLY MONTHLY QUARTERLY
  start_date TEXT NOT NULL,
  payment_date_offset INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE pay_items (
  id INTEGER PRIMARY KEY,
  kind TEXT NOT NULL,     -- EARNINGS DEDUCTION REIMBURSEMENT SUPER LEAVE
  name TEXT NOT NULL,
  account_id INTEGER REFERENCES accounts(id),
  liability_account_id INTEGER REFERENCES accounts(id),
  rate_type TEXT NOT NULL DEFAULT 'FIXED',  -- FIXED RATEPERUNIT MULTIPLE PERCENT
  multiplier_bp INTEGER,
  taxable INTEGER NOT NULL DEFAULT 1,
  super_eligible INTEGER NOT NULL DEFAULT 1,
  system INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE leave_types (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  accrual_hours_per_year_e2 INTEGER NOT NULL DEFAULT 0,
  paid INTEGER NOT NULL DEFAULT 1,
  show_on_payslip INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE leave_balances (
  employee_id INTEGER NOT NULL REFERENCES employees(id),
  leave_type_id INTEGER NOT NULL REFERENCES leave_types(id),
  hours_e2 INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (employee_id, leave_type_id)
);
CREATE TABLE leave_requests (
  id INTEGER PRIMARY KEY,
  employee_id INTEGER NOT NULL REFERENCES employees(id),
  leave_type_id INTEGER NOT NULL REFERENCES leave_types(id),
  start_date TEXT NOT NULL,
  end_date TEXT NOT NULL,
  hours_e2 INTEGER NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'PENDING',  -- PENDING APPROVED DECLINED PROCESSED
  payslip_id INTEGER
);
CREATE TABLE timesheets (
  id INTEGER PRIMARY KEY,
  employee_id INTEGER NOT NULL REFERENCES employees(id),
  period_start TEXT NOT NULL,
  period_end TEXT NOT NULL,
  lines TEXT NOT NULL DEFAULT '[]',   -- JSON [{date, pay_item_id, hours_e2}]
  total_hours_e2 INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'DRAFT'  -- DRAFT APPROVED PROCESSED
);
CREATE TABLE pay_runs (
  id INTEGER PRIMARY KEY,
  number TEXT NOT NULL,
  pay_calendar_id INTEGER REFERENCES pay_calendars(id),
  period_start TEXT NOT NULL,
  period_end TEXT NOT NULL,
  payment_date TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'DRAFT',  -- DRAFT POSTED
  wages INTEGER NOT NULL DEFAULT 0,
  tax INTEGER NOT NULL DEFAULT 0,
  super INTEGER NOT NULL DEFAULT 0,
  deductions INTEGER NOT NULL DEFAULT 0,
  reimbursements INTEGER NOT NULL DEFAULT 0,
  net_pay INTEGER NOT NULL DEFAULT 0,
  journal_id INTEGER REFERENCES journals(id),
  filing_status TEXT NOT NULL DEFAULT 'NOT_FILED',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE payslips (
  id INTEGER PRIMARY KEY,
  pay_run_id INTEGER NOT NULL REFERENCES pay_runs(id) ON DELETE CASCADE,
  employee_id INTEGER NOT NULL REFERENCES employees(id),
  gross INTEGER NOT NULL DEFAULT 0,
  tax INTEGER NOT NULL DEFAULT 0,
  super INTEGER NOT NULL DEFAULT 0,
  deductions INTEGER NOT NULL DEFAULT 0,
  reimbursements INTEGER NOT NULL DEFAULT 0,
  net INTEGER NOT NULL DEFAULT 0,
  lines TEXT NOT NULL DEFAULT '[]',     -- JSON [{pay_item_id, name, kind, units_e2, rate_e2, amount}]
  leave TEXT NOT NULL DEFAULT '[]',
  sent INTEGER NOT NULL DEFAULT 0
);

-- Tax returns (BAS, GST101, VAT100 ...)
CREATE TABLE tax_returns (
  id INTEGER PRIMARY KEY,
  jurisdiction TEXT NOT NULL,
  period_start TEXT NOT NULL,
  period_end TEXT NOT NULL,
  basis TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'DRAFT',   -- DRAFT FILED
  data TEXT NOT NULL DEFAULT '{}',
  adjustments TEXT NOT NULL DEFAULT '{}',
  filed_at TEXT,
  journal_id INTEGER REFERENCES journals(id),
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Accountant workpapers lite
CREATE TABLE workpapers (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES accounts(id),
  period_end TEXT NOT NULL,
  note TEXT NOT NULL DEFAULT '',
  reviewed INTEGER NOT NULL DEFAULT 0,
  reviewed_by INTEGER REFERENCES users(id),
  reviewed_at TEXT,
  UNIQUE(account_id, period_end)
);

-- Integrations
CREATE TABLE api_tokens (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  token_hash TEXT NOT NULL,
  prefix TEXT NOT NULL,
  scopes TEXT NOT NULL DEFAULT '[]',
  user_id INTEGER NOT NULL REFERENCES users(id),
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  last_used TEXT,
  revoked INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE webhooks (
  id INTEGER PRIMARY KEY,
  url TEXT NOT NULL,
  events TEXT NOT NULL DEFAULT '["*"]',
  secret TEXT NOT NULL,
  active INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE webhook_deliveries (
  id INTEGER PRIMARY KEY,
  webhook_id INTEGER NOT NULL REFERENCES webhooks(id) ON DELETE CASCADE,
  event TEXT NOT NULL,
  payload TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'PENDING',
  attempts INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Custom reports & report packs
CREATE TABLE report_layouts (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'CUSTOM',  -- CUSTOM SAVED PACK
  base_report TEXT,
  definition TEXT NOT NULL DEFAULT '{}',
  published INTEGER NOT NULL DEFAULT 0,
  published_snapshot TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Files inbox (bill / receipt capture)
CREATE TABLE file_inbox (
  id INTEGER PRIMARY KEY,
  attachment_id INTEGER NOT NULL REFERENCES attachments(id),
  status TEXT NOT NULL DEFAULT 'NEW',  -- NEW PROCESSED ARCHIVED
  extracted TEXT NOT NULL DEFAULT '{}',
  document_id INTEGER REFERENCES documents(id),
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
