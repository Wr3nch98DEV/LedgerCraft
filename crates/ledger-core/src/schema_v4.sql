-- v4: performance indexes for large organisations
CREATE INDEX IF NOT EXISTS documents_number ON documents(type, number);
CREATE INDEX IF NOT EXISTS documents_date ON documents(date);
CREATE INDEX IF NOT EXISTS banktx_status ON bank_transactions(status, reconciled);
CREATE INDEX IF NOT EXISTS payments_status ON payments(status, date);
CREATE INDEX IF NOT EXISTS credit_alloc_doc ON credit_allocations(document_id);
CREATE INDEX IF NOT EXISTS credit_alloc_cn ON credit_allocations(credit_note_id);
CREATE INDEX IF NOT EXISTS jl_tax ON journal_lines(tax_rate_id);
-- Map (entity, id) -> FTS rowid so index updates don't scan the FTS table.
CREATE TABLE IF NOT EXISTS search_keys (entity TEXT NOT NULL, entity_id INTEGER NOT NULL, fts_rowid INTEGER NOT NULL, PRIMARY KEY (entity, entity_id));
INSERT OR IGNORE INTO search_keys(entity, entity_id, fts_rowid) SELECT entity, entity_id, rowid FROM search_index;
