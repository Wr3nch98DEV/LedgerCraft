use crate::{error::invalid, util, Ledger, Result};
use rusqlite::params;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ContactInput {
    pub name: String,
    pub first_name: String,
    pub last_name: String,
    pub email: String,
    pub phone: String,
    pub mobile: String,
    pub website: String,
    pub tax_number: String,
    pub account_number: String,
    pub bank_account: String,
    pub postal_address: String,
    pub street_address: String,
    pub is_customer: Option<bool>,
    pub is_supplier: Option<bool>,
    pub is_employee: Option<bool>,
    pub default_currency: Option<String>,
    pub sales_account_id: Option<i64>,
    pub purchase_account_id: Option<i64>,
    pub sales_tax_rate_id: Option<i64>,
    pub purchase_tax_rate_id: Option<i64>,
    pub sales_terms_type: Option<String>,
    pub sales_terms_days: Option<i64>,
    pub bill_terms_type: Option<String>,
    pub bill_terms_days: Option<i64>,
    pub discount_bp: i64,
    pub credit_limit: Option<i64>,
    pub notes: String,
    pub people: Vec<PersonInput>,
    pub groups: Vec<i64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct PersonInput { pub first_name: String, pub last_name: String, pub email: String, pub include_in_emails: bool }

impl Ledger {
    pub fn list_contacts(&self, filter: &str, q: &str) -> Result<Vec<Value>> {
        let like = format!("%{}%", q.trim());
        util::query_json(&self.conn,
            "SELECT c.id, c.name, c.email, c.phone, c.is_customer, c.is_supplier, c.is_employee, c.status, c.default_currency,
               COALESCE((SELECT SUM(total - amount_paid - amount_credited) FROM documents d WHERE d.contact_id = c.id AND d.type = 'ACCREC' AND d.status = 'AUTHORISED'), 0) AS receivable,
               COALESCE((SELECT SUM(total - amount_paid - amount_credited) FROM documents d WHERE d.contact_id = c.id AND d.type = 'ACCPAY' AND d.status = 'AUTHORISED'), 0) AS payable,
               COALESCE((SELECT SUM(total - amount_paid - amount_credited) FROM documents d WHERE d.contact_id = c.id AND d.type = 'ACCREC' AND d.status = 'AUTHORISED' AND d.due_date < date('now')), 0) AS overdue
             FROM contacts c
             WHERE c.merged_into IS NULL
               AND (?1 = 'all' AND c.status = 'ACTIVE' OR ?1 = 'customers' AND c.is_customer = 1 AND c.status = 'ACTIVE'
                    OR ?1 = 'suppliers' AND c.is_supplier = 1 AND c.status = 'ACTIVE' OR ?1 = 'employees' AND c.is_employee = 1 AND c.status = 'ACTIVE'
                    OR ?1 = 'archived' AND c.status = 'ARCHIVED' OR ?1 = 'everything')
               AND (?2 = '%%' OR c.name LIKE ?2 OR c.email LIKE ?2 OR c.account_number LIKE ?2)
             ORDER BY c.name COLLATE NOCASE", params![filter, like])
    }

    pub fn get_contact(&self, id: i64) -> Result<Value> {
        let mut c = util::query_one_json(&self.conn, "SELECT * FROM contacts WHERE id = ?1", [id])?;
        c["people"] = json!(util::query_json(&self.conn, "SELECT * FROM contact_people WHERE contact_id = ?1 ORDER BY id", [id])?);
        c["groups"] = json!(util::query_json(&self.conn, "SELECT g.id, g.name FROM contact_groups g JOIN contact_group_members m ON m.group_id = g.id WHERE m.contact_id = ?1", [id])?);
        c["summary"] = util::query_one_json(&self.conn,
            "SELECT
               COALESCE(SUM(CASE WHEN type='ACCREC' AND status='AUTHORISED' THEN total-amount_paid-amount_credited END),0) AS receivable,
               COALESCE(SUM(CASE WHEN type='ACCREC' AND status='AUTHORISED' AND due_date < date('now') THEN total-amount_paid-amount_credited END),0) AS receivable_overdue,
               COALESCE(SUM(CASE WHEN type='ACCPAY' AND status='AUTHORISED' THEN total-amount_paid-amount_credited END),0) AS payable,
               COALESCE(SUM(CASE WHEN type='ACCPAY' AND status='AUTHORISED' AND due_date < date('now') THEN total-amount_paid-amount_credited END),0) AS payable_overdue,
               COALESCE(SUM(CASE WHEN type='ACCREC' AND status IN ('AUTHORISED','PAID') AND date >= date('now','-12 months') THEN base_total END),0) AS sales_12m,
               COALESCE(SUM(CASE WHEN type='ACCPAY' AND status IN ('AUTHORISED','PAID') AND date >= date('now','-12 months') THEN base_total END),0) AS purchases_12m
             FROM documents WHERE contact_id = ?1", [id])?;
        Ok(c)
    }

    /// Activity timeline for a contact: documents, payments, bank transactions.
    pub fn contact_activity(&self, id: i64) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT * FROM (
               SELECT 'document' AS kind, d.id, d.type AS subtype, d.number, d.reference, d.date, d.due_date, d.total AS amount, d.currency, d.status,
                 d.total - d.amount_paid - d.amount_credited AS due FROM documents d WHERE d.contact_id = ?1 AND d.status <> 'DELETED'
               UNION ALL
               SELECT 'payment', p.id, d.type, d.number, p.reference, p.date, NULL, p.amount, d.currency, p.status, 0
                 FROM payments p JOIN documents d ON d.id = p.document_id WHERE d.contact_id = ?1
               UNION ALL
               SELECT 'banktx', b.id, b.type, '', b.reference, b.date, NULL, b.total, b.currency, b.status, 0
                 FROM bank_transactions b WHERE b.contact_id = ?1
             ) ORDER BY date DESC, id DESC LIMIT 500", [id])
    }

    pub fn find_or_create_contact(&self, name: &str) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() { return invalid("contact name is required"); }
        if let Ok(id) = self.conn.query_row("SELECT id FROM contacts WHERE name = ?1 COLLATE NOCASE AND merged_into IS NULL", [name], |r| r.get::<_, i64>(0)) {
            return Ok(id);
        }
        self.create_contact(&ContactInput { name: name.into(), ..Default::default() })
    }

    pub fn create_contact(&self, c: &ContactInput) -> Result<i64> {
        if c.name.trim().is_empty() { return invalid("contact name is required"); }
        let dup: i64 = self.conn.query_row("SELECT COUNT(*) FROM contacts WHERE name = ?1 COLLATE NOCASE AND merged_into IS NULL", [c.name.trim()], |r| r.get(0))?;
        if dup > 0 { return invalid(format!("a contact named '{}' already exists", c.name.trim())); }
        self.atomic(|| {
            self.conn.execute("INSERT INTO contacts(name) VALUES (?1)", [c.name.trim()])?;
            let id = self.conn.last_insert_rowid();
            self.write_contact(id, c)?;
            self.audit("CREATE", "contact", Some(id), None, Some(json!({"name": c.name})))?;
            self.add_note("contact", id, "HISTORY", "Contact created")?;
            Ok(id)
        })
    }

    pub fn update_contact(&self, id: i64, c: &ContactInput) -> Result<()> {
        if c.name.trim().is_empty() { return invalid("contact name is required"); }
        let dup: i64 = self.conn.query_row("SELECT COUNT(*) FROM contacts WHERE name = ?1 COLLATE NOCASE AND id <> ?2 AND merged_into IS NULL", params![c.name.trim(), id], |r| r.get(0))?;
        if dup > 0 { return invalid(format!("a contact named '{}' already exists", c.name.trim())); }
        let before = self.get_contact(id)?;
        self.atomic(|| {
            self.write_contact(id, c)?;
            self.audit("UPDATE", "contact", Some(id), Some(before), Some(json!({"name": c.name})))?;
            self.add_note("contact", id, "HISTORY", "Contact details edited")?;
            Ok(())
        })
    }

    fn write_contact(&self, id: i64, c: &ContactInput) -> Result<()> {
        self.conn.execute(
            "UPDATE contacts SET name=?2, first_name=?3, last_name=?4, email=?5, phone=?6, mobile=?7, website=?8, tax_number=?9, account_number=?10,
               bank_account=?11, postal_address=?12, street_address=?13,
               is_customer=COALESCE(?14, is_customer), is_supplier=COALESCE(?15, is_supplier), is_employee=COALESCE(?16, is_employee),
               default_currency=?17, sales_account_id=?18, purchase_account_id=?19, sales_tax_rate_id=?20, purchase_tax_rate_id=?21,
               sales_terms_type=?22, sales_terms_days=?23, bill_terms_type=?24, bill_terms_days=?25, discount_bp=?26, credit_limit=?27, notes=?28
             WHERE id=?1",
            params![id, c.name.trim(), c.first_name, c.last_name, c.email, c.phone, c.mobile, c.website, c.tax_number, c.account_number,
                c.bank_account, c.postal_address, c.street_address, c.is_customer.map(|b| b as i64), c.is_supplier.map(|b| b as i64),
                c.is_employee.map(|b| b as i64), c.default_currency.clone().filter(|s| !s.is_empty()), c.sales_account_id, c.purchase_account_id,
                c.sales_tax_rate_id, c.purchase_tax_rate_id, c.sales_terms_type.clone().filter(|s| !s.is_empty()), c.sales_terms_days,
                c.bill_terms_type.clone().filter(|s| !s.is_empty()), c.bill_terms_days, c.discount_bp, c.credit_limit, c.notes],
        )?;
        self.conn.execute("DELETE FROM contact_people WHERE contact_id = ?1", [id])?;
        for p in &c.people {
            if p.first_name.is_empty() && p.last_name.is_empty() && p.email.is_empty() { continue; }
            self.conn.execute("INSERT INTO contact_people(contact_id, first_name, last_name, email, include_in_emails) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![id, p.first_name, p.last_name, p.email, p.include_in_emails as i64])?;
        }
        self.conn.execute("DELETE FROM contact_group_members WHERE contact_id = ?1", [id])?;
        for g in &c.groups { self.conn.execute("INSERT OR IGNORE INTO contact_group_members(group_id, contact_id) VALUES (?1, ?2)", params![g, id])?; }
        self.index_search("contact", id, c.name.trim(), &format!("{} {} {} {} {}", c.email, c.first_name, c.last_name, c.account_number, c.tax_number))?;
        Ok(())
    }

    /// Mark a contact as customer/supplier automatically when used on a document.
    pub fn flag_contact_role(&self, id: i64, customer: bool, supplier: bool) -> Result<()> {
        if customer { self.conn.execute("UPDATE contacts SET is_customer = 1 WHERE id = ?1", [id])?; }
        if supplier { self.conn.execute("UPDATE contacts SET is_supplier = 1 WHERE id = ?1", [id])?; }
        Ok(())
    }

    pub fn set_contact_status(&self, id: i64, status: &str) -> Result<()> {
        if status == "ARCHIVED" {
            let open: i64 = self.conn.query_row("SELECT COUNT(*) FROM documents WHERE contact_id = ?1 AND status IN ('AUTHORISED','SUBMITTED')", [id], |r| r.get(0))?;
            if open > 0 { return invalid("this contact has outstanding invoices or bills"); }
        }
        self.conn.execute("UPDATE contacts SET status = ?2 WHERE id = ?1", params![id, status])?;
        self.audit(status, "contact", Some(id), None, None)?;
        Ok(())
    }

    /// Merge `from` into `into`: all transactions move to `into`, `from` is archived.
    pub fn merge_contacts(&self, from: i64, into: i64) -> Result<()> {
        if from == into { return invalid("choose two different contacts"); }
        self.atomic(|| {
            self.conn.execute("UPDATE documents SET contact_id = ?2 WHERE contact_id = ?1", params![from, into])?;
            self.conn.execute("UPDATE bank_transactions SET contact_id = ?2 WHERE contact_id = ?1", params![from, into])?;
            // Journal lines are immutable; reporting resolves merged contacts via merged_into.
            self.conn.execute("UPDATE contacts SET status = 'ARCHIVED', merged_into = ?2 WHERE id = ?1", params![from, into])?;
            self.conn.execute("UPDATE contacts SET is_customer = MAX(is_customer, (SELECT is_customer FROM contacts WHERE id = ?1)),
                 is_supplier = MAX(is_supplier, (SELECT is_supplier FROM contacts WHERE id = ?1)) WHERE id = ?2", params![from, into])?;
            self.unindex_search("contact", from)?;
            self.audit("MERGE", "contact", Some(into), Some(json!({"merged": from})), None)?;
            self.add_note("contact", into, "HISTORY", &format!("Merged contact #{from} into this contact"))?;
            Ok(())
        })
    }

    pub fn list_contact_groups(&self) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT g.id, g.name, (SELECT COUNT(*) FROM contact_group_members m WHERE m.group_id = g.id) AS members FROM contact_groups g ORDER BY name", [])
    }

    pub fn create_contact_group(&self, name: &str) -> Result<i64> {
        if name.trim().is_empty() { return invalid("group name is required"); }
        self.conn.execute("INSERT INTO contact_groups(name) VALUES (?1)", [name.trim()])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_contact_group(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM contact_groups WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Payment terms for a contact (falls back to org defaults).
    pub fn contact_terms(&self, id: i64, sales: bool) -> Result<(String, i64)> {
        let (st, sd, bt, bd): (Option<String>, Option<i64>, Option<String>, Option<i64>) = self.conn.query_row(
            "SELECT sales_terms_type, sales_terms_days, bill_terms_type, bill_terms_days FROM contacts WHERE id = ?1", [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        let (t, d) = if sales { (st, sd) } else { (bt, bd) };
        let prefix = if sales { "sales" } else { "bill" };
        let t = match t { Some(t) => t, None => self.setting_or(&format!("{prefix}_terms_type"), "DAYSAFTERBILLDATE")? };
        let d = match d { Some(d) => d, None => self.setting_or(&format!("{prefix}_terms_days"), if sales { "14" } else { "30" })?.parse().unwrap_or(14) };
        Ok((t, d))
    }
}
