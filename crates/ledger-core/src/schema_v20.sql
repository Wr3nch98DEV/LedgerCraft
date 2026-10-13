-- v20: approval rules (Acumatica approval maps, Sage/Xero approvals): documents of a type at or
-- above an amount must be approved by someone with a given role.
CREATE TABLE approval_rules (
  id INTEGER PRIMARY KEY,
  doc_type TEXT NOT NULL,
  min_total INTEGER NOT NULL DEFAULT 0,
  approver_role TEXT NOT NULL DEFAULT 'ADVISER',
  active INTEGER NOT NULL DEFAULT 1
);
