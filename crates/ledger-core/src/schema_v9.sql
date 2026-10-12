-- v9: US sales tax agencies. Each tax component (state, county, city…) is owed to an agency,
-- which has its own filing frequency.
CREATE TABLE tax_agencies (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE COLLATE NOCASE,
  frequency TEXT NOT NULL DEFAULT 'QUARTERLY',
  status TEXT NOT NULL DEFAULT 'ACTIVE',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
ALTER TABLE tax_components ADD COLUMN agency_id INTEGER REFERENCES tax_agencies(id);
