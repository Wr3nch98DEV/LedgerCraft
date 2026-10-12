-- v10: price levels (QuickBooks Desktop). A fixed-percentage level raises or lowers item prices
-- for the customers it's assigned to.
CREATE TABLE price_levels (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  percent_bp INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'ACTIVE',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
ALTER TABLE contacts ADD COLUMN price_level_id INTEGER REFERENCES price_levels(id);
