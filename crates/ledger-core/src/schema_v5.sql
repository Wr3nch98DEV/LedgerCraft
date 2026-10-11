-- v5: QuickBooks-style workflows.
-- Document subtype distinguishes variants that post like their base type, e.g. a sales receipt
-- ('RECEIPT') is a sales invoice paid in full on the spot.
ALTER TABLE documents ADD COLUMN subtype TEXT NOT NULL DEFAULT '';
-- Checks: a spend-money transaction paid by check ('CHECK'), with its check number.
ALTER TABLE bank_transactions ADD COLUMN subtype TEXT NOT NULL DEFAULT '';
ALTER TABLE bank_transactions ADD COLUMN check_number TEXT NOT NULL DEFAULT '';
