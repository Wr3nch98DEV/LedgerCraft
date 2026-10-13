-- v17: custom fields (QuickBooks Desktop/Online): extra named fields on customers & vendors,
-- items and employees.
CREATE TABLE custom_fields (
  id INTEGER PRIMARY KEY,
  entity TEXT NOT NULL,            -- contact | item | employee
  name TEXT NOT NULL,
  kind TEXT NOT NULL DEFAULT 'TEXT', -- TEXT | NUMBER | DATE | YESNO
  sort INTEGER NOT NULL DEFAULT 0,
  active INTEGER NOT NULL DEFAULT 1,
  UNIQUE (entity, name)
);
CREATE TABLE custom_values (
  field_id INTEGER NOT NULL REFERENCES custom_fields(id),
  entity_id INTEGER NOT NULL,
  value TEXT NOT NULL DEFAULT '',
  PRIMARY KEY (field_id, entity_id)
);
