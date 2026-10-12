-- v7: QuickBooks-style account detail types (e.g. "Advertising/Promotional" under Expenses).
-- Free text chosen from the layout's list; empty means "use the default for the account type".
ALTER TABLE accounts ADD COLUMN detail_type TEXT NOT NULL DEFAULT '';
