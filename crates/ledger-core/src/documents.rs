//! Invoices (ACCREC), bills (ACCPAY), credit notes, quotes and purchase orders.

use crate::{error::invalid, journal::{JLine, NewJournal}, money, tax::{self, Lat}, util, Error, Ledger, Result};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

pub const DOC_TYPES: &[&str] = &["ACCREC", "ACCPAY", "ACCRECCREDIT", "ACCPAYCREDIT", "QUOTE", "PO"];

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct LineInput {
    pub item_id: Option<i64>,
    pub description: String,
    pub quantity: Option<String>,
    pub unit_price: Option<String>,
    pub discount: Option<String>,
    pub amount: Option<String>,
    pub account_id: Option<i64>,
    pub tax_rate_id: Option<i64>,
    pub tracking1: Option<i64>,
    pub tracking2: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct CLine {
    pub item_id: Option<i64>,
    pub description: String,
    pub qty_e4: i64,
    pub unit_e2: i64,
    pub discount_bp: i64,
    pub account_id: Option<i64>,
    pub tax_rate_id: Option<i64>,
    pub line_amount: i64,
    pub net: i64,
    pub tax: i64,
    pub tracking1: Option<i64>,
    pub tracking2: Option<i64>,
    pub report_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DocInput {
    #[serde(rename = "type")]
    pub kind: String,
    pub contact_id: Option<i64>,
    pub contact_name: Option<String>,
    pub number: Option<String>,
    pub reference: String,
    pub date: String,
    pub due_date: Option<String>,
    pub expiry_date: Option<String>,
    pub delivery_date: Option<String>,
    pub delivery_address: String,
    pub attention: String,
    pub title: String,
    pub summary: String,
    pub terms: String,
    pub line_amount_type: Option<String>,
    pub currency: Option<String>,
    pub rate: Option<String>,
    pub branding_theme_id: Option<i64>,
    pub lines: Vec<LineInput>,
    pub status: Option<String>,
    pub source_document_id: Option<i64>,
    pub expected_payment_date: Option<String>,
    pub planned_payment_date: Option<String>,
    pub repeating_id: Option<i64>,
}

pub fn is_sales(kind: &str) -> bool { matches!(kind, "ACCREC" | "ACCRECCREDIT" | "QUOTE") }
pub fn is_ledger_doc(kind: &str) -> bool { matches!(kind, "ACCREC" | "ACCPAY" | "ACCRECCREDIT" | "ACCPAYCREDIT") }

fn seq_for(kind: &str) -> Option<&'static str> {
    match kind { "ACCREC" => Some("INV"), "ACCRECCREDIT" | "ACCPAYCREDIT" => Some("CN"), "QUOTE" => Some("QU"), "PO" => Some("PO"), _ => None }
}

pub fn entity_name(kind: &str) -> &'static str {
    match kind { "ACCREC" => "invoice", "ACCPAY" => "bill", "ACCRECCREDIT" | "ACCPAYCREDIT" => "credit note", "QUOTE" => "quote", "PO" => "purchase order", _ => "document" }
}

