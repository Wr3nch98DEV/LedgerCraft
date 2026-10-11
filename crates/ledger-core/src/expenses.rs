//! Expense claims (receipts and mileage). Approved claims become a bill to the claimant,
//! which is then reimbursed like any other bill.

use crate::{documents::{DocInput, LineInput}, error::invalid, money, tax::Lat, util, Error, Ledger, Result};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ReceiptInput {
    pub claimant_contact_id: Option<i64>,
    pub supplier: String,
    pub date: String,
    pub reference: String,
    pub kind: String,             // RECEIPT | MILEAGE
    pub distance: Option<String>,
    pub rate: Option<String>,     // per km/mile
    pub line_amount_type: Option<String>,
    pub lines: Vec<LineInput>,
    pub project_id: Option<i64>,
}

impl Ledger {
    /// Contact used to reimburse the current user (created on first use).
    pub fn claimant_contact(&self) -> Result<i64> {
        let uid = self.user_id.unwrap_or(1);
        let (name, email): (String, String) = self.conn.query_row("SELECT name, email FROM users WHERE id = ?1", [uid], |r| Ok((r.get(0)?, r.get(1)?)))?;
        if let Some(id) = self.conn.query_row("SELECT id FROM contacts WHERE name = ?1 AND is_employee = 1", [&name], |r| r.get::<_, i64>(0)).optional()? { return Ok(id); }
        let id = match self.conn.query_row("SELECT id FROM contacts WHERE name = ?1", [&name], |r| r.get::<_, i64>(0)).optional()? {
            Some(id) => id,
            None => self.create_contact(&crate::contacts::ContactInput { name: name.clone(), email, is_employee: Some(true), ..Default::default() })?,
        };
        self.conn.execute("UPDATE contacts SET is_employee = 1 WHERE id = ?1", [id])?;
        Ok(id)
    }

