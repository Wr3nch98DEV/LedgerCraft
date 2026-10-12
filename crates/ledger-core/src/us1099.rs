//! US 1099 tracking: vendors flagged for 1099s, and the yearly contractor summary of what was
//! paid to them. Card payments are left out (the card processor reports those on a 1099-K).

use crate::{reports::{fmt_long, ReportParams}, util, Ledger, Result};
use rusqlite::params;
use serde_json::{json, Value};

/// The IRS reporting threshold for 1099-NEC, in cents.
pub const THRESHOLD: i64 = 60000;

impl Ledger {
    pub fn set_track_1099(&self, contact: i64, on: bool) -> Result<()> {
        self.conn.execute("UPDATE contacts SET track_1099 = ?2 WHERE id = ?1", params![contact, on as i64])?;
        self.audit("UPDATE", "contact", Some(contact), None, Some(json!({"track_1099": on})))?;
        Ok(())
    }

    /// Per flagged vendor: (contact id, name, tax id, paid other than by card, paid by card).
    pub fn payments_1099(&self, from: &str, to: &str) -> Result<Vec<(i64, String, String, i64, i64)>> {
        let vendors = util::query_json(&self.conn, "SELECT id, name, tax_number FROM contacts WHERE track_1099 = 1 ORDER BY name", [])?;
        let mut out = Vec::new();
        for v in vendors {
            let id = v["id"].as_i64().unwrap_or(0);
            // Bill payments (less refunds on vendor credits) and direct spends (less refunds), split by card or not.
            let (cash, card): (i64, i64) = self.conn.query_row(
                "SELECT COALESCE(SUM(CASE WHEN card = 0 THEN amt END), 0), COALESCE(SUM(CASE WHEN card = 1 THEN amt END), 0) FROM (
                   SELECT p.base_amount * CASE WHEN d.type = 'ACCPAY' THEN 1 ELSE -1 END AS amt, COALESCE(a.bank_type = 'CREDITCARD', 0) AS card
                     FROM payments p JOIN documents d ON d.id = p.document_id JOIN accounts a ON a.id = p.account_id
                    WHERE d.contact_id = ?1 AND d.type IN ('ACCPAY','ACCPAYCREDIT') AND p.status <> 'DELETED' AND p.date >= ?2 AND p.date <= ?3
                   UNION ALL
                   SELECT CASE WHEN b.type LIKE 'SPEND%' THEN b.total ELSE -b.total END, COALESCE(a.bank_type = 'CREDITCARD', 0)
                     FROM bank_transactions b JOIN accounts a ON a.id = b.account_id
                    WHERE b.contact_id = ?1 AND b.status <> 'DELETED' AND b.date >= ?2 AND b.date <= ?3)",
                params![id, from, to], |r| Ok((r.get(0)?, r.get(1)?)))?;
            out.push((id, v["name"].as_str().unwrap_or("").to_string(), v["tax_number"].as_str().unwrap_or("").to_string(), cash, card));
        }
        Ok(out)
    }

    /// "1099 Contractor Summary": vendors at or over the threshold, then those under it.
    pub fn report_1099(&self, p: &ReportParams) -> Result<Value> {
        let to = p.to.clone().or(p.date.clone()).unwrap_or_else(util::today);
        let from = p.from.clone().unwrap_or_else(|| format!("{}-01-01", &to[..4]));
        let mut over = Vec::new();
        let mut under = Vec::new();
        let (mut tot_over, mut tot_under) = ([0i64; 3], [0i64; 3]);
        for (id, name, tin, cash, card) in self.payments_1099(&from, &to)? {
            let label = if tin.is_empty() { format!("{name} (no tax ID on file)") } else { format!("{name} ({tin})") };
            let row = json!({"label": label, "contact_id": id, "values": [cash + card, card, cash]});
            let (rows, tot) = if cash >= THRESHOLD { (&mut over, &mut tot_over) } else { (&mut under, &mut tot_under) };
            rows.push(row);
            tot[0] += cash + card; tot[1] += card; tot[2] += cash;
        }
        Ok(json!({"title": "1099 Contractor Summary", "org": self.setting_or("org_name", "")?, "subtitle": format!("{} to {}", fmt_long(&from), fmt_long(&to)),
            "columns": ["Total paid", "Paid by card (excluded)", "Reportable on 1099-NEC"],
            "sections": [
                {"title": "Vendors to file a 1099 for ($600 or more)", "rows": over, "total": {"label": "Total", "values": tot_over}},
                {"title": "Below the threshold", "rows": under, "total": {"label": "Total", "values": tot_under}},
            ]}))
    }
}
