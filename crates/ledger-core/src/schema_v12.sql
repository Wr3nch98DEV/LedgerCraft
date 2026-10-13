-- v12: credit holds (MYOB Acumatica / QuickBooks). A customer on hold can't be given new
-- approved invoices; the credit limit (contacts.credit_limit, cents) is checked on approval.
ALTER TABLE contacts ADD COLUMN credit_hold INTEGER NOT NULL DEFAULT 0;
