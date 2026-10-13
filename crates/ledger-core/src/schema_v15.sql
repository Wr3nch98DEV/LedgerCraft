-- v15: lot and serial numbers. An item can be tracked by lot (batch) or serial number; document
-- lines say which lot or serials they move, and stock movements carry the lot/serial.
ALTER TABLE items ADD COLUMN lot_tracking TEXT NOT NULL DEFAULT 'NONE';
ALTER TABLE line_items ADD COLUMN lot TEXT NOT NULL DEFAULT '';
ALTER TABLE inventory_movements ADD COLUMN lot TEXT NOT NULL DEFAULT '';
CREATE TABLE lots (
  item_id INTEGER NOT NULL REFERENCES items(id),
  lot TEXT NOT NULL,
  expiry TEXT,
  PRIMARY KEY (item_id, lot)
);
