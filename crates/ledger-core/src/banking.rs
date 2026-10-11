//! Spend/receive money, transfers, overpayments & prepayments, bank rules.

use crate::{documents::LineInput, error::invalid, journal::{JLine, NewJournal}, money, tax::Lat, util, Error, Ledger, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct BankTxInput {
    #[serde(rename = "type")]
    pub kind: String, // SPEND RECEIVE SPEND-OVERPAYMENT RECEIVE-OVERPAYMENT SPEND-PREPAYMENT RECEIVE-PREPAYMENT
    pub account_id: i64,
    pub contact_id: Option<i64>,
    pub contact_name: Option<String>,
    pub date: String,
    pub reference: String,
    pub line_amount_type: Option<String>,
    pub rate: Option<String>,
    pub lines: Vec<LineInput>,
    pub amount: Option<String>, // for overpayments
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct TransferInput {
    pub from_account_id: i64,
    pub to_account_id: i64,
    pub date: String,
    pub amount: String,
    pub to_amount: Option<String>,
    pub reference: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct RuleCondition { pub field: String, pub op: String, pub value: String }

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct RuleLine {
    pub account_id: Option<i64>,
    pub tax_rate_id: Option<i64>,
    pub description: String,
    pub amount: Option<String>,
    pub percent: Option<String>,
    pub tracking1: Option<i64>,
    pub tracking2: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(default)]
pub struct BankRuleInput {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub account_id: Option<i64>,
    pub match_mode: String,
    pub conditions: Vec<RuleCondition>,
    pub contact_id: Option<i64>,
    pub reference_field: String,
    pub fixed_lines: Vec<RuleLine>,
    pub percent_lines: Vec<RuleLine>,
    pub transfer_account_id: Option<i64>,
    pub priority: i64,
    pub active: Option<bool>,
}

impl Ledger {
    fn bank_account_currency(&self, id: i64) -> Result<String> {
        let (ty, cur): (String, Option<String>) = self.conn.query_row("SELECT type, currency FROM accounts WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(|_| Error::NotFound("bank account".into()))?;
        if ty != "BANK" { return invalid("choose a bank account"); }
        Ok(cur.unwrap_or(self.base_currency()?))
    }

    /// Spend or receive money directly against a bank account.
    pub fn save_bank_transaction(&self, id: Option<i64>, t: &BankTxInput) -> Result<i64> {
        util::parse_date(&t.date)?;
        let kinds = ["SPEND", "RECEIVE", "SPEND-OVERPAYMENT", "RECEIVE-OVERPAYMENT", "SPEND-PREPAYMENT", "RECEIVE-PREPAYMENT"];
        if !kinds.contains(&t.kind.as_str()) { return invalid("unknown transaction type"); }
        let currency = self.bank_account_currency(t.account_id)?;
        let base = self.base_currency()?;
        let rate = if currency == base { 100_000_000 } else {
            match t.rate.as_deref().filter(|s| !s.trim().is_empty()) {
                Some(r) => money::parse_rate_e8(r).filter(|r| *r > 0).ok_or_else(|| Error::Validation("invalid exchange rate".into()))?,
                None => self.rate_for(&currency, &t.date)?,
            }
        };
        let contact = match (t.contact_id, t.contact_name.as_deref().filter(|s| !s.trim().is_empty())) {
            (Some(c), _) => Some(c), (None, Some(n)) => Some(self.find_or_create_contact(n)?), _ => None,
        };
        let is_credit = t.kind.contains("OVERPAYMENT") || t.kind.contains("PREPAYMENT");
        if is_credit && contact.is_none() { return invalid("overpayments and prepayments need a contact"); }
        let receive = t.kind.starts_with("RECEIVE");
        let lat = if is_credit { Lat::NoTax } else { Lat::parse(t.line_amount_type.as_deref().unwrap_or("INCLUSIVE"))? };
        let mut lines_in = t.lines.clone();
        if is_credit {
            // Overpayments and prepayments sit on AR/AP as an unallocated credit for the contact.
            let control = if receive { self.sys_account("AR")? } else { self.sys_account("AP")? };
            let amt = t.amount.clone().or_else(|| t.lines.first().and_then(|l| l.amount.clone())).ok_or_else(|| Error::Validation("amount required".into()))?;
            lines_in = vec![LineInput { description: if t.kind.contains("OVERPAYMENT") { "Overpayment".into() } else { "Prepayment".into() }, amount: Some(amt), account_id: Some(control), ..Default::default() }];
        }
        let lines = self.compute_lines(lat, &lines_in, receive)?;
        if lines.is_empty() { return invalid("add at least one line"); }
        for (i, l) in lines.iter().enumerate() { if l.account_id.is_none() { return invalid(format!("line {} needs an account", i + 1)); } }
        let subtotal: i64 = lines.iter().map(|l| l.net).sum();
        let total_tax: i64 = lines.iter().filter(|l| l.report_type.as_deref() != Some("REVERSECHARGE")).map(|l| l.tax).sum();
        let total = subtotal + total_tax;
        if total <= 0 { return invalid("the total must be greater than zero"); }
        self.check_lock(&t.date)?;
        let lat_s = match lat { Lat::Exclusive => "EXCLUSIVE", Lat::Inclusive => "INCLUSIVE", Lat::NoTax => "NOTAX" };
        self.atomic(|| {
            let tid = match id {
                None => {
                    self.conn.execute("INSERT INTO bank_transactions(type, account_id, date, currency) VALUES (?1, ?2, ?3, ?4)", params![t.kind, t.account_id, t.date, currency])?;
                    self.conn.last_insert_rowid()
                }
                Some(id) => {
                    let (jid, rec, status): (Option<i64>, i64, String) = self.conn.query_row("SELECT journal_id, reconciled, status FROM bank_transactions WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                    if status != "AUTHORISED" { return invalid("this transaction can't be edited"); }
                    let used: i64 = self.conn.query_row("SELECT COUNT(*) FROM prepayment_allocations WHERE bank_transaction_id = ?1 AND status <> 'DELETED'", [id], |r| r.get(0))?;
                    if used > 0 { return invalid("remove allocations before editing this credit"); }
                    let _ = rec;
                    if let Some(j) = jid { self.reverse_journal(j, None, None)?; }
                    id
                }
            };
            self.conn.execute(
                "UPDATE bank_transactions SET type=?2, account_id=?3, contact_id=?4, date=?5, reference=?6, line_amount_type=?7, currency=?8, rate_e8=?9,
                   subtotal=?10, total_tax=?11, total=?12, remaining_credit=?13 WHERE id=?1",
                params![tid, t.kind, t.account_id, contact, t.date, t.reference, lat_s, currency, rate, subtotal, total_tax, total, if is_credit { total } else { 0 }])?;
            self.write_lines("BANKTX", tid, &lines)?;
            let jid = self.post_bank_transaction(tid)?;
            self.conn.execute("UPDATE bank_transactions SET journal_id = ?2 WHERE id = ?1", params![tid, jid])?;
            let cname: String = contact.map(|c| self.conn.query_row("SELECT name FROM contacts WHERE id = ?1", [c], |r| r.get(0))).transpose()?.unwrap_or_default();
            self.index_search("banktx", tid, &format!("{} {} {}", if receive { "Receive money" } else { "Spend money" }, cname, t.reference), &money::fmt_money(total))?;
            self.audit(if id.is_some() { "UPDATE" } else { "CREATE" }, "bank_transaction", Some(tid), None, Some(json!({"type": t.kind, "total": total})))?;
            let what = if receive { "Receive money" } else { "Spend money" };
            self.add_note("banktx", tid, "HISTORY", &format!("{} {} for {}", what, if id.is_some() { "edited" } else { "created" }, money::fmt_money_grouped(total)))?;
            Ok(tid)
        })
    }

    fn post_bank_transaction(&self, id: i64) -> Result<i64> {
        let (kind, account, contact, date, reference, currency, rate): (String, i64, Option<i64>, String, String, String, i64) = self.conn.query_row(
            "SELECT type, account_id, contact_id, date, reference, currency, rate_e8 FROM bank_transactions WHERE id = ?1", [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?;
        let lines = self.read_lines("BANKTX", id)?;
        let s: i64 = if kind.starts_with("RECEIVE") { -1 } else { 1 };
        let gst = self.sys_account("GST")?;
        let mut jl = Vec::new();
        let mut total_fx = 0;
        for l in &lines {
            let net_b = self.to_base_amount(l.net, &currency, rate)?;
            let tax_b = self.to_base_amount(l.tax, &currency, rate)?;
            jl.push(JLine { account_id: l.account_id.unwrap(), amount: s * net_b, description: l.description.clone(), tax_rate_id: l.tax_rate_id, tax_amount: s * tax_b,
                contact_id: contact, tracking1: l.tracking1, tracking2: l.tracking2, currency: Some(currency.clone()), fx_amount: Some(s * l.net), item_id: l.item_id, qty_e4: Some(l.qty_e4) });
            total_fx += l.net;
            if l.tax != 0 && l.report_type.as_deref() != Some("REVERSECHARGE") {
                jl.push(JLine { account_id: gst, amount: s * tax_b, description: l.description.clone(), contact_id: contact, currency: Some(currency.clone()), fx_amount: Some(s * l.tax), ..Default::default() });
                total_fx += l.tax;
            }
        }
        let other: i64 = jl.iter().map(|l| l.amount).sum();
        jl.push(JLine { account_id: account, amount: -other, description: reference.clone(), contact_id: contact, currency: Some(currency.clone()), fx_amount: Some(-s * total_fx), ..Default::default() });
        self.post_journal(NewJournal { date, source_type: if s < 0 { "CASHREC".into() } else { "CASHPAID".into() }, source_id: Some(id), reference: reference.clone(), narration: reference, lines: jl, cash_basis: true, reverses: None })
    }

    pub fn void_bank_transaction(&self, id: i64) -> Result<()> {
        let (jid, rec, date, status): (Option<i64>, i64, String, String) = self.conn.query_row("SELECT journal_id, reconciled, date, status FROM bank_transactions WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        if status == "DELETED" { return Ok(()); }
        if rec != 0 { return invalid("this transaction is reconciled; unreconcile it first"); }
        let used: i64 = self.conn.query_row("SELECT COUNT(*) FROM prepayment_allocations WHERE bank_transaction_id = ?1 AND status <> 'DELETED'", [id], |r| r.get(0))?;
        if used > 0 { return invalid("remove allocations first"); }
        self.check_lock(&date)?;
        self.atomic(|| {
            if let Some(j) = jid { let r = self.reverse_journal(j, None, None)?; self.conn.execute("UPDATE bank_transactions SET void_journal_id = ?2 WHERE id = ?1", params![id, r])?; }
            self.conn.execute("UPDATE bank_transactions SET status = 'DELETED' WHERE id = ?1", [id])?;
            self.unindex_search("banktx", id)?;
            self.audit("DELETE", "bank_transaction", Some(id), None, None)?;
            self.add_note("banktx", id, "HISTORY", "Transaction deleted (reversed in the ledger)")?;
            Ok(())
        })
    }

    pub fn get_bank_transaction(&self, id: i64) -> Result<Value> {
        let mut t = util::query_one_json(&self.conn,
            "SELECT b.*, a.name AS account_name, c.name AS contact_name FROM bank_transactions b JOIN accounts a ON a.id = b.account_id LEFT JOIN contacts c ON c.id = b.contact_id WHERE b.id = ?1", [id])?;
        t["lines"] = json!(util::query_json(&self.conn,
            "SELECT li.*, a.code AS account_code, a.name AS account_name, t.name AS tax_rate_name FROM line_items li LEFT JOIN accounts a ON a.id = li.account_id
             LEFT JOIN tax_rates t ON t.id = li.tax_rate_id WHERE li.parent_kind = 'BANKTX' AND li.parent_id = ?1 ORDER BY li.sort", [id])?);
        t["allocations"] = json!(util::query_json(&self.conn,
            "SELECT pa.*, d.number FROM prepayment_allocations pa JOIN documents d ON d.id = pa.document_id WHERE pa.bank_transaction_id = ?1 AND pa.status <> 'DELETED'", [id])?);
        t["history"] = json!(self.history("banktx", id)?);
        Ok(t)
    }

    /// Apply an overpayment/prepayment credit to an invoice or bill.
    pub fn allocate_prepayment(&self, tx_id: i64, doc_id: i64, amount: &str, date: Option<&str>) -> Result<i64> {
        let amount = money::parse_fixed(amount, 2).filter(|a| *a > 0).ok_or_else(|| Error::Validation("amount must be greater than zero".into()))?;
        let (kind, contact, remaining, currency, rate, status): (String, Option<i64>, i64, String, i64, String) = self.conn.query_row(
            "SELECT type, contact_id, remaining_credit, currency, rate_e8, status FROM bank_transactions WHERE id = ?1", [tx_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
        if status != "AUTHORISED" || !(kind.contains("OVERPAYMENT") || kind.contains("PREPAYMENT")) { return invalid("not an available credit"); }
        let (dkind, dcontact, dcur, dtotal, dpaid, dbase, dstatus): (String, i64, String, i64, i64, i64, String) = self.conn.query_row(
            "SELECT type, contact_id, currency, total, amount_paid + amount_credited, base_total, status FROM documents WHERE id = ?1", [doc_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?;
        if dstatus != "AUTHORISED" { return invalid("the document must be approved and unpaid"); }
        if Some(dcontact) != contact { return invalid("the credit belongs to a different contact"); }
        if dcur != currency { return invalid("currencies must match"); }
        let receive = kind.starts_with("RECEIVE");
        if receive != (dkind == "ACCREC") || !(dkind == "ACCREC" || dkind == "ACCPAY") { return invalid("credit can't be applied to this document type"); }
        if amount > remaining { return invalid("amount exceeds the remaining credit"); }
        if amount > dtotal - dpaid { return invalid("amount exceeds the amount due"); }
        let date = date.map(String::from).unwrap_or_else(util::today);
        let doc_base = money::div_round(dbase as i128 * amount as i128, dtotal.max(1) as i128) as i64;
        let credit_base = self.to_base_amount(amount, &currency, rate)?;
        self.atomic(|| {
            let control = if receive { self.sys_account("AR")? } else { self.sys_account("AP")? };
            let mut jid = None;
            if doc_base != credit_base {
                let s = if receive { 1 } else { -1 };
                let mut lines = vec![JLine::new(control, -s * doc_base).contact(contact).desc("Credit applied"), JLine::new(control, s * credit_base).contact(contact).desc("Credit applied")];
                let diff: i64 = lines.iter().map(|l| l.amount).sum();
                lines.push(JLine::new(self.sys_account("FXGAIN")?, -diff).desc("Realised FX"));
                jid = Some(self.post_journal(NewJournal { date: date.clone(), source_type: "PREPAYALLOC".into(), source_id: Some(tx_id), reference: String::new(),
                    narration: "Credit applied".into(), lines, cash_basis: true, reverses: None })?);
            } else { self.check_lock(&date)?; }
            self.conn.execute("INSERT INTO prepayment_allocations(bank_transaction_id, document_id, amount, doc_base, date, journal_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![tx_id, doc_id, amount, doc_base, date, jid])?;
            let aid = self.conn.last_insert_rowid();
            self.conn.execute("UPDATE bank_transactions SET remaining_credit = remaining_credit - ?2 WHERE id = ?1", params![tx_id, amount])?;
            self.refresh_paid(doc_id)?;
            self.add_note("document", doc_id, "HISTORY", &format!("Credit of {} applied", money::fmt_money_grouped(amount)))?;
            self.add_note("banktx", tx_id, "HISTORY", &format!("{} applied to a document", money::fmt_money_grouped(amount)))?;
            Ok(aid)
        })
    }

    /// Outstanding overpayment/prepayment credits for a contact.
    pub fn contact_credits(&self, contact_id: i64) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT 'banktx' AS kind, id, type, date, reference, currency, total, remaining_credit AS remaining FROM bank_transactions
               WHERE contact_id = ?1 AND status = 'AUTHORISED' AND remaining_credit > 0
             UNION ALL
             SELECT 'document', id, type, date, number, currency, total, total - amount_paid - amount_credited FROM documents
               WHERE contact_id = ?1 AND type IN ('ACCRECCREDIT','ACCPAYCREDIT') AND status = 'AUTHORISED'
             ORDER BY date", [contact_id])
    }

    pub fn save_transfer(&self, t: &TransferInput) -> Result<i64> {
        util::parse_date(&t.date)?;
        if t.from_account_id == t.to_account_id { return invalid("choose two different accounts"); }
        let fc = self.bank_account_currency(t.from_account_id)?;
        let tc = self.bank_account_currency(t.to_account_id)?;
        let amount = money::parse_fixed(&t.amount, 2).filter(|a| *a > 0).ok_or_else(|| Error::Validation("amount must be greater than zero".into()))?;
        let to_amount = if fc == tc { amount } else {
            t.to_amount.as_deref().and_then(|s| money::parse_fixed(s, 2)).filter(|a| *a > 0).ok_or_else(|| Error::Validation("enter the amount received in the destination currency".into()))?
        };
        let base = self.base_currency()?;
        let from_base = if fc == base { amount } else { self.to_base_amount(amount, &fc, self.rate_for(&fc, &t.date)?)? };
        let to_base = if tc == base { to_amount } else if fc == base { from_base } else { self.to_base_amount(to_amount, &tc, self.rate_for(&tc, &t.date)?)? };
        self.atomic(|| {
            self.conn.execute("INSERT INTO transfers(from_account_id, to_account_id, date, amount, to_amount, reference) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![t.from_account_id, t.to_account_id, t.date, amount, to_amount, t.reference])?;
            let id = self.conn.last_insert_rowid();
            let mut lines = vec![
                JLine { account_id: t.to_account_id, amount: to_base, description: format!("Transfer {}", t.reference), currency: Some(tc.clone()), fx_amount: Some(to_amount), ..Default::default() },
                JLine { account_id: t.from_account_id, amount: -from_base, description: format!("Transfer {}", t.reference), currency: Some(fc.clone()), fx_amount: Some(-amount), ..Default::default() },
            ];
            if to_base != from_base { lines.push(JLine::new(self.sys_account("FXGAIN")?, from_base - to_base).desc("FX on transfer")); }
            let jid = self.post_journal(NewJournal { date: t.date.clone(), source_type: "TRANSFER".into(), source_id: Some(id), reference: t.reference.clone(),
                narration: "Bank transfer".into(), lines, cash_basis: true, reverses: None })?;
            self.conn.execute("UPDATE transfers SET journal_id = ?2 WHERE id = ?1", params![id, jid])?;
            self.audit("CREATE", "transfer", Some(id), None, Some(json!({"amount": amount})))?;
            Ok(id)
        })
    }

    pub fn void_transfer(&self, id: i64) -> Result<()> {
        let (jid, fr, tr, status): (Option<i64>, i64, i64, String) = self.conn.query_row("SELECT journal_id, from_reconciled, to_reconciled, status FROM transfers WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        if status == "DELETED" { return Ok(()); }
        if fr != 0 || tr != 0 { return invalid("this transfer is reconciled; unreconcile it first"); }
        self.atomic(|| {
            if let Some(j) = jid { self.reverse_journal(j, None, None)?; }
            self.conn.execute("UPDATE transfers SET status = 'DELETED' WHERE id = ?1", [id])?;
            Ok(())
        })
    }

    /// All cash movements for a bank account (the "Account Transactions" tab).
    pub fn bank_account_transactions(&self, account_id: i64, limit: i64) -> Result<Vec<Value>> {
        util::query_json(&self.conn,
            "SELECT * FROM (
               SELECT 'payment' AS kind, p.id, p.date, CASE WHEN d.type IN ('ACCREC','ACCPAYCREDIT') THEN p.amount ELSE -p.amount END AS amount,
                 c.name AS contact, COALESCE(NULLIF(p.reference,''), d.number) AS reference, 'Payment: ' || d.number AS description, p.reconciled, p.document_id AS document_id
               FROM payments p JOIN documents d ON d.id = p.document_id JOIN contacts c ON c.id = d.contact_id WHERE p.account_id = ?1 AND p.status <> 'DELETED'
               UNION ALL
               SELECT 'banktx', b.id, b.date, CASE WHEN b.type LIKE 'RECEIVE%' THEN b.total ELSE -b.total END, c.name, b.reference,
                 CASE WHEN b.type LIKE 'RECEIVE%' THEN 'Receive money' ELSE 'Spend money' END, b.reconciled, NULL
               FROM bank_transactions b LEFT JOIN contacts c ON c.id = b.contact_id WHERE b.account_id = ?1 AND b.status <> 'DELETED'
               UNION ALL
               SELECT 'transfer_from', t.id, t.date, -t.amount, (SELECT name FROM accounts WHERE id = t.to_account_id), t.reference, 'Transfer to ' || (SELECT name FROM accounts WHERE id = t.to_account_id), t.from_reconciled, NULL
               FROM transfers t WHERE t.from_account_id = ?1 AND t.status <> 'DELETED'
               UNION ALL
               SELECT 'transfer_to', t.id, t.date, t.to_amount, (SELECT name FROM accounts WHERE id = t.from_account_id), t.reference, 'Transfer from ' || (SELECT name FROM accounts WHERE id = t.from_account_id), t.to_reconciled, NULL
               FROM transfers t WHERE t.to_account_id = ?1 AND t.status <> 'DELETED'
             ) ORDER BY date DESC, id DESC LIMIT ?2", params![account_id, limit])
    }

    // ---------- Bank rules ----------

    pub fn list_bank_rules(&self) -> Result<Vec<Value>> {
        let mut rules = util::query_json(&self.conn, "SELECT r.*, c.name AS contact_name, a.name AS account_name FROM bank_rules r LEFT JOIN contacts c ON c.id = r.contact_id LEFT JOIN accounts a ON a.id = r.account_id ORDER BY r.type, r.priority, r.id", [])?;
        for r in rules.iter_mut() {
            for k in ["conditions", "fixed_lines", "percent_lines"] {
                let v: Value = serde_json::from_str(r[k].as_str().unwrap_or("[]")).unwrap_or(json!([]));
                r[k] = v;
            }
        }
        Ok(rules)
    }

    pub fn save_bank_rule(&self, id: Option<i64>, r: &BankRuleInput) -> Result<i64> {
        if r.name.trim().is_empty() { return invalid("give the rule a name"); }
        if !["SPEND", "RECEIVE", "TRANSFER"].contains(&r.kind.as_str()) { return invalid("rule type must be SPEND, RECEIVE or TRANSFER"); }
        if r.conditions.is_empty() { return invalid("add at least one condition"); }
        if r.kind == "TRANSFER" && r.transfer_account_id.is_none() { return invalid("choose the account to transfer to"); }
        if r.kind != "TRANSFER" && r.fixed_lines.is_empty() && r.percent_lines.is_empty() { return invalid("add at least one allocation line"); }
        let pct: i64 = r.percent_lines.iter().filter_map(|l| l.percent.as_deref().and_then(|p| money::parse_fixed(p, 2))).sum();
        if !r.percent_lines.is_empty() && pct != 10_000 { return invalid("percentage lines must add up to 100%"); }
        let mm = if r.match_mode == "ANY" { "ANY" } else { "ALL" };
        let c = serde_json::to_string(&r.conditions)?; let f = serde_json::to_string(&r.fixed_lines)?; let p = serde_json::to_string(&r.percent_lines)?;
        match id {
            None => {
                self.conn.execute("INSERT INTO bank_rules(name, type, account_id, match_mode, conditions, contact_id, reference_field, fixed_lines, percent_lines, transfer_account_id, priority, active)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                    params![r.name.trim(), r.kind, r.account_id, mm, c, r.contact_id, r.reference_field, f, p, r.transfer_account_id, r.priority, r.active.unwrap_or(true) as i64])?;
                Ok(self.conn.last_insert_rowid())
            }
            Some(id) => {
                self.conn.execute("UPDATE bank_rules SET name=?2, type=?3, account_id=?4, match_mode=?5, conditions=?6, contact_id=?7, reference_field=?8, fixed_lines=?9, percent_lines=?10,
                    transfer_account_id=?11, priority=?12, active=?13 WHERE id=?1",
                    params![id, r.name.trim(), r.kind, r.account_id, mm, c, r.contact_id, r.reference_field, f, p, r.transfer_account_id, r.priority, r.active.unwrap_or(true) as i64])?;
                Ok(id)
            }
        }
    }

    pub fn delete_bank_rule(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM bank_rules WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Find the first active rule matching a statement line.
    pub fn match_rule(&self, line_id: i64) -> Result<Option<(i64, BankRuleInput)>> {
        let (account, amount, payee, desc, reference): (i64, i64, String, String, String) = self.conn.query_row(
            "SELECT account_id, amount, payee, description, reference FROM statement_lines WHERE id = ?1", [line_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?;
        let kind = if amount < 0 { "SPEND" } else { "RECEIVE" };
        let mut stmt = self.conn.prepare("SELECT id, name, type, account_id, match_mode, conditions, contact_id, reference_field, fixed_lines, percent_lines, transfer_account_id, priority
            FROM bank_rules WHERE active = 1 AND (account_id IS NULL OR account_id = ?1) ORDER BY priority, id")?;
        let rules = stmt.query_map([account], |r| {
            Ok((r.get::<_, i64>(0)?, BankRuleInput {
                name: r.get(1)?, kind: r.get(2)?, account_id: r.get(3)?, match_mode: r.get(4)?,
                conditions: serde_json::from_str(&r.get::<_, String>(5)?).unwrap_or_default(), contact_id: r.get(6)?, reference_field: r.get(7)?,
                fixed_lines: serde_json::from_str(&r.get::<_, String>(8)?).unwrap_or_default(), percent_lines: serde_json::from_str(&r.get::<_, String>(9)?).unwrap_or_default(),
                transfer_account_id: r.get(10)?, priority: r.get(11)?, active: Some(true),
            }))
        })?.collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, rule) in rules {
            if rule.kind != "TRANSFER" && rule.kind != kind { continue; }
            let test = |c: &RuleCondition| -> bool {
                let v = c.value.to_lowercase();
                if c.field == "amount" {
                    let a = amount.abs();
                    let x = money::parse_fixed(&c.value, 2).unwrap_or(i64::MIN);
                    return match c.op.as_str() { "equals" => a == x, "gt" => a > x, "lt" => a < x, "gte" => a >= x, "lte" => a <= x, _ => false };
                }
                let hay = match c.field.as_str() { "payee" => payee.to_lowercase(), "description" => desc.to_lowercase(), "reference" => reference.to_lowercase(),
                    _ => format!("{} {} {}", payee, desc, reference).to_lowercase() };
                match c.op.as_str() { "equals" => hay == v, "starts_with" => hay.starts_with(&v), "not_contains" => !hay.contains(&v), _ => hay.contains(&v) }
            };
            let hit = if rule.match_mode == "ANY" { rule.conditions.iter().any(test) } else { rule.conditions.iter().all(test) };
            if hit { return Ok(Some((id, rule))); }
        }
        Ok(None)
    }

    pub fn rule_by_id(&self, id: i64) -> Result<BankRuleInput> {
        let rules = self.list_bank_rules()?;
        let r = rules.into_iter().find(|r| r["id"].as_i64() == Some(id)).ok_or_else(|| Error::NotFound("bank rule".into()))?;
        Ok(serde_json::from_value(r)?)
    }
}
