-- v3: payroll employer taxes, project link on documents, default asset types
ALTER TABLE pay_runs ADD COLUMN employer_tax INTEGER NOT NULL DEFAULT 0;
ALTER TABLE documents ADD COLUMN project_id INTEGER REFERENCES projects(id);
CREATE INDEX documents_project ON documents(project_id);
