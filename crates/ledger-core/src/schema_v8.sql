-- v8: US 1099 tracking. Vendors flagged here have their payments totalled on the 1099 report.
ALTER TABLE contacts ADD COLUMN track_1099 INTEGER NOT NULL DEFAULT 0;
