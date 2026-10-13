-- v18: landed costs. A freight/duty/insurance bill's cost is added to the stock it brought in.
CREATE TABLE landed_costs (
  id INTEGER PRIMARY KEY,
  source_bill_id INTEGER NOT NULL UNIQUE REFERENCES documents(id),
  method TEXT NOT NULL,          -- VALUE | QTY
  amount INTEGER NOT NULL,
  journal_id INTEGER REFERENCES journals(id),
  date TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE landed_cost_lines (
  landed_cost_id INTEGER NOT NULL REFERENCES landed_costs(id),
  bill_id INTEGER NOT NULL REFERENCES documents(id),
  item_id INTEGER NOT NULL REFERENCES items(id),
  movement_id INTEGER REFERENCES inventory_movements(id),
  amount INTEGER NOT NULL
);
