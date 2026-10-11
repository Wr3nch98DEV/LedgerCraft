-- v5: QuickBooks-style workflows.
-- Document subtype distinguishes variants that post like their base type, e.g. a sales receipt
-- ('RECEIPT') is a sales invoice paid in full on the spot.
ALTER TABLE documents ADD COLUMN subtype TEXT NOT NULL DEFAULT '';
-- Checks: a spend-money transaction paid by check ('CHECK'), with its check number.
ALTER TABLE bank_transactions ADD COLUMN subtype TEXT NOT NULL DEFAULT '';
ALTER TABLE bank_transactions ADD COLUMN check_number TEXT NOT NULL DEFAULT '';
-- Bank deposits: groups of payments moved from Undeposited Funds into a bank account in one entry.
CREATE TABLE deposits (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES accounts(id),
  date TEXT NOT NULL,
  reference TEXT NOT NULL DEFAULT '',
  total INTEGER NOT NULL DEFAULT 0,
  journal_id INTEGER REFERENCES journals(id),
  status TEXT NOT NULL DEFAULT 'AUTHORISED',
  reconciled INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
ALTER TABLE payments ADD COLUMN deposit_id INTEGER REFERENCES deposits(id);
CREATE INDEX payments_deposit ON payments(deposit_id);
