-- LedgerCraft schema v1
-- Conventions: money = INTEGER minor units; qty_e4 = qty * 10^4; unit_e2 = minor units * 10^2;
-- *_bp = basis points; rate_e8 = exchange rate * 10^8 (1 base = rate foreign); dates = 'YYYY-MM-DD'.

CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE users (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  email TEXT NOT NULL UNIQUE,
  role TEXT NOT NULL DEFAULT 'ADVISER',
  permissions TEXT,               -- JSON array for custom roles
  password_hash TEXT,
  totp_secret TEXT,
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE audit_log (
  id INTEGER PRIMARY KEY,
  at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  user_id INTEGER,
  action TEXT NOT NULL,
  entity TEXT NOT NULL,
  entity_id INTEGER,
  before TEXT,
  after TEXT
);
CREATE TRIGGER audit_log_no_update BEFORE UPDATE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit log is append-only'); END;
CREATE TRIGGER audit_log_no_delete BEFORE DELETE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit log is append-only'); END;

CREATE TABLE notes (
  id INTEGER PRIMARY KEY,
  entity TEXT NOT NULL,
  entity_id INTEGER NOT NULL,
  at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  user_id INTEGER,
  kind TEXT NOT NULL DEFAULT 'NOTE',  -- NOTE | HISTORY
  text TEXT NOT NULL
);
CREATE INDEX notes_entity ON notes(entity, entity_id);

CREATE TABLE attachments (
  id INTEGER PRIMARY KEY,
  entity TEXT NOT NULL,
  entity_id INTEGER,
  filename TEXT NOT NULL,
  mime TEXT NOT NULL,
  size INTEGER NOT NULL,
  data BLOB NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX attachments_entity ON attachments(entity, entity_id);

CREATE TABLE currencies (
  code TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  decimals INTEGER NOT NULL DEFAULT 2,
  enabled INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE exchange_rates (
  currency TEXT NOT NULL,
  date TEXT NOT NULL,
  rate_e8 INTEGER NOT NULL,
  source TEXT NOT NULL DEFAULT 'MANUAL',
  PRIMARY KEY (currency, date)
);

CREATE TABLE tax_rates (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  report_type TEXT NOT NULL,   -- OUTPUT, INPUT, CAPEXINPUT, EXEMPTOUTPUT, EXEMPTINPUT, ZERORATED, BASEXCLUDED, GSTONIMPORTS, REVERSECHARGE, ECOUTPUT, ECINPUT, NONE
  rate_bp INTEGER NOT NULL DEFAULT 0, -- effective rate
  status TEXT NOT NULL DEFAULT 'ACTIVE',
  system INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE tax_components (
  id INTEGER PRIMARY KEY,
  tax_rate_id INTEGER NOT NULL REFERENCES tax_rates(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  rate_bp INTEGER NOT NULL,
  compound INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE accounts (
  id INTEGER PRIMARY KEY,
  code TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  type TEXT NOT NULL,   -- BANK CURRENT FIXED NONCURRENT INVENTORY PREPAYMENT CURRLIAB LIABILITY TERMLIAB EQUITY REVENUE SALES OTHERINCOME EXPENSE DIRECTCOSTS OVERHEADS DEPRECIATN
  class TEXT NOT NULL,  -- ASSET LIABILITY EQUITY REVENUE EXPENSE
  tax_rate_id INTEGER REFERENCES tax_rates(id),
  description TEXT NOT NULL DEFAULT '',
  system_account TEXT UNIQUE, -- AR AP GST RETAINED CURRENTEARNINGS ROUNDING FXGAIN UNREALISEDFX HISTORICAL SUSPENSE WAGESPAYABLE PAYGWITHHOLDING SUPERPAYABLE EXPENSECLAIMS INVENTORY
  status TEXT NOT NULL DEFAULT 'ACTIVE',
  enable_payments INTEGER NOT NULL DEFAULT 0,
  show_in_expense_claims INTEGER NOT NULL DEFAULT 0,
  dashboard_watch INTEGER NOT NULL DEFAULT 0,
  bank_type TEXT,          -- BANK CREDITCARD PAYPAL CASH (only for type BANK)
  bank_account_number TEXT,
  currency TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE tracking_categories (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'ACTIVE'
);
CREATE TABLE tracking_options (
  id INTEGER PRIMARY KEY,
  category_id INTEGER NOT NULL REFERENCES tracking_categories(id),
  name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'ACTIVE'
);

CREATE TABLE contacts (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  first_name TEXT NOT NULL DEFAULT '',
  last_name TEXT NOT NULL DEFAULT '',
  email TEXT NOT NULL DEFAULT '',
  phone TEXT NOT NULL DEFAULT '',
  mobile TEXT NOT NULL DEFAULT '',
  website TEXT NOT NULL DEFAULT '',
  tax_number TEXT NOT NULL DEFAULT '',
  account_number TEXT NOT NULL DEFAULT '',
  bank_account TEXT NOT NULL DEFAULT '',
  postal_address TEXT NOT NULL DEFAULT '',
  street_address TEXT NOT NULL DEFAULT '',
  is_customer INTEGER NOT NULL DEFAULT 0,
  is_supplier INTEGER NOT NULL DEFAULT 0,
  is_employee INTEGER NOT NULL DEFAULT 0,
  default_currency TEXT,
  sales_account_id INTEGER REFERENCES accounts(id),
  purchase_account_id INTEGER REFERENCES accounts(id),
  sales_tax_rate_id INTEGER REFERENCES tax_rates(id),
  purchase_tax_rate_id INTEGER REFERENCES tax_rates(id),
  sales_terms_type TEXT,     -- DAYSAFTERBILLDATE DAYSAFTERBILLMONTH OFCURRENTMONTH OFFOLLOWINGMONTH
  sales_terms_days INTEGER,
  bill_terms_type TEXT,
  bill_terms_days INTEGER,
  discount_bp INTEGER NOT NULL DEFAULT 0,
  credit_limit INTEGER,
  notes TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'ACTIVE',   -- ACTIVE ARCHIVED
  merged_into INTEGER,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX contacts_name ON contacts(name);

CREATE TABLE contact_people (
  id INTEGER PRIMARY KEY,
  contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
  first_name TEXT NOT NULL DEFAULT '',
  last_name TEXT NOT NULL DEFAULT '',
  email TEXT NOT NULL DEFAULT '',
  include_in_emails INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE contact_groups (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE);
CREATE TABLE contact_group_members (
  group_id INTEGER NOT NULL REFERENCES contact_groups(id) ON DELETE CASCADE,
  contact_id INTEGER NOT NULL REFERENCES contacts(id) ON DELETE CASCADE,
  PRIMARY KEY (group_id, contact_id)
);

CREATE TABLE items (
  id INTEGER PRIMARY KEY,
  code TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  sales_description TEXT NOT NULL DEFAULT '',
  purchase_description TEXT NOT NULL DEFAULT '',
  is_sold INTEGER NOT NULL DEFAULT 1,
  is_purchased INTEGER NOT NULL DEFAULT 1,
  sale_unit_e2 INTEGER,
  sale_account_id INTEGER REFERENCES accounts(id),
  sale_tax_rate_id INTEGER REFERENCES tax_rates(id),
  purchase_unit_e2 INTEGER,
  purchase_account_id INTEGER REFERENCES accounts(id), -- COGS account when tracked
  purchase_tax_rate_id INTEGER REFERENCES tax_rates(id),
  is_tracked INTEGER NOT NULL DEFAULT 0,
  inventory_account_id INTEGER REFERENCES accounts(id),
  qty_on_hand_e4 INTEGER NOT NULL DEFAULT 0,
  total_cost INTEGER NOT NULL DEFAULT 0,       -- base currency value of stock on hand
  reorder_level_e4 INTEGER,
  status TEXT NOT NULL DEFAULT 'ACTIVE'
);

CREATE TABLE inventory_movements (
  id INTEGER PRIMARY KEY,
  item_id INTEGER NOT NULL REFERENCES items(id),
  date TEXT NOT NULL,
  qty_e4 INTEGER NOT NULL,
  cost INTEGER NOT NULL,           -- base currency, signed
  source_type TEXT NOT NULL,
  source_id INTEGER,
  journal_id INTEGER
);
CREATE INDEX inv_mov_item ON inventory_movements(item_id, date);

-- Posted journals: the immutable general ledger. Lines are inserted while posted = 0,
-- then the journal is posted in the same transaction; the trigger checks it balances.
CREATE TABLE journals (
  id INTEGER PRIMARY KEY,
  number INTEGER NOT NULL UNIQUE,
  date TEXT NOT NULL,
  source_type TEXT NOT NULL,  -- ACCREC ACCPAY ACCRECCREDIT ACCPAYCREDIT ACCRECPAYMENT ACCPAYPAYMENT CASHREC CASHPAID TRANSFER MANJOURNAL DEPRECIATION PAYRUN INVENTORY CONVERSION FXREVAL EXPCLAIM ...
  source_id INTEGER,
  reference TEXT NOT NULL DEFAULT '',
  narration TEXT NOT NULL DEFAULT '',
  reverses_journal_id INTEGER REFERENCES journals(id),
  reversed INTEGER NOT NULL DEFAULT 0,
  cash_basis INTEGER NOT NULL DEFAULT 1,  -- include in cash-basis reports when no source doc
  posted INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  created_by INTEGER
);
CREATE INDEX journals_date ON journals(date);
CREATE INDEX journals_source ON journals(source_type, source_id);

CREATE TABLE journal_lines (
  id INTEGER PRIMARY KEY,
  journal_id INTEGER NOT NULL REFERENCES journals(id),
  account_id INTEGER NOT NULL REFERENCES accounts(id),
  amount INTEGER NOT NULL,          -- base currency, debit positive, credit negative
  description TEXT NOT NULL DEFAULT '',
  tax_rate_id INTEGER REFERENCES tax_rates(id),
  tax_amount INTEGER NOT NULL DEFAULT 0,  -- base; the tax attached to this net line (for tax reports)
  tax_report_type TEXT,
  contact_id INTEGER REFERENCES contacts(id),
  tracking1 INTEGER REFERENCES tracking_options(id),
  tracking2 INTEGER REFERENCES tracking_options(id),
  currency TEXT,
  fx_amount INTEGER,                -- amount in transaction currency
  item_id INTEGER REFERENCES items(id),
  qty_e4 INTEGER
);
CREATE INDEX jl_journal ON journal_lines(journal_id);
CREATE INDEX jl_account ON journal_lines(account_id);
CREATE INDEX jl_contact ON journal_lines(contact_id);

CREATE TRIGGER jl_no_insert_posted BEFORE INSERT ON journal_lines
WHEN (SELECT posted FROM journals WHERE id = NEW.journal_id) = 1
BEGIN SELECT RAISE(ABORT, 'cannot add lines to a posted journal'); END;
CREATE TRIGGER jl_no_update BEFORE UPDATE ON journal_lines
WHEN (SELECT posted FROM journals WHERE id = OLD.journal_id) = 1
BEGIN SELECT RAISE(ABORT, 'posted journal lines are immutable'); END;
CREATE TRIGGER jl_no_delete BEFORE DELETE ON journal_lines
WHEN (SELECT posted FROM journals WHERE id = OLD.journal_id) = 1
BEGIN SELECT RAISE(ABORT, 'posted journal lines are immutable'); END;
CREATE TRIGGER journal_post_balanced BEFORE UPDATE OF posted ON journals
WHEN NEW.posted = 1 AND OLD.posted = 0
  AND COALESCE((SELECT SUM(amount) FROM journal_lines WHERE journal_id = NEW.id), 0) <> 0
BEGIN SELECT RAISE(ABORT, 'journal does not balance'); END;
CREATE TRIGGER journal_post_lockdate BEFORE UPDATE OF posted ON journals
WHEN NEW.posted = 1 AND OLD.posted = 0
  AND NEW.date <= COALESCE((SELECT value FROM settings WHERE key = 'lock_date_all'), '0000-00-00')
BEGIN SELECT RAISE(ABORT, 'journal date is on or before the lock date'); END;
CREATE TRIGGER journal_immutable BEFORE UPDATE ON journals
WHEN OLD.posted = 1 AND (NEW.date <> OLD.date OR NEW.posted <> OLD.posted OR NEW.source_type <> OLD.source_type)
BEGIN SELECT RAISE(ABORT, 'posted journals are immutable'); END;
CREATE TRIGGER journal_no_delete BEFORE DELETE ON journals
WHEN OLD.posted = 1
BEGIN SELECT RAISE(ABORT, 'posted journals cannot be deleted'); END;

-- Invoices, bills, credit notes, quotes and purchase orders share one table (type).
CREATE TABLE documents (
  id INTEGER PRIMARY KEY,
  type TEXT NOT NULL,         -- ACCREC ACCPAY ACCRECCREDIT ACCPAYCREDIT QUOTE PO
  number TEXT NOT NULL DEFAULT '',
  reference TEXT NOT NULL DEFAULT '',
  contact_id INTEGER NOT NULL REFERENCES contacts(id),
  date TEXT NOT NULL,
  due_date TEXT,
  expiry_date TEXT,
  delivery_date TEXT,
  delivery_address TEXT NOT NULL DEFAULT '',
  attention TEXT NOT NULL DEFAULT '',
  title TEXT NOT NULL DEFAULT '',
  summary TEXT NOT NULL DEFAULT '',
  terms TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'DRAFT',
  line_amount_type TEXT NOT NULL DEFAULT 'EXCLUSIVE',  -- EXCLUSIVE INCLUSIVE NOTAX
  currency TEXT NOT NULL,
  rate_e8 INTEGER NOT NULL DEFAULT 100000000,
  subtotal INTEGER NOT NULL DEFAULT 0,
  total_tax INTEGER NOT NULL DEFAULT 0,
  total INTEGER NOT NULL DEFAULT 0,
  amount_paid INTEGER NOT NULL DEFAULT 0,
  amount_credited INTEGER NOT NULL DEFAULT 0,
  base_total INTEGER NOT NULL DEFAULT 0,
  branding_theme_id INTEGER,
  journal_id INTEGER REFERENCES journals(id),
  void_journal_id INTEGER REFERENCES journals(id),
  source_document_id INTEGER REFERENCES documents(id),
  repeating_id INTEGER,
  sent INTEGER NOT NULL DEFAULT 0,
  expected_payment_date TEXT,
  planned_payment_date TEXT,
  fully_paid_on TEXT,
  online_token TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX documents_type_status ON documents(type, status);
CREATE INDEX documents_contact ON documents(contact_id);

-- Line items for documents, bank transactions, manual journals, repeating templates, expense claims
CREATE TABLE line_items (
  id INTEGER PRIMARY KEY,
  parent_kind TEXT NOT NULL,  -- DOC BANKTX MJ RECEIPT
  parent_id INTEGER NOT NULL,
  sort INTEGER NOT NULL DEFAULT 0,
  item_id INTEGER REFERENCES items(id),
  description TEXT NOT NULL DEFAULT '',
  qty_e4 INTEGER NOT NULL DEFAULT 10000,
  unit_e2 INTEGER NOT NULL DEFAULT 0,
  discount_bp INTEGER NOT NULL DEFAULT 0,
  account_id INTEGER REFERENCES accounts(id),
  tax_rate_id INTEGER REFERENCES tax_rates(id),
  line_amount INTEGER NOT NULL DEFAULT 0, -- as entered per line_amount_type (signed for MJ: debit +)
  net_amount INTEGER NOT NULL DEFAULT 0,
  tax_amount INTEGER NOT NULL DEFAULT 0,
  tracking1 INTEGER REFERENCES tracking_options(id),
  tracking2 INTEGER REFERENCES tracking_options(id)
);
CREATE INDEX line_items_parent ON line_items(parent_kind, parent_id);

CREATE TABLE payments (
  id INTEGER PRIMARY KEY,
  document_id INTEGER NOT NULL REFERENCES documents(id),
  account_id INTEGER NOT NULL REFERENCES accounts(id),
  date TEXT NOT NULL,
  amount INTEGER NOT NULL,          -- in document currency
  rate_e8 INTEGER NOT NULL DEFAULT 100000000,
  base_amount INTEGER NOT NULL,
  control_base INTEGER NOT NULL DEFAULT 0, -- base amount cleared from AR/AP at the document's rate
  reference TEXT NOT NULL DEFAULT '',
  kind TEXT NOT NULL DEFAULT 'PAYMENT', -- PAYMENT REFUND WRITEOFF
  batch_id INTEGER,
  status TEXT NOT NULL DEFAULT 'AUTHORISED',
  journal_id INTEGER REFERENCES journals(id),
  void_journal_id INTEGER REFERENCES journals(id),
  reconciled INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX payments_doc ON payments(document_id);
CREATE INDEX payments_account ON payments(account_id);

CREATE TABLE batch_payments (
  id INTEGER PRIMARY KEY,
  type TEXT NOT NULL,   -- RECBATCH PAYBATCH
  account_id INTEGER NOT NULL REFERENCES accounts(id),
  date TEXT NOT NULL,
  reference TEXT NOT NULL DEFAULT '',
  total INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'AUTHORISED',
  reconciled INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE credit_allocations (
  id INTEGER PRIMARY KEY,
  credit_note_id INTEGER NOT NULL REFERENCES documents(id),
  document_id INTEGER NOT NULL REFERENCES documents(id),
  amount INTEGER NOT NULL,
  cn_base INTEGER NOT NULL DEFAULT 0,
  doc_base INTEGER NOT NULL DEFAULT 0,
  date TEXT NOT NULL,
  journal_id INTEGER REFERENCES journals(id),
  status TEXT NOT NULL DEFAULT 'AUTHORISED'
);

-- Overpayments / prepayments (bank transactions) applied to invoices or bills
CREATE TABLE prepayment_allocations (
  id INTEGER PRIMARY KEY,
  bank_transaction_id INTEGER NOT NULL REFERENCES bank_transactions(id),
  document_id INTEGER NOT NULL REFERENCES documents(id),
  amount INTEGER NOT NULL,
  doc_base INTEGER NOT NULL DEFAULT 0,
  date TEXT NOT NULL,
  journal_id INTEGER REFERENCES journals(id),
  status TEXT NOT NULL DEFAULT 'AUTHORISED'
);

-- Spend / receive money directly against a bank account (incl. overpayments & prepayments)
CREATE TABLE bank_transactions (
  id INTEGER PRIMARY KEY,
  type TEXT NOT NULL,  -- SPEND RECEIVE SPEND-TRANSFER RECEIVE-TRANSFER SPEND-OVERPAYMENT RECEIVE-OVERPAYMENT SPEND-PREPAYMENT RECEIVE-PREPAYMENT
  account_id INTEGER NOT NULL REFERENCES accounts(id),
  contact_id INTEGER REFERENCES contacts(id),
  date TEXT NOT NULL,
  reference TEXT NOT NULL DEFAULT '',
  line_amount_type TEXT NOT NULL DEFAULT 'INCLUSIVE',
  currency TEXT NOT NULL,
  rate_e8 INTEGER NOT NULL DEFAULT 100000000,
  subtotal INTEGER NOT NULL DEFAULT 0,
  total_tax INTEGER NOT NULL DEFAULT 0,
  total INTEGER NOT NULL DEFAULT 0,
  remaining_credit INTEGER NOT NULL DEFAULT 0, -- for overpayments/prepayments
  status TEXT NOT NULL DEFAULT 'AUTHORISED',
  journal_id INTEGER REFERENCES journals(id),
  void_journal_id INTEGER REFERENCES journals(id),
  reconciled INTEGER NOT NULL DEFAULT 0,
  transfer_id INTEGER,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX banktx_account ON bank_transactions(account_id, date);

CREATE TABLE transfers (
  id INTEGER PRIMARY KEY,
  from_account_id INTEGER NOT NULL REFERENCES accounts(id),
  to_account_id INTEGER NOT NULL REFERENCES accounts(id),
  date TEXT NOT NULL,
  amount INTEGER NOT NULL,      -- in from-account currency
  to_amount INTEGER NOT NULL,   -- in to-account currency
  reference TEXT NOT NULL DEFAULT '',
  journal_id INTEGER REFERENCES journals(id),
  from_reconciled INTEGER NOT NULL DEFAULT 0,
  to_reconciled INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'AUTHORISED'
);

CREATE TABLE statement_imports (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES accounts(id),
  filename TEXT NOT NULL,
  format TEXT NOT NULL,
  imported_at TEXT NOT NULL DEFAULT (datetime('now')),
  line_count INTEGER NOT NULL DEFAULT 0,
  start_date TEXT,
  end_date TEXT,
  closing_balance INTEGER
);

CREATE TABLE statement_lines (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES accounts(id),
  import_id INTEGER REFERENCES statement_imports(id),
  date TEXT NOT NULL,
  amount INTEGER NOT NULL,       -- signed: + money in, - money out
  payee TEXT NOT NULL DEFAULT '',
  description TEXT NOT NULL DEFAULT '',
  reference TEXT NOT NULL DEFAULT '',
  txn_type TEXT NOT NULL DEFAULT '',
  balance INTEGER,
  dedupe_key TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'UNRECONCILED', -- UNRECONCILED RECONCILED DELETED
  notes TEXT NOT NULL DEFAULT '',
  reconciled_at TEXT,
  UNIQUE(account_id, dedupe_key)
);
CREATE INDEX stl_account_status ON statement_lines(account_id, status, date);

CREATE TABLE reconciliation_matches (
  id INTEGER PRIMARY KEY,
  statement_line_id INTEGER NOT NULL REFERENCES statement_lines(id),
  kind TEXT NOT NULL,   -- PAYMENT BANKTX TRANSFER_FROM TRANSFER_TO BATCH
  target_id INTEGER NOT NULL,
  amount INTEGER NOT NULL
);
CREATE INDEX recmatch_line ON reconciliation_matches(statement_line_id);

CREATE TABLE bank_rules (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  type TEXT NOT NULL,         -- SPEND RECEIVE TRANSFER
  account_id INTEGER REFERENCES accounts(id),  -- restrict to bank account (NULL = all)
  match_mode TEXT NOT NULL DEFAULT 'ALL',
  conditions TEXT NOT NULL DEFAULT '[]',  -- JSON [{field, op, value}]
  contact_id INTEGER REFERENCES contacts(id),
  reference_field TEXT NOT NULL DEFAULT '',
  fixed_lines TEXT NOT NULL DEFAULT '[]',   -- JSON [{account_id,tax_rate_id,amount,description,tracking1,tracking2}]
  percent_lines TEXT NOT NULL DEFAULT '[]', -- JSON [{account_id,tax_rate_id,percent_bp,...}]
  transfer_account_id INTEGER REFERENCES accounts(id),
  priority INTEGER NOT NULL DEFAULT 0,
  active INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE csv_mappings (
  id INTEGER PRIMARY KEY,
  account_id INTEGER REFERENCES accounts(id),
  name TEXT NOT NULL,
  mapping TEXT NOT NULL
);

CREATE TABLE manual_journals (
  id INTEGER PRIMARY KEY,
  date TEXT NOT NULL,
  narration TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'DRAFT', -- DRAFT POSTED VOIDED
  line_amount_type TEXT NOT NULL DEFAULT 'NOTAX',
  auto_reverse_date TEXT,
  show_on_cash_basis INTEGER NOT NULL DEFAULT 1,
  journal_id INTEGER REFERENCES journals(id),
  reversal_journal_id INTEGER REFERENCES journals(id),
  void_journal_id INTEGER REFERENCES journals(id),
  repeating_id INTEGER,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE repeating_templates (
  id INTEGER PRIMARY KEY,
  kind TEXT NOT NULL,  -- ACCREC ACCPAY MJ
  template TEXT NOT NULL,  -- JSON of the document/journal input
  period_unit TEXT NOT NULL DEFAULT 'MONTHLY', -- WEEKLY MONTHLY
  period_count INTEGER NOT NULL DEFAULT 1,
  start_date TEXT NOT NULL,
  next_date TEXT NOT NULL,
  end_date TEXT,
  due_terms_type TEXT NOT NULL DEFAULT 'DAYSAFTERBILLDATE',
  due_terms_days INTEGER NOT NULL DEFAULT 14,
  save_as TEXT NOT NULL DEFAULT 'DRAFT',  -- DRAFT AUTHORISED AUTHORISED_SEND
  status TEXT NOT NULL DEFAULT 'ACTIVE',
  last_run TEXT
);

CREATE TABLE sequences (name TEXT PRIMARY KEY, prefix TEXT NOT NULL, next INTEGER NOT NULL, width INTEGER NOT NULL DEFAULT 4);

CREATE TABLE branding_themes (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  is_default INTEGER NOT NULL DEFAULT 0,
  logo_attachment_id INTEGER,
  font TEXT NOT NULL DEFAULT 'Inter',
  accent TEXT NOT NULL DEFAULT '#0f766e',
  layout TEXT NOT NULL DEFAULT 'STANDARD',
  terms TEXT NOT NULL DEFAULT '',
  payment_advice INTEGER NOT NULL DEFAULT 1,
  show_tax_number INTEGER NOT NULL DEFAULT 1,
  payment_instructions TEXT NOT NULL DEFAULT '',
  invoice_title TEXT NOT NULL DEFAULT 'Tax Invoice',
  credit_title TEXT NOT NULL DEFAULT 'Credit Note',
  quote_title TEXT NOT NULL DEFAULT 'Quote',
  po_title TEXT NOT NULL DEFAULT 'Purchase Order'
);

CREATE TABLE email_templates (
  id INTEGER PRIMARY KEY,
  kind TEXT NOT NULL, -- INVOICE QUOTE STATEMENT REMINDER PO CREDIT PAYSLIP RECEIPT
  name TEXT NOT NULL,
  subject TEXT NOT NULL,
  body TEXT NOT NULL,
  is_default INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE outbox (
  id INTEGER PRIMARY KEY,
  to_addr TEXT NOT NULL,
  subject TEXT NOT NULL,
  body TEXT NOT NULL,
  entity TEXT,
  entity_id INTEGER,
  status TEXT NOT NULL DEFAULT 'QUEUED', -- QUEUED SENT FAILED SAVED
  error TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  sent_at TEXT
);

CREATE TABLE reminder_rules (
  id INTEGER PRIMARY KEY,
  days INTEGER NOT NULL,     -- negative = before due date
  template_id INTEGER REFERENCES email_templates(id),
  active INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE reminders_sent (document_id INTEGER NOT NULL, rule_id INTEGER NOT NULL, sent_at TEXT NOT NULL, PRIMARY KEY(document_id, rule_id));

CREATE VIRTUAL TABLE search_index USING fts5(entity UNINDEXED, entity_id UNINDEXED, title, body, tokenize = 'unicode61');
