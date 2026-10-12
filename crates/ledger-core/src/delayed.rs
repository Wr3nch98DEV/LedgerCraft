//! Delayed charges and delayed credits (QuickBooks): non-posting records of something to bill (or
//! credit) a customer later. They're stored like quotes (they post nothing) with a subtype, stay
//! "open" until added to an invoice, then close.

use crate::documents::DocInput;
use crate::{error::invalid, util, Ledger, Result};
use serde_json::Value;

pub const CHARGE: &str = "DELAYEDCHARGE";
pub const CREDIT: &str = "DELAYEDCREDIT";

impl Ledger {
    /// Create or update a delayed charge (`credit = false`) or delayed credit.
    pub fn save_delayed(&self, id: Option<i64>, input: &DocInput, credit: bool) -> Result<i64> {
        if let Some(id) = id {
            let (sub, status): (String, String) = self.conn.query_row("SELECT subtype, status FROM documents WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
            if !sub.starts_with("DELAYED") { return invalid("not a delayed charge or credit"); }
            if status == "INVOICED" { return invalid("this has already been added to an invoice"); }
        }
        let mut d = input.clone();
        d.kind = "QUOTE".into();
        d.status = Some("SENT".into());
        self.atomic(|| {
            let id = self.save_document(id, &d)?;
            self.conn.execute("UPDATE documents SET subtype = ?2 WHERE id = ?1", rusqlite::params![id, if credit { CREDIT } else { CHARGE }])?;
            Ok(id)
        })
    }

    /// Open delayed charges and credits for a customer, with their lines, oldest first.
    pub fn open_delayed(&self, contact_id: i64) -> Result<Vec<Value>> {
        let ids: Vec<i64> = util::query_json(&self.conn,
            "SELECT id FROM documents WHERE type = 'QUOTE' AND subtype IN ('DELAYEDCHARGE','DELAYEDCREDIT') AND status = 'SENT' AND contact_id = ?1 ORDER BY date, id", [contact_id])?
            .iter().filter_map(|r| r["id"].as_i64()).collect();
        ids.into_iter().map(|id| self.get_document(id)).collect()
    }

    /// Mark delayed charges/credits as added to an invoice.
    pub fn close_delayed(&self, ids: &[i64], invoice_id: i64) -> Result<()> {
        let (number, contact): (String, i64) = self.conn.query_row("SELECT number, contact_id FROM documents WHERE id = ?1 AND type = 'ACCREC'", [invoice_id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        for id in ids {
            let (sub, status, c): (String, String, i64) = self.conn.query_row("SELECT subtype, status, contact_id FROM documents WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            if !sub.starts_with("DELAYED") { return invalid("not a delayed charge or credit"); }
            if status != "SENT" { return invalid("that delayed charge has already been invoiced"); }
            if c != contact { return invalid("delayed charges must be for the invoice's customer"); }
            self.conn.execute("UPDATE documents SET status = 'INVOICED' WHERE id = ?1", [id])?;
            self.add_note("document", *id, "HISTORY", &format!("Added to invoice {number}"))?;
        }
        Ok(())
    }

    /// Save an invoice and close the delayed charges/credits added to it, in one step.
    pub fn save_invoice_with_delayed(&self, id: Option<i64>, input: &DocInput, delayed: &[i64]) -> Result<i64> {
        self.atomic(|| {
            let inv = self.save_document(id, input)?;
            if !delayed.is_empty() { self.close_delayed(delayed, inv)?; }
            Ok(inv)
        })
    }
}
