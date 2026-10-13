-- v14: inventory locations (warehouses / sites). Stock movements and documents name a location;
-- NULL means the main location. Cost stays one average per item across locations.
CREATE TABLE locations (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  address TEXT NOT NULL DEFAULT '',
  status TEXT NOT NULL DEFAULT 'ACTIVE',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
ALTER TABLE inventory_movements ADD COLUMN location_id INTEGER REFERENCES locations(id);
ALTER TABLE documents ADD COLUMN location_id INTEGER REFERENCES locations(id);
