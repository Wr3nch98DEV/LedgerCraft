-- v5: QuickBooks-style workflows.
-- Document subtype distinguishes variants that post like their base type, e.g. a sales receipt
-- ('RECEIPT') is a sales invoice paid in full on the spot.
ALTER TABLE documents ADD COLUMN subtype TEXT NOT NULL DEFAULT '';
