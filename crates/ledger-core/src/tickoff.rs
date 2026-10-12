//! Statement tick-off reconciliation (QuickBooks "Reconcile").
//!
//! Enter the bank statement's ending date and balance, tick the transactions that cleared, and
//! finish once the difference is zero. Ticked items are marked reconciled with the same flags
//! that statement-line matching (Xero-style) uses, so both methods agree on what's reconciled.

use crate::{error::invalid, util, Ledger, Result};
use rusqlite::{params, OptionalExtension};
use serde_json::{json, Value};

impl Ledger {
    /// Beginning balance: everything already cleared — the account balance less every transaction
    /// not yet reconciled. Works whichever way earlier items were reconciled (tick-off or
    /// statement-line matching); opening balances and manual journals count as cleared.
    pub fn reconcile_beginning_balance(&self, account: i64) -> Result<i64> {
        let total: i64 = self.conn.query_row("SELECT COALESCE(SUM(COALESCE(jl.fx_amount, jl.amount)), 0) FROM journal_lines jl JOIN journals j ON j.id = jl.journal_id
            WHERE jl.account_id = ?1 AND j.posted = 1", [account], |r| r.get(0))?;
        let open: i64 = self.unreconciled_ledger_items(account)?.iter().map(|i| i["amount"].as_i64().unwrap_or(0)).sum();
        Ok(total - open)
    }

    /// Start (or resume, updating its figures) a reconciliation for an account.
    pub fn reconcile_start(&self, account: i64, statement_date: &str, ending_balance: i64) -> Result<i64> {
        util::parse_date(statement_date)?;
        let kind: String = self.conn.query_row("SELECT type FROM accounts WHERE id = ?1", [account], |r| r.get(0))?;
        if kind != "BANK" { return invalid("only bank and credit card accounts are reconciled"); }
        if let Some(id) = self.conn.query_row("SELECT id FROM reconciliations WHERE account_id = ?1 AND status = 'IN_PROGRESS'", [account], |r| r.get::<_, i64>(0)).optional()? {
            self.conn.execute("UPDATE reconciliations SET statement_date = ?2, ending_balance = ?3 WHERE id = ?1", params![id, statement_date, ending_balance])?;
            return Ok(id);
        }
        let begin = self.reconcile_beginning_balance(account)?;
        self.conn.execute("INSERT INTO reconciliations(account_id, statement_date, beginning_balance, ending_balance) VALUES (?1, ?2, ?3, ?4)", params![account, statement_date, begin, ending_balance])?;
        Ok(self.conn.last_insert_rowid())
    }

    fn reconciliation_row(&self, id: i64) -> Result<(i64, String, i64, i64, String)> {
        Ok(self.conn.query_row("SELECT account_id, statement_date, beginning_balance, ending_balance, status FROM reconciliations WHERE id = ?1", [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?)
    }

    /// The reconciliation screen: uncleared transactions up to the statement date, ticked ones, totals.
    pub fn reconcile_session(&self, id: i64) -> Result<Value> {
        let (account, date, stored_begin, end, status) = self.reconciliation_row(id)?;
        let begin = if status == "IN_PROGRESS" { self.reconcile_beginning_balance(account)? } else { stored_begin };
        let ticked: Vec<(String, i64)> = {
            let mut st = self.conn.prepare("SELECT kind, target_id FROM reconciliation_items WHERE reconciliation_id = ?1")?;
            st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
        };
        let mut items = self.unreconciled_ledger_items(account)?;
        items.retain(|i| i["date"].as_str().unwrap_or("") <= date.as_str());
        let mut cleared = 0i64;
        for i in items.iter_mut() {
            let on = ticked.iter().any(|(k, t)| i["kind"].as_str() == Some(k) && i["id"].as_i64() == Some(*t));
            if on { cleared += i["amount"].as_i64().unwrap_or(0); }
            i["cleared"] = json!(on);
        }
        Ok(json!({"id": id, "account_id": account, "statement_date": date, "beginning_balance": begin, "ending_balance": end, "status": status,
            "items": items, "cleared_total": cleared, "cleared_balance": begin + cleared, "difference": end - (begin + cleared)}))
    }

    /// Tick or untick one transaction.
    pub fn reconcile_toggle(&self, id: i64, kind: &str, target: i64, on: bool) -> Result<()> {
        let (_, _, _, _, status) = self.reconciliation_row(id)?;
        if status != "IN_PROGRESS" { return invalid("this reconciliation is finished"); }
        if on {
            self.conn.execute("INSERT OR IGNORE INTO reconciliation_items(reconciliation_id, kind, target_id) VALUES (?1, ?2, ?3)", params![id, kind, target])?;
        } else {
            self.conn.execute("DELETE FROM reconciliation_items WHERE reconciliation_id = ?1 AND kind = ?2 AND target_id = ?3", params![id, kind, target])?;
        }
        Ok(())
    }

    /// Finish when the difference is zero: everything ticked becomes reconciled.
    pub fn reconcile_finish(&self, id: i64) -> Result<()> {
        let s = self.reconcile_session(id)?;
        if s["status"] != "IN_PROGRESS" { return invalid("this reconciliation is already finished"); }
        let diff = s["difference"].as_i64().unwrap_or(0);
        if diff != 0 { return invalid(format!("the difference is {} — it must be 0.00 to finish", crate::money::fmt_money_grouped(diff))); }
        self.atomic(|| {
            for i in s["items"].as_array().cloned().unwrap_or_default().iter().filter(|i| i["cleared"] == true) {
                self.set_target_reconciled(i["kind"].as_str().unwrap_or(""), i["id"].as_i64().unwrap_or(0), 1)?;
            }
            // Ticked items that are no longer on the list (e.g. dated after the statement) are dropped.
            self.conn.execute("UPDATE reconciliations SET status = 'FINISHED', finished_at = datetime('now'), cleared_total = ?2, beginning_balance = ?3 WHERE id = ?1", params![id, s["cleared_total"].as_i64(), s["beginning_balance"].as_i64()])?;
            self.audit("RECONCILE", "account", s["account_id"].as_i64(), None, Some(json!({"statement_date": s["statement_date"], "ending_balance": s["ending_balance"]})))?;
            Ok(())
        })
    }

    /// Undo the most recent finished reconciliation of an account.
    pub fn reconcile_undo_last(&self, account: i64) -> Result<()> {
        let Some(id) = self.conn.query_row("SELECT id FROM reconciliations WHERE account_id = ?1 AND status = 'FINISHED' ORDER BY statement_date DESC, id DESC LIMIT 1", [account], |r| r.get::<_, i64>(0)).optional()? else {
            return invalid("there's no finished reconciliation to undo");
        };
        self.atomic(|| {
            let items: Vec<(String, i64)> = {
                let mut st = self.conn.prepare("SELECT kind, target_id FROM reconciliation_items WHERE reconciliation_id = ?1")?;
                st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<_>>()?
            };
            for (k, t) in items { self.set_target_reconciled(&k, t, 0)?; }
            self.conn.execute("UPDATE reconciliations SET status = 'UNDONE' WHERE id = ?1", [id])?;
            self.audit("UNRECONCILE", "account", Some(account), None, Some(json!({"reconciliation": id})))?;
            Ok(())
        })
    }

    pub fn list_reconciliations(&self, account: i64) -> Result<Vec<Value>> {
        util::query_json(&self.conn, "SELECT * FROM reconciliations WHERE account_id = ?1 ORDER BY statement_date DESC, id DESC", [account])
    }
}
