//! Approval rules: bills, purchase orders, invoices or credit notes at or above an amount need
//! approving by someone with a given role (Acumatica approval maps, Sage and Xero approvals).
//! Advisers can approve anything. Anyone else's "approve" saves the document as Awaiting
//! approval, and approving it directly is refused.

use crate::{error::invalid, money, util, Ledger, Result};
use rusqlite::params;
use serde_json::Value;

pub const DOC_TYPES: [&str; 5] = ["ACCPAY", "PO", "ACCREC", "ACCPAYCREDIT", "ACCRECCREDIT"];

pub fn role_label(r: &str) -> &'static str {
    match r { "ADVISER" => "Adviser", "STANDARD" => "Standard", "INVOICE_ONLY" => "Invoice only", "CASHBOOK" => "Cashbook", "PAYROLL_ADMIN" => "Payroll admin", "READONLY" => "Read only", _ => "Custom" }
}

fn type_label(t: &str) -> &'static str {
    match t { "ACCPAY" => "bills", "PO" => "purchase orders", "ACCREC" => "invoices", "ACCPAYCREDIT" => "supplier credits", "ACCRECCREDIT" => "credit notes", _ => "documents" }
}

impl Ledger {
    pub fn list_approval_rules(&self) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT id, doc_type, min_total, approver_role FROM approval_rules WHERE active = 1 ORDER BY doc_type, min_total", [])
    }

    pub fn save_approval_rule(&self, doc_type: &str, min_total: i64, role: &str) -> Result<i64> {
        if !DOC_TYPES.contains(&doc_type) { return invalid("approval rules are for bills, purchase orders, invoices and credit notes"); }
        if min_total < 0 { return invalid("the amount can't be negative"); }
        if !matches!(role, "ADVISER" | "STANDARD" | "CASHBOOK" | "PAYROLL_ADMIN" | "INVOICE_ONLY") { return invalid("choose who approves"); }
        self.conn.execute("INSERT INTO approval_rules(doc_type, min_total, approver_role) VALUES (?1, ?2, ?3)", params![doc_type, min_total, role])?;
        let id = self.conn.last_insert_rowid();
        self.audit("CREATE", "approval_rule", Some(id), None, Some(serde_json::json!({"doc_type": doc_type, "min_total": min_total, "role": role})))?;
        Ok(id)
    }

    pub fn delete_approval_rule(&self, id: i64) -> Result<()> {
        self.conn.execute("UPDATE approval_rules SET active = 0 WHERE id = ?1", [id])?;
        self.audit("DELETE", "approval_rule", Some(id), None, None)?;
        Ok(())
    }

    /// Why the current user can't approve this document (None = they can).
    pub fn approval_needed(&self, kind: &str, total: i64) -> Result<Option<String>> {
        if self.role == "ADVISER" { return Ok(None); }
        let rules: Vec<(i64, String)> = {
            let mut st = self.conn.prepare("SELECT min_total, approver_role FROM approval_rules WHERE active = 1 AND doc_type = ?1 AND min_total <= ?2 ORDER BY min_total DESC")?;
            st.query_map(params![kind, total], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
        };
        // The strictest rule that applies decides.
        Ok(rules.into_iter().find(|(_, role)| *role != self.role)
            .map(|(min, role)| format!("{} of {} or more need approving by {} {}", capital(type_label(kind)), money::fmt_money_grouped(min), article(role_label(&role)), role_label(&role).to_lowercase())))
    }
}

fn capital(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }
fn article(s: &str) -> &'static str { if s.starts_with(['A', 'E', 'I', 'O', 'U']) { "an" } else { "a" } }