impl Ledger {
    /// Turn user line input into computed lines (defaults from items, tax maths).
    pub fn compute_lines(&self, lat: Lat, lines: &[LineInput], sales: bool) -> Result<Vec<CLine>> {
        let mut out = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            let blank = l.item_id.is_none() && l.description.trim().is_empty() && l.unit_price.as_deref().unwrap_or("").trim().is_empty()
                && l.amount.as_deref().unwrap_or("").trim().is_empty() && l.account_id.is_none();
            if blank { continue; }
            let mut c = CLine { item_id: l.item_id, description: l.description.clone(), tracking1: l.tracking1, tracking2: l.tracking2,
                account_id: l.account_id, tax_rate_id: l.tax_rate_id, ..Default::default() };
            let mut item_unit: Option<i64> = None;
            if let Some(item) = l.item_id {
                let (sa, st, su, pa, pt, pu, tracked, inv, sd, pd): (Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, i64, Option<i64>, String, String) =
                    self.conn.query_row("SELECT sale_account_id, sale_tax_rate_id, sale_unit_e2, purchase_account_id, purchase_tax_rate_id, purchase_unit_e2, is_tracked, inventory_account_id, sales_description, purchase_description FROM items WHERE id = ?1",
                        [item], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?)))
                    .map_err(|_| Error::NotFound("item".into()))?;
                if sales {
                    c.account_id = c.account_id.or(sa); c.tax_rate_id = c.tax_rate_id.or(st); item_unit = su;
                    if c.description.is_empty() { c.description = sd; }
                } else {
                    // tracked purchases always go to the inventory asset account
                    c.account_id = if tracked != 0 { inv.or(Some(self.sys_account("INVENTORY")?)) } else { c.account_id.or(pa) };
                    c.tax_rate_id = c.tax_rate_id.or(pt); item_unit = pu;
                    if c.description.is_empty() { c.description = pd; }
                }
            }
            if c.account_id.is_none() {
                // fall back to the account's default tax rate later; account is required to approve
            } else if c.tax_rate_id.is_none() {
                c.tax_rate_id = self.conn.query_row("SELECT tax_rate_id FROM accounts WHERE id = ?1", [c.account_id.unwrap()], |r| r.get(0)).optional()?.flatten();
            }
            // "No tax" amounts carry no tax rate, so they stay out of GST/VAT returns (like "BAS excluded").
            if lat == Lat::NoTax { c.tax_rate_id = None; }
            let (rate_bp, comps, rt) = match (lat, c.tax_rate_id) {
                (Lat::NoTax, _) | (_, None) => (0, vec![], None),
                (_, Some(t)) => { let (r, cs, rt) = self.tax_rate(t)?; (r, cs, Some(rt)) }
            };
            c.report_type = rt;
            let parse = |s: &Option<String>, dp: u32, what: &str| -> Result<Option<i64>> {
                match s.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                    None => Ok(None),
                    Some(v) => money::parse_fixed(v, dp).map(Some).ok_or_else(|| Error::Validation(format!("line {}: invalid {what} '{v}'", i + 1))),
                }
            };
            let qty = parse(&l.quantity, 4, "quantity")?;
            let unit = parse(&l.unit_price, 4, "unit price")?.or(item_unit);
            let disc = parse(&l.discount, 2, "discount")?.unwrap_or(0);
            let amount = parse(&l.amount, 2, "amount")?;
            if disc < 0 || disc > 10_000 { return invalid(format!("line {}: discount must be between 0 and 100%", i + 1)); }
            let (la, net, tx) = if let (Some(a), None) = (amount, unit) {
                c.qty_e4 = qty.unwrap_or(10_000); c.unit_e2 = if c.qty_e4 != 0 { money::div_round(a as i128 * 1_000_000, c.qty_e4 as i128) as i64 } else { 0 };
                tax::split_amount(lat, a, rate_bp, &comps)
            } else {
                c.qty_e4 = qty.unwrap_or(10_000); c.unit_e2 = unit.unwrap_or(0); c.discount_bp = disc;
                tax::compute_line(lat, c.qty_e4, c.unit_e2, c.discount_bp, rate_bp, &comps)
            };
            c.line_amount = la; c.net = net; c.tax = tx;
            out.push(c);
        }
        Ok(out)
    }

    pub fn write_lines(&self, kind: &str, parent: i64, lines: &[CLine]) -> Result<()> {
        self.conn.execute("DELETE FROM line_items WHERE parent_kind = ?1 AND parent_id = ?2", params![kind, parent])?;
        let mut stmt = self.conn.prepare_cached(
            "INSERT INTO line_items(parent_kind, parent_id, sort, item_id, description, qty_e4, unit_e2, discount_bp, account_id, tax_rate_id, line_amount, net_amount, tax_amount, tracking1, tracking2)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)")?;
        for (i, l) in lines.iter().enumerate() {
            stmt.execute(params![kind, parent, i as i64, l.item_id, l.description, l.qty_e4, l.unit_e2, l.discount_bp, l.account_id, l.tax_rate_id,
                l.line_amount, l.net, l.tax, l.tracking1, l.tracking2])?;
        }
        Ok(())
    }

    pub fn read_lines(&self, kind: &str, parent: i64) -> Result<Vec<CLine>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT li.item_id, li.description, li.qty_e4, li.unit_e2, li.discount_bp, li.account_id, li.tax_rate_id, li.line_amount, li.net_amount, li.tax_amount, li.tracking1, li.tracking2, t.report_type
             FROM line_items li LEFT JOIN tax_rates t ON t.id = li.tax_rate_id WHERE li.parent_kind = ?1 AND li.parent_id = ?2 ORDER BY li.sort")?;
        let rows = stmt.query_map(params![kind, parent], |r| Ok(CLine {
            item_id: r.get(0)?, description: r.get(1)?, qty_e4: r.get(2)?, unit_e2: r.get(3)?, discount_bp: r.get(4)?, account_id: r.get(5)?,
            tax_rate_id: r.get(6)?, line_amount: r.get(7)?, net: r.get(8)?, tax: r.get(9)?, tracking1: r.get(10)?, tracking2: r.get(11)?, report_type: r.get(12)?,
        }))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Exchange rate for a currency on a date (1 base = rate foreign). Base currency is 1.
    pub fn rate_for(&self, currency: &str, date: &str) -> Result<i64> {
        if currency == self.base_currency()? { return Ok(100_000_000); }
        self.conn.query_row("SELECT rate_e8 FROM exchange_rates WHERE currency = ?1 AND date <= ?2 ORDER BY date DESC LIMIT 1", params![currency, date], |r| r.get(0))
            .optional()?.ok_or_else(|| Error::Validation(format!("no exchange rate for {currency} on or before {date}; enter a rate")))
    }

    pub fn currency_decimals(&self, code: &str) -> Result<u32> {
        Ok(self.conn.query_row("SELECT decimals FROM currencies WHERE code = ?1", [code], |r| r.get::<_, i64>(0)).optional()?.unwrap_or(2) as u32)
    }

    pub fn to_base_amount(&self, amount: i64, currency: &str, rate_e8: i64) -> Result<i64> {
        let base = self.base_currency()?;
        if currency == base { return Ok(amount); }
        Ok(money::to_base(amount, rate_e8, self.currency_decimals(currency)?, self.currency_decimals(&base)?))
    }

    /// Create (id = None) or update a document. Applies requested status at the end.
    pub fn save_document(&self, id: Option<i64>, d: &DocInput) -> Result<i64> {
        let kind = match id {
            Some(id) => self.conn.query_row("SELECT type FROM documents WHERE id = ?1", [id], |r| r.get::<_, String>(0))?,
            None => d.kind.clone(),
        };
        if !DOC_TYPES.contains(&kind.as_str()) { return invalid(format!("unknown document type {kind}")); }
        let sales = is_sales(&kind);
        util::parse_date(&d.date)?;
        let contact_id = match (d.contact_id, d.contact_name.as_deref().filter(|s| !s.trim().is_empty())) {
            (Some(c), _) => c,
            (None, Some(n)) => self.find_or_create_contact(n)?,
            _ => return invalid("a contact is required"),
        };
        let lat = Lat::parse(d.line_amount_type.as_deref().unwrap_or(""))?;
        let base = self.base_currency()?;
        let currency = d.currency.clone().filter(|c| !c.is_empty()).unwrap_or_else(|| base.clone());
        let rate = match d.rate.as_deref().filter(|s| !s.trim().is_empty()) {
            Some(r) if currency != base => money::parse_rate_e8(r).filter(|r| *r > 0).ok_or_else(|| Error::Validation("invalid exchange rate".into()))?,
            _ if currency == base => 100_000_000,
            _ => self.rate_for(&currency, &d.date)?,
        };
        let lines = self.compute_lines(lat, &d.lines, sales)?;
        let subtotal: i64 = lines.iter().map(|l| l.net).sum();
        let total_tax: i64 = lines.iter().filter(|l| l.report_type.as_deref() != Some("REVERSECHARGE")).map(|l| l.tax).sum();
        let total = subtotal + total_tax;
        let due = match d.due_date.as_deref().filter(|s| !s.is_empty()) {
            Some(s) => Some(s.to_string()),
            None if is_ledger_doc(&kind) => { let (t, n) = self.contact_terms(contact_id, sales)?; Some(util::due_date(&d.date, &t, n)?) }
            None => None,
        };
        let lat_s = match lat { Lat::Exclusive => "EXCLUSIVE", Lat::Inclusive => "INCLUSIVE", Lat::NoTax => "NOTAX" };

        self.atomic(|| {
            let (doc_id, prior_status) = match id {
                None => {
                    let number = match d.number.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                        Some(n) => n.to_string(),
                        None => match seq_for(&kind) {
                            // Auto-numbering skips numbers that are already taken (e.g. after the
                            // next number was set back in Invoice settings, or entered by hand).
                            Some(s) => {
                                let mut n = self.next_number(s)?;
                                for _ in 0..10_000 {
                                    let used: i64 = self.conn.query_row("SELECT COUNT(*) FROM documents WHERE type = ?1 AND number = ?2 AND status <> 'DELETED'", params![kind, n], |r| r.get(0))?;
                                    if used == 0 || kind == "ACCPAY" { break; }
                                    n = self.next_number(s)?;
                                }
                                n
                            }
                            None => String::new(),
                        },
                    };
                    if !number.is_empty() && kind != "ACCPAY" {
                        let dup: i64 = self.conn.query_row("SELECT COUNT(*) FROM documents WHERE type = ?1 AND number = ?2 AND status <> 'DELETED'", params![kind, number], |r| r.get(0))?;
                        if dup > 0 { return invalid(format!("{} number {number} is already used", entity_name(&kind))); }
                    }
                    let theme: Option<i64> = d.branding_theme_id.or(self.conn.query_row("SELECT id FROM branding_themes WHERE is_default = 1", [], |r| r.get(0)).optional()?);
                    self.conn.execute(
                        "INSERT INTO documents(type, number, contact_id, date, currency, rate_e8, branding_theme_id, online_token, source_document_id, repeating_id)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                        params![kind, number, contact_id, d.date, currency, rate, theme, util::random_token(24), d.source_document_id, d.repeating_id])?;
                    let nid = self.conn.last_insert_rowid();
                    self.add_note("document", nid, "HISTORY", &format!("{} created", capitalise(entity_name(&kind))))?;
                    (nid, "DRAFT".to_string())
                }
                Some(id) => {
                    let (status, paid, credited): (String, i64, i64) = self.conn.query_row("SELECT status, amount_paid, amount_credited FROM documents WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
                    match status.as_str() {
                        "DRAFT" | "SUBMITTED" | "SENT" => {}
                        "AUTHORISED" if paid == 0 && credited == 0 => {
                            // Editing an approved document: reverse its journal; it is re-posted below.
                            if is_ledger_doc(&kind) { self.unpost_document(id)?; }
                        }
                        "AUTHORISED" => return invalid(format!("this {} has payments or credits applied; remove them before editing", entity_name(&kind))),
                        _ => return invalid(format!("a {} {} cannot be edited", status.to_lowercase(), entity_name(&kind))),
                    }
                    if let Some(n) = d.number.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                        self.conn.execute("UPDATE documents SET number = ?2 WHERE id = ?1", params![id, n])?;
                    }
                    if let Some(t) = d.branding_theme_id { self.conn.execute("UPDATE documents SET branding_theme_id = ?2 WHERE id = ?1", params![id, t])?; }
                    self.add_note("document", id, "HISTORY", &format!("{} edited", capitalise(entity_name(&kind))))?;
                    (id, status)
                }
            };
            self.conn.execute(
                "UPDATE documents SET contact_id=?2, reference=?3, date=?4, due_date=?5, expiry_date=?6, delivery_date=?7, delivery_address=?8, attention=?9,
                   title=?10, summary=?11, terms=?12, line_amount_type=?13, currency=?14, rate_e8=?15, subtotal=?16, total_tax=?17, total=?18,
                   base_total=?19, expected_payment_date=?20, planned_payment_date=?21, updated_at=datetime('now') WHERE id=?1",
                params![doc_id, contact_id, d.reference, d.date, due, d.expiry_date.clone().filter(|s| !s.is_empty()), d.delivery_date.clone().filter(|s| !s.is_empty()),
                    d.delivery_address, d.attention, d.title, d.summary, d.terms, lat_s, currency, rate, subtotal, total_tax, total,
                    self.to_base_amount(total, &currency, rate)?, d.expected_payment_date.clone().filter(|s| !s.is_empty()), d.planned_payment_date.clone().filter(|s| !s.is_empty())])?;
            self.write_lines("DOC", doc_id, &lines)?;
            self.flag_contact_role(contact_id, sales && kind != "QUOTE" || kind == "QUOTE", !sales)?;
            // Status
            let want = d.status.clone().unwrap_or_else(|| if prior_status == "AUTHORISED" { "AUTHORISED".into() } else { prior_status.clone() });
            if want != "DRAFT" || prior_status != "DRAFT" {
                self.set_document_status_inner(doc_id, &want, prior_status == "AUTHORISED")?;
            }
            self.reindex_document(doc_id)?;
            self.audit(if id.is_some() { "UPDATE" } else { "CREATE" }, "document", Some(doc_id), None, Some(json!({"type": kind, "total": total, "status": want})))?;
            Ok(doc_id)
        })
    }

    fn reindex_document(&self, id: i64) -> Result<()> {
        let (kind, number, reference, contact, total, cur): (String, String, String, String, i64, String) = self.conn.query_row(
            "SELECT d.type, d.number, d.reference, c.name, d.total, d.currency FROM documents d JOIN contacts c ON c.id = d.contact_id WHERE d.id = ?1", [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
        let title = format!("{} {} {}", capitalise(entity_name(&kind)), number, contact);
        let descs: Vec<String> = self.read_lines("DOC", id)?.into_iter().map(|l| l.description).collect();
        self.index_search("document", id, &title, &format!("{reference} {} {cur} {}", money::fmt_money(total), descs.join(" ")))
    }

    /// Public status change (submit, approve, void, delete, mark sent/accepted/declined...).
    pub fn set_document_status(&self, id: i64, status: &str) -> Result<()> {
        self.atomic(|| self.set_document_status_inner(id, status, false))
    }

    fn set_document_status_inner(&self, id: i64, to: &str, reposting: bool) -> Result<()> {
        let (kind, from, paid, credited, date, journal): (String, String, i64, i64, String, Option<i64>) = self.conn.query_row(
            "SELECT type, status, amount_paid, amount_credited, date, journal_id FROM documents WHERE id = ?1", [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)))?;
        let from = if reposting { "SUBMITTED".to_string() } else { from };
        if from == to && journal.is_some() == (to == "AUTHORISED" && is_ledger_doc(&kind)) { return Ok(()); }
        let name = entity_name(&kind);
        let ok = match kind.as_str() {
            "QUOTE" => matches!((from.as_str(), to), ("DRAFT", "SENT") | ("DRAFT", "DELETED") | ("SENT", "ACCEPTED") | ("SENT", "DECLINED") | ("SENT", "DELETED")
                | ("ACCEPTED", "INVOICED") | ("SENT", "INVOICED") | ("DELETED", "DRAFT") | ("DECLINED", "SENT") | ("ACCEPTED", "SENT") | ("SENT", "DRAFT") | ("DRAFT", "DRAFT") | ("SENT", "SENT")),
            "PO" => matches!((from.as_str(), to), ("DRAFT", "SUBMITTED") | ("DRAFT", "AUTHORISED") | ("SUBMITTED", "AUTHORISED") | ("DRAFT", "DELETED")
                | ("SUBMITTED", "DELETED") | ("AUTHORISED", "BILLED") | ("AUTHORISED", "DELETED") | ("DELETED", "DRAFT") | ("BILLED", "AUTHORISED") | ("SUBMITTED", "DRAFT") | ("DRAFT", "DRAFT") | ("SUBMITTED", "SUBMITTED")),
            _ => matches!((from.as_str(), to), ("DRAFT", "SUBMITTED") | ("DRAFT", "AUTHORISED") | ("SUBMITTED", "AUTHORISED") | ("SUBMITTED", "DRAFT")
                | ("DRAFT", "DELETED") | ("SUBMITTED", "DELETED") | ("AUTHORISED", "VOIDED") | ("DRAFT", "DRAFT") | ("SUBMITTED", "SUBMITTED") | ("DELETED", "DRAFT")),
        };
        if !ok { return invalid(format!("cannot change a {} {name} to {}", from.to_lowercase(), to.to_lowercase())); }
        if to == "AUTHORISED" && is_ledger_doc(&kind) {
            self.validate_for_approval(id, &kind)?;
            let jid = self.post_document(id)?;
            self.conn.execute("UPDATE documents SET status = 'AUTHORISED', journal_id = ?2 WHERE id = ?1", params![id, jid])?;
            if !reposting { self.add_note("document", id, "HISTORY", &format!("{} approved", capitalise(name)))?; }
            return Ok(());
        }
        if to == "VOIDED" {
            if paid != 0 || credited != 0 { return invalid(format!("remove payments and credits before voiding this {name}")); }
            self.check_lock(&date)?;
            self.unpost_document(id)?;
            self.conn.execute("UPDATE documents SET status = 'VOIDED' WHERE id = ?1", [id])?;
            self.add_note("document", id, "HISTORY", &format!("{} voided", capitalise(name)))?;
            return Ok(());
        }
        if to == "DELETED" { self.unindex_search("document", id)?; }
        if from == "DELETED" { self.conn.execute("UPDATE documents SET status = ?2 WHERE id = ?1", params![id, to])?; self.reindex_document(id)?; self.add_note("document", id, "HISTORY", "Restored")?; return Ok(()); }
        self.conn.execute("UPDATE documents SET status = ?2, updated_at = datetime('now') WHERE id = ?1", params![id, to])?;
        let verb = match to { "SUBMITTED" => "submitted for approval", "DELETED" => "deleted", "SENT" => "marked as sent", "ACCEPTED" => "marked as accepted",
            "DECLINED" => "marked as declined", "INVOICED" => "marked as invoiced", "BILLED" => "marked as billed", "DRAFT" => "returned to draft", _ => "updated" };
        self.add_note("document", id, "HISTORY", &format!("{} {verb}", capitalise(name)))?;
        Ok(())
    }

    fn validate_for_approval(&self, id: i64, kind: &str) -> Result<()> {
        let lines = self.read_lines("DOC", id)?;
        if lines.is_empty() { return invalid(format!("add at least one line before approving this {}", entity_name(kind))); }
        for (i, l) in lines.iter().enumerate() {
            if l.account_id.is_none() { return invalid(format!("line {} needs an account", i + 1)); }
        }
        let total: i64 = self.conn.query_row("SELECT total FROM documents WHERE id = ?1", [id], |r| r.get(0))?;
        if total < 0 { return invalid(format!("the total of a {} cannot be negative; create a credit note instead", entity_name(kind))); }
        let date: String = self.conn.query_row("SELECT date FROM documents WHERE id = ?1", [id], |r| r.get(0))?;
        self.check_lock(&date)?;
        Ok(())
    }

    /// Post the journal for an approved invoice/bill/credit note (plus inventory movements).
    pub(crate) fn post_document(&self, id: i64) -> Result<i64> {
        let (kind, contact, date, currency, rate, number, reference): (String, i64, String, String, i64, String, String) = self.conn.query_row(
            "SELECT type, contact_id, date, currency, rate_e8, number, reference FROM documents WHERE id = ?1", [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)))?;
        let lines = self.read_lines("DOC", id)?;
        let s: i64 = match kind.as_str() { "ACCREC" => -1, "ACCRECCREDIT" => 1, "ACCPAY" => 1, "ACCPAYCREDIT" => -1, _ => return invalid("not a ledger document") };
        let control = if is_sales(&kind) { self.sys_account("AR")? } else { self.sys_account("AP")? };
        let gst = self.sys_account("GST")?;
        let mut jl = Vec::new();
        let mut movements: Vec<(i64, i64, i64)> = Vec::new();
        let mut total_fx = 0i64;
        for l in &lines {
            let acct = l.account_id.ok_or_else(|| Error::Validation("line missing account".into()))?;
            let net_b = self.to_base_amount(l.net, &currency, rate)?;
            let tax_b = self.to_base_amount(l.tax, &currency, rate)?;
            let reverse = l.report_type.as_deref() == Some("REVERSECHARGE");
            jl.push(JLine { account_id: acct, amount: s * net_b, description: l.description.clone(), tax_rate_id: l.tax_rate_id, tax_amount: s * tax_b,
                contact_id: Some(contact), tracking1: l.tracking1, tracking2: l.tracking2, currency: Some(currency.clone()), fx_amount: Some(s * l.net),
                item_id: l.item_id, qty_e4: Some(l.qty_e4) });
            total_fx += l.net;
            if l.tax != 0 && !reverse {
                jl.push(JLine { account_id: gst, amount: s * tax_b, description: l.description.clone(), contact_id: Some(contact),
                    tracking1: l.tracking1, tracking2: l.tracking2, currency: Some(currency.clone()), fx_amount: Some(s * l.tax), ..Default::default() });
                total_fx += l.tax;
            }
            if let Some(item) = l.item_id {
                let (tracked, inv, cogs) = self.item_tracking(item)?;
                if tracked {
                    let inv = inv.unwrap_or(self.sys_account("INVENTORY")?);
                    match kind.as_str() {
                        "ACCREC" => {
                            let cost = self.inventory_cost_out(item, l.qty_e4)?;
                            let cogs = cogs.ok_or_else(|| Error::Validation("tracked item needs a cost of goods sold account".into()))?;
                            jl.push(JLine::new(cogs, cost).desc(format!("COGS: {}", l.description)).contact(Some(contact)).tracking(l.tracking1, l.tracking2));
                            jl.push(JLine::new(inv, -cost).desc(format!("COGS: {}", l.description)).contact(Some(contact)));
                            movements.push((item, -l.qty_e4, -cost));
                        }
                        "ACCRECCREDIT" => {
                            let (qoh, tc): (i64, i64) = self.conn.query_row("SELECT qty_on_hand_e4, total_cost FROM items WHERE id = ?1", [item], |r| Ok((r.get(0)?, r.get(1)?)))?;
                            let unit = if qoh > 0 { money::div_round(tc as i128 * 10_000, qoh as i128) as i64 } else {
                                self.conn.query_row("SELECT COALESCE(purchase_unit_e2, 0) / 100 FROM items WHERE id = ?1", [item], |r| r.get(0))?
                            };
                            let cost = money::div_round(unit as i128 * l.qty_e4 as i128, 10_000) as i64;
                            if let Some(cogs) = cogs {
                                jl.push(JLine::new(inv, cost).desc(format!("Stock returned: {}", l.description)).contact(Some(contact)));
                                jl.push(JLine::new(cogs, -cost).desc(format!("Stock returned: {}", l.description)).contact(Some(contact)));
                                movements.push((item, l.qty_e4, cost));
                            }
                        }
                        "ACCPAY" => movements.push((item, l.qty_e4, net_b)),
                        "ACCPAYCREDIT" => { self.inventory_cost_out(item, l.qty_e4)?; movements.push((item, -l.qty_e4, -net_b)) }
                        _ => {}
                    }
                }
            }
        }
        let other: i64 = jl.iter().map(|l| l.amount).sum();
        jl.push(JLine { account_id: control, amount: -other, description: format!("{} {}", capitalise(entity_name(&kind)), number), contact_id: Some(contact),
            currency: Some(currency.clone()), fx_amount: Some(-s * total_fx), ..Default::default() });
        let jid = self.post_journal(NewJournal { date: date.clone(), source_type: kind.clone(), source_id: Some(id), reference: if number.is_empty() { reference.clone() } else { number.clone() },
            narration: reference, lines: jl, cash_basis: false, reverses: None })?;
        for (item, q, c) in movements { self.record_movement(item, &date, q, c, &kind, Some(id), Some(jid))?; }
        self.conn.execute("UPDATE documents SET base_total = ?2 WHERE id = ?1", params![id, other.abs()])?;
        Ok(jid)
    }

    /// Reverse a document's journal and inventory movements (void or edit of approved doc).
    pub(crate) fn unpost_document(&self, id: i64) -> Result<()> {
        let (jid, kind, date): (Option<i64>, String, String) = self.conn.query_row("SELECT journal_id, type, date FROM documents WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        if let Some(j) = jid {
            let rid = self.reverse_journal(j, None, None)?;
            let moves: Vec<(i64, i64, i64)> = {
                let mut st = self.conn.prepare("SELECT item_id, qty_e4, cost FROM inventory_movements WHERE source_type = ?1 AND source_id = ?2 AND journal_id = ?3")?;
                st.query_map(params![kind, id, j], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<rusqlite::Result<Vec<_>>>()?
            };
            for (item, q, c) in moves { self.record_movement(item, &date, -q, -c, "REVERSAL", Some(id), Some(rid))?; }
            self.conn.execute("UPDATE documents SET journal_id = NULL, void_journal_id = ?2 WHERE id = ?1", params![id, rid])?;
        }
        Ok(())
    }

    pub fn get_document(&self, id: i64) -> Result<Value> {
        let mut d = util::query_one_json(&self.conn,
            "SELECT d.*, c.name AS contact_name, c.email AS contact_email, c.postal_address AS contact_address, c.tax_number AS contact_tax_number,
               d.total - d.amount_paid - d.amount_credited AS amount_due,
               (d.status = 'AUTHORISED' AND d.due_date < date('now','localtime')) AS overdue
             FROM documents d JOIN contacts c ON c.id = d.contact_id WHERE d.id = ?1", [id])?;
        d["lines"] = json!(util::query_json(&self.conn,
            "SELECT li.*, a.code AS account_code, a.name AS account_name, t.name AS tax_rate_name, i.code AS item_code,
               t1.name AS tracking1_name, t2.name AS tracking2_name
             FROM line_items li LEFT JOIN accounts a ON a.id = li.account_id LEFT JOIN tax_rates t ON t.id = li.tax_rate_id
             LEFT JOIN items i ON i.id = li.item_id LEFT JOIN tracking_options t1 ON t1.id = li.tracking1 LEFT JOIN tracking_options t2 ON t2.id = li.tracking2
             WHERE li.parent_kind = 'DOC' AND li.parent_id = ?1 ORDER BY li.sort", [id])?);
        d["payments"] = json!(util::query_json(&self.conn,
            "SELECT p.*, a.name AS account_name FROM payments p JOIN accounts a ON a.id = p.account_id WHERE p.document_id = ?1 AND p.status <> 'DELETED' ORDER BY p.date, p.id", [id])?);
        d["allocations"] = json!(util::query_json(&self.conn,
            "SELECT ca.*, cn.number AS credit_number, inv.number AS document_number, cn.type AS credit_type
             FROM credit_allocations ca JOIN documents cn ON cn.id = ca.credit_note_id JOIN documents inv ON inv.id = ca.document_id
             WHERE (ca.credit_note_id = ?1 OR ca.document_id = ?1) AND ca.status <> 'DELETED' ORDER BY ca.date", [id])?);
        // tax summary by rate
        d["tax_summary"] = json!(util::query_json(&self.conn,
            "SELECT t.name, SUM(li.tax_amount) AS tax, SUM(li.net_amount) AS net FROM line_items li JOIN tax_rates t ON t.id = li.tax_rate_id
             WHERE li.parent_kind = 'DOC' AND li.parent_id = ?1 AND li.tax_amount <> 0 GROUP BY t.id", [id])?);
        d["history"] = json!(self.history("document", id)?);
        d["attachments"] = json!(util::query_json(&self.conn, "SELECT id, filename, mime, size, created_at FROM attachments WHERE entity = 'document' AND entity_id = ?1", [id])?);
        Ok(d)
    }

    /// List documents of a type, filtered by a status tab, with per-tab counts.
    pub fn list_documents(&self, kind: &str, tab: &str, q: &str, contact_id: Option<i64>, limit: i64, offset: i64) -> Result<Value> {
        let kinds: Vec<&str> = match kind { "CREDIT_SALES" => vec!["ACCRECCREDIT"], "CREDIT_PURCHASES" => vec!["ACCPAYCREDIT"],
            "SALES" => vec!["ACCREC", "ACCRECCREDIT"], "PURCHASES" => vec!["ACCPAY", "ACCPAYCREDIT"], "DELAYED" | "SALESORDER" => vec!["QUOTE"], k => vec![k] };
        // Delayed charges/credits and sales orders are stored as quotes with a subtype; each has its
        // own list. The subtype filter: '*' = any, '' = plain quotes only, else a LIKE pattern.
        let delayed = match kind { "DELAYED" => "DELAYED%", "SALESORDER" => "SALESORDER", "QUOTE" => "", _ => "*" };
        let k1 = kinds[0]; let k2 = *kinds.get(1).unwrap_or(&kinds[0]);
        let like = format!("%{}%", q.trim());
        let rows = util::query_json(&self.conn,
            "SELECT d.id, d.type, d.subtype, d.number, d.reference, d.date, d.due_date, d.expiry_date, d.status, d.currency, d.subtotal, d.total_tax, d.total, d.amount_paid, d.amount_credited,
               d.total - d.amount_paid - d.amount_credited AS amount_due, d.sent, d.title, d.contact_id, c.name AS contact_name, d.fully_paid_on, d.planned_payment_date, d.expected_payment_date,
               (d.status = 'AUTHORISED' AND d.due_date < date('now','localtime')) AS overdue,
               (SELECT MAX(p.date) FROM payments p WHERE p.document_id = d.id AND p.status <> 'DELETED') AS paid_date
             FROM documents d JOIN contacts c ON c.id = d.contact_id
             WHERE d.type IN (?1, ?2) AND (?6 IS NULL OR d.contact_id = ?6) AND (?8 = '*' OR (?8 = '' AND d.subtype = '') OR (?8 <> '' AND d.subtype LIKE ?8))
               AND (?3 = 'all' AND d.status <> 'DELETED' OR ?3 = 'overdue' AND d.status = 'AUTHORISED' AND d.due_date < date('now','localtime')
                    OR ?3 = 'awaiting' AND d.status = 'AUTHORISED' OR d.status = upper(?3))
               AND (?4 = '%%' OR d.number LIKE ?4 OR d.reference LIKE ?4 OR c.name LIKE ?4 OR printf('%.2f', d.total / 100.0) LIKE ?4)
             ORDER BY d.date DESC, d.id DESC LIMIT ?5 OFFSET ?7",
            params![k1, k2, tab, like, limit, contact_id, offset, delayed])?;
        let counts = util::query_one_json(&self.conn,
            "SELECT COUNT(CASE WHEN status <> 'DELETED' THEN 1 END) AS all_count,
               COUNT(CASE WHEN status = 'DRAFT' THEN 1 END) AS draft,
               COUNT(CASE WHEN status = 'SUBMITTED' THEN 1 END) AS submitted,
               COUNT(CASE WHEN status = 'AUTHORISED' THEN 1 END) AS awaiting,
               COUNT(CASE WHEN status = 'AUTHORISED' AND due_date < date('now','localtime') THEN 1 END) AS overdue,
               COUNT(CASE WHEN status = 'PAID' THEN 1 END) AS paid,
               COUNT(CASE WHEN status = 'SENT' THEN 1 END) AS sent,
               COUNT(CASE WHEN status = 'ACCEPTED' THEN 1 END) AS accepted,
               COUNT(CASE WHEN status = 'DECLINED' THEN 1 END) AS declined,
               COUNT(CASE WHEN status = 'INVOICED' THEN 1 END) AS invoiced,
               COUNT(CASE WHEN status = 'BILLED' THEN 1 END) AS billed,
               COALESCE(SUM(CASE WHEN status = 'DRAFT' THEN total END), 0) AS draft_total,
               COALESCE(SUM(CASE WHEN status = 'SUBMITTED' THEN total END), 0) AS submitted_total,
               COALESCE(SUM(CASE WHEN status = 'AUTHORISED' THEN base_total * (total - amount_paid - amount_credited) / MAX(total, 1) END), 0) AS awaiting_total,
               COALESCE(SUM(CASE WHEN status = 'AUTHORISED' AND due_date < date('now','localtime') THEN base_total * (total - amount_paid - amount_credited) / MAX(total, 1) END), 0) AS overdue_total
             FROM documents WHERE type IN (?1, ?2) AND (?3 IS NULL OR contact_id = ?3) AND (?4 = '*' OR (?4 = '' AND subtype = '') OR (?4 <> '' AND subtype LIKE ?4))", params![k1, k2, contact_id, delayed])?;
        Ok(json!({"rows": rows, "counts": counts}))
    }

    /// Copy a document to a new draft, optionally of another type (quote→invoice, PO→bill, invoice→credit note).
    pub fn copy_document(&self, id: i64, to_kind: &str, date: Option<&str>) -> Result<i64> {
        let d = self.get_document(id)?;
        let from_kind = d["type"].as_str().unwrap_or("").to_string();
        let lines: Vec<LineInput> = d["lines"].as_array().unwrap().iter().map(|l| LineInput {
            item_id: l["item_id"].as_i64(), description: l["description"].as_str().unwrap_or("").into(),
            quantity: Some(money::fmt_fixed(l["qty_e4"].as_i64().unwrap_or(0), 4)), unit_price: Some(money::fmt_fixed(l["unit_e2"].as_i64().unwrap_or(0), 4)),
            discount: Some(money::fmt_fixed(l["discount_bp"].as_i64().unwrap_or(0), 2)),
            account_id: if is_sales(&from_kind) == is_sales(to_kind) { l["account_id"].as_i64() } else { None },
            tax_rate_id: if is_sales(&from_kind) == is_sales(to_kind) { l["tax_rate_id"].as_i64() } else { None },
            tracking1: l["tracking1"].as_i64(), tracking2: l["tracking2"].as_i64(), amount: None,
        }).collect();
        let date = date.map(String::from).unwrap_or_else(util::today);
        let input = DocInput {
            kind: to_kind.into(), contact_id: d["contact_id"].as_i64(), reference: if from_kind == "QUOTE" || from_kind == "PO" { d["number"].as_str().unwrap_or("").into() } else { d["reference"].as_str().unwrap_or("").into() },
            date, line_amount_type: d["line_amount_type"].as_str().map(String::from), currency: d["currency"].as_str().map(String::from),
            lines, title: d["title"].as_str().unwrap_or("").into(), summary: d["summary"].as_str().unwrap_or("").into(), terms: d["terms"].as_str().unwrap_or("").into(),
            delivery_address: d["delivery_address"].as_str().unwrap_or("").into(), source_document_id: Some(id), ..Default::default()
        };
        self.atomic(|| {
            let nid = self.save_document(None, &input)?;
            match (from_kind.as_str(), to_kind) {
                ("QUOTE", "ACCREC") => { let st = d["status"].as_str().unwrap_or(""); if st == "ACCEPTED" || st == "SENT" { self.set_document_status_inner(id, "INVOICED", false)?; } }
                ("PO", "ACCPAY") => { if d["status"] == "AUTHORISED" { self.set_document_status_inner(id, "BILLED", false)?; } }
                _ => {}
            }
            self.add_note("document", nid, "HISTORY", &format!("Copied from {} {}", entity_name(&from_kind), d["number"].as_str().unwrap_or("")))?;
            Ok(nid)
        })
    }

    /// Expected (invoices) / planned (bills) payment date used by the cash-flow forecast.
    pub fn set_payment_dates(&self, id: i64, expected: Option<&str>, planned: Option<&str>) -> Result<()> {
        if let Some(d) = expected.filter(|d| !d.is_empty()) { util::parse_date(d)?; }
        if let Some(d) = planned.filter(|d| !d.is_empty()) { util::parse_date(d)?; }
        self.conn.execute("UPDATE documents SET expected_payment_date = ?2, planned_payment_date = ?3 WHERE id = ?1",
            params![id, expected.filter(|d| !d.is_empty()), planned.filter(|d| !d.is_empty())])?;
        Ok(())
    }

    pub fn mark_sent(&self, id: i64) -> Result<()> {
        self.conn.execute("UPDATE documents SET sent = 1 WHERE id = ?1", [id])?;
        let kind: String = self.conn.query_row("SELECT type FROM documents WHERE id = ?1", [id], |r| r.get(0))?;
        let status: String = self.conn.query_row("SELECT status FROM documents WHERE id = ?1", [id], |r| r.get(0))?;
        if kind == "QUOTE" && status == "DRAFT" { self.set_document_status(id, "SENT")?; }
        self.add_note("document", id, "HISTORY", "Marked as sent")?;
        Ok(())
    }

    /// Customer accepts or declines a quote from the online link.
    pub fn respond_to_quote(&self, token: &str, accept: bool, name: &str) -> Result<()> {
        let id = self.document_by_token(token)?;
        let (kind, status): (String, String) = self.conn.query_row("SELECT type, status FROM documents WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        if kind != "QUOTE" || status != "SENT" { return invalid("this quote can no longer be accepted or declined online"); }
        self.atomic(|| {
            self.set_document_status_inner(id, if accept { "ACCEPTED" } else { "DECLINED" }, false)?;
            let who: String = name.chars().filter(|c| !c.is_control()).take(80).collect();
            let by = if who.trim().is_empty() { String::new() } else { String::from(" by ") + who.trim() };
            self.add_note("document", id, "HISTORY", &(String::from("Quote ") + if accept { "accepted" } else { "declined" } + " online" + &by))?;
            Ok(())
        })
    }

    pub fn document_by_token(&self, token: &str) -> Result<i64> {
        self.conn.query_row("SELECT id FROM documents WHERE online_token = ?1 AND status NOT IN ('DELETED','DRAFT')", [token], |r| r.get(0)).map_err(|_| Error::NotFound("invoice".into()))
    }
}

pub fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => String::new() }
}
