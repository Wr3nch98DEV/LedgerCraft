-- v19: item receipts (QuickBooks Enhanced Inventory Receiving): stock received before the bill
-- arrives, held in Goods Received Not Invoiced until it's billed.
CREATE TABLE item_receipts (
  id INTEGER PRIMARY KEY,
  contact_id INTEGER NOT NULL REFERENCES contacts(id),
  date TEXT NOT NULL,
  reference TEXT NOT NULL DEFAULT '',
  location_id INTEGER REFERENCES locations(id),
  total INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'OPEN',   -- OPEN | VOIDED
  journal_id INTEGER REFERENCES journals(id),
  bill_id INTEGER REFERENCES documents(id),
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE item_receipt_lines (
  receipt_id INTEGER NOT NULL REFERENCES item_receipts(id),
  item_id INTEGER NOT NULL REFERENCES items(id),
  description TEXT NOT NULL DEFAULT '',
  qty_e4 INTEGER NOT NULL,
  unit_e2 INTEGER NOT NULL,
  amount INTEGER NOT NULL,
  lot TEXT NOT NULL DEFAULT ''
);
