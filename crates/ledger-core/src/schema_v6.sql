-- v6: statement tick-off reconciliation (QuickBooks "Reconcile").
-- A reconciliation records the statement's ending date and balance; its items are the
-- transactions ticked as cleared. Finishing marks them reconciled.
CREATE TABLE reconciliations (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES accounts(id),
  statement_date TEXT NOT NULL,
  beginning_balance INTEGER NOT NULL DEFAULT 0,
  ending_balance INTEGER NOT NULL DEFAULT 0,
  cleared_total INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'IN_PROGRESS',
  created_at TEXT NOT NULL DEFAULT (datetime('now')),
  finished_at TEXT
);
CREATE INDEX reconciliations_account ON reconciliations(account_id, status);
CREATE TABLE reconciliation_items (
  reconciliation_id INTEGER NOT NULL REFERENCES reconciliations(id),
  kind TEXT NOT NULL,
  target_id INTEGER NOT NULL,
  PRIMARY KEY (reconciliation_id, kind, target_id)
);
