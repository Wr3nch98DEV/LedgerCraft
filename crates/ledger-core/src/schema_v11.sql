-- v11: inventory assemblies (QuickBooks Desktop Premier/Enterprise). An assembly item is built
-- from component items; building moves the components' average cost into the assembly.
CREATE TABLE item_components (
  assembly_id INTEGER NOT NULL REFERENCES items(id),
  component_id INTEGER NOT NULL REFERENCES items(id),
  qty_e4 INTEGER NOT NULL,
  PRIMARY KEY (assembly_id, component_id)
);
CREATE TABLE assembly_builds (
  id INTEGER PRIMARY KEY,
  assembly_id INTEGER NOT NULL REFERENCES items(id),
  date TEXT NOT NULL,
  qty_e4 INTEGER NOT NULL,
  cost INTEGER NOT NULL,
  memo TEXT NOT NULL DEFAULT '',
  journal_id INTEGER REFERENCES journals(id),
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