    pub fn list_receipts(&self, status: &str) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT r.*, c.name AS claimant, d.status AS bill_status, d.total - d.amount_paid AS owing FROM receipts r JOIN contacts c ON c.id = r.claimant_contact_id LEFT JOIN documents d ON d.id = r.document_id
            WHERE (?1 = 'ALL' AND r.status <> 'VOIDED') OR r.status = ?1 ORDER BY r.date DESC, r.id DESC", [status])
    }

    pub fn get_receipt(&self, id: i64) -> Result<Value> {
        let mut r = util::query_one_json(&self.conn, "SELECT r.*, c.name AS claimant FROM receipts r JOIN contacts c ON c.id = r.claimant_contact_id WHERE r.id = ?1", [id])?;
        r["lines"] = json!(util::query_json(&self.conn, "SELECT li.*, a.code AS account_code, a.name AS account_name FROM line_items li LEFT JOIN accounts a ON a.id = li.account_id WHERE parent_kind = 'RECEIPT' AND parent_id = ?1 ORDER BY sort", [id])?);
        r["attachments"] = json!(util::query_json(&self.conn, "SELECT id, filename, mime, size FROM attachments WHERE entity = 'receipt' AND entity_id = ?1", [id])?);
        r["history"] = json!(self.history("receipt", id)?);
        Ok(r)
    }

    pub fn save_receipt(&self, id: Option<i64>, r: &ReceiptInput) -> Result<i64> {
        util::parse_date(&r.date)?;
        let claimant = match r.claimant_contact_id { Some(c) => c, None => self.claimant_contact()? };
        let mut lines = r.lines.clone();
        let (distance, rate) = if r.kind == "MILEAGE" {
            let d = r.distance.as_deref().and_then(|s| money::parse_fixed(s, 4)).ok_or_else(|| Error::Validation("enter the distance travelled".into()))?;
            let rt = r.rate.as_deref().and_then(|s| money::parse_fixed(s, 4)).unwrap_or(8800); // 0.88/km default (ATO cents-per-km 2024-25)
            let acct = self.account_by_code("496")?.or(self.account_by_code("449")?);
            lines = vec![LineInput { description: format!("Mileage {} @ {}", money::fmt_fixed(d, 4).trim_end_matches('0').trim_end_matches('.'), money::fmt_fixed(rt, 4).trim_end_matches("00")),
                quantity: Some(money::fmt_fixed(d, 4)), unit_price: Some(money::fmt_fixed(rt, 4)), account_id: lines.first().and_then(|l| l.account_id).or(acct), ..Default::default() }];
            (Some(d), Some(rt / 100))
        } else { (None, None) };
        let lat = if r.kind == "MILEAGE" { Lat::NoTax } else { Lat::parse(r.line_amount_type.as_deref().unwrap_or("INCLUSIVE"))? };
        let computed = self.compute_lines(lat, &lines, false)?;
        if computed.is_empty() { return invalid("add what the expense was for"); }
        let sub: i64 = computed.iter().map(|l| l.net).sum();
        let tax: i64 = computed.iter().map(|l| l.tax).sum();
        let lat_s = match lat { Lat::Exclusive => "EXCLUSIVE", Lat::Inclusive => "INCLUSIVE", Lat::NoTax => "NOTAX" };
        self.atomic(|| {
            let rid = match id {
                None => { self.conn.execute("INSERT INTO receipts(user_id, claimant_contact_id, date) VALUES (?1, ?2, ?3)", params![self.user_id, claimant, r.date])?; self.conn.last_insert_rowid() }
                Some(id) => {
                    let st: String = self.conn.query_row("SELECT status FROM receipts WHERE id = ?1", [id], |x| x.get(0))?;
                    if !matches!(st.as_str(), "DRAFT" | "SUBMITTED" | "DECLINED") { return invalid("approved claims can't be edited"); }
                    id
                }
            };
            self.conn.execute("UPDATE receipts SET claimant_contact_id=?2, supplier=?3, date=?4, reference=?5, kind=?6, distance_e4=?7, rate_e2=?8, line_amount_type=?9, subtotal=?10, total_tax=?11, total=?12, project_id=?13 WHERE id=?1",
                params![rid, claimant, r.supplier, r.date, r.reference, if r.kind == "MILEAGE" { "MILEAGE" } else { "RECEIPT" }, distance, rate, lat_s, sub, tax, sub + tax, r.project_id])?;
            self.write_lines("RECEIPT", rid, &computed)?;
            if id.is_none() { self.add_note("receipt", rid, "HISTORY", "Expense claim created")?; }
            Ok(rid)
        })
    }

    pub fn set_receipt_status(&self, id: i64, status: &str) -> Result<()> {
        let r = self.get_receipt(id)?;
        let from = r["status"].as_str().unwrap_or("");
        self.atomic(|| {
            match (from, status) {
                ("DRAFT", "SUBMITTED") | ("DECLINED", "SUBMITTED") | ("SUBMITTED", "DECLINED") | ("SUBMITTED", "DRAFT") => {}
                ("DRAFT", "AUTHORISED") | ("SUBMITTED", "AUTHORISED") => {
                    // Create the reimbursement bill to the claimant.
                    let lines: Vec<LineInput> = r["lines"].as_array().unwrap().iter().map(|l| LineInput {
                        description: format!("{}{}", r["supplier"].as_str().filter(|s| !s.is_empty()).map(|s| format!("{s}: ")).unwrap_or_default(), l["description"].as_str().unwrap_or("")),
                        quantity: Some(money::fmt_fixed(l["qty_e4"].as_i64().unwrap_or(10_000), 4)), unit_price: Some(money::fmt_fixed(l["unit_e2"].as_i64().unwrap_or(0), 4)),
                        account_id: l["account_id"].as_i64(), tax_rate_id: l["tax_rate_id"].as_i64(), tracking1: l["tracking1"].as_i64(), tracking2: l["tracking2"].as_i64(), ..Default::default() }).collect();
                    let did = self.save_document(None, &DocInput { kind: "ACCPAY".into(), contact_id: r["claimant_contact_id"].as_i64(), date: r["date"].as_str().unwrap_or("").into(),
                        number: Some(format!("EXP-{id}")), reference: format!("Expense claim{}", r["reference"].as_str().filter(|s| !s.is_empty()).map(|s| format!(" · {s}")).unwrap_or_default()),
                        line_amount_type: r["line_amount_type"].as_str().map(String::from), lines, status: Some("AUTHORISED".into()), ..Default::default() })?;
                    self.conn.execute("UPDATE receipts SET document_id = ?2 WHERE id = ?1", params![id, did])?;
                    if let Some(p) = r["project_id"].as_i64() {
                        self.conn.execute("INSERT INTO project_costs(project_id, date, description, amount, source_kind, source_id) VALUES (?1, ?2, ?3, ?4, 'RECEIPT', ?5)",
                            params![p, r["date"].as_str(), format!("Expense claim: {}", r["supplier"].as_str().unwrap_or("")), r["subtotal"].as_i64(), id])?;
                    }
                }
                (_, "VOIDED") => {
                    if let Some(d) = r["document_id"].as_i64() { self.set_document_status(d, "VOIDED")?; }
                }
                _ => return invalid(format!("can't change a {} claim to {}", from.to_lowercase(), status.to_lowercase())),
            }
            self.conn.execute("UPDATE receipts SET status = ?2 WHERE id = ?1", params![id, status])?;
            self.add_note("receipt", id, "HISTORY", &format!("Claim {}", match status { "SUBMITTED" => "submitted for approval", "AUTHORISED" => "approved — awaiting reimbursement", "DECLINED" => "declined", "VOIDED" => "voided", _ => "returned to draft" }))?;
            Ok(())
        })
    }

    /// Refresh PAID status of claims whose bill has been paid.
    pub fn sync_receipts(&self) -> Result<()> {
        self.conn.execute("UPDATE receipts SET status = 'PAID' WHERE status = 'AUTHORISED' AND document_id IN (SELECT id FROM documents WHERE status = 'PAID')", [])?;
        Ok(())
    }
}
