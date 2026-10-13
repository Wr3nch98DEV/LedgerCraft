-- v13: price rules (QuickBooks Enterprise advanced pricing, Acumatica sales prices): a percentage
-- change for an item (or every item), for a price level's customers (or everyone), from a
-- minimum quantity, optionally between two dates.
CREATE TABLE price_rules (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  item_id INTEGER REFERENCES items(id),
  price_level_id INTEGER REFERENCES price_levels(id),
  min_qty_e4 INTEGER NOT NULL DEFAULT 0,
  percent_bp INTEGER NOT NULL,
  start_date TEXT,
  end_date TEXT,
  active INTEGER NOT NULL DEFAULT 1,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
