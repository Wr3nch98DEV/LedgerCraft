-- v16: revenue recognition (deferral) schedules. An approved invoice's income is moved to a
-- deferred revenue liability and recognised month by month.
CREATE TABLE revenue_schedules (
  id INTEGER PRIMARY KEY,
  document_id INTEGER NOT NULL REFERENCES documents(id),
  deferral_account_id INTEGER NOT NULL REFERENCES accounts(id),
  start_date TEXT NOT NULL,
  months INTEGER NOT NULL,
  total INTEGER NOT NULL,
  journal_id INTEGER REFERENCES journals(id),
  status TEXT NOT NULL DEFAULT 'ACTIVE',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE TABLE revenue_entries (
  id INTEGER PRIMARY KEY,
  schedule_id INTEGER NOT NULL REFERENCES revenue_schedules(id),
  period INTEGER NOT NULL,
  date TEXT NOT NULL,
  account_id INTEGER NOT NULL REFERENCES accounts(id),
  amount INTEGER NOT NULL,
  journal_id INTEGER REFERENCES journals(id)
);
CREATE INDEX revenue_entries_due ON revenue_entries(journal_id, date);
